use anyhow::Result;
use rune::{
    runtime::{Protocol, RuntimeContext},
    termcolor::{ColorChoice, StandardStream},
    Context, Diagnostics, Source, Sources, Value, Vm,
};
use std::{collections::HashSet, sync::Arc, time::Instant};
use tracing::*;

use kernel::prelude::Surroundings;

use crate::sources::*;

#[derive(rune::Any, Debug, Default)]
struct Thing {
    #[rune(get)]
    value: u32,
}

impl Thing {
    fn new() -> Self {
        Self { value: 0 }
    }

    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "Thing({:?})", self.value)
    }
}

#[derive(rune::Any, Debug)]
struct Perform(kernel::prelude::Perform);

impl Perform {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self.0)
    }
}

#[derive(rune::Any, Debug)]
struct Effect(kernel::prelude::Effect);

impl Effect {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self.0)
    }
}

#[derive(rune::Any, Debug)]
struct Incoming(kernel::prelude::Incoming);

impl Incoming {
    fn key(&self) -> &str {
        &self.0.key
    }

    fn value(&self) -> Result<Value> {
        Ok(serde_json::from_value(self.0.value.clone().into())?)
    }

    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "Incoming()")
    }
}

fn create_integration_module() -> Result<rune::Module> {
    let mut module = rune::Module::default();
    module.function(["info"], |s: &str| {
        info!(target: "RUNE", "{}", s);
    })?;
    module.function(["debug"], |s: &str| {
        debug!(target: "RUNE", "{}", s);
    })?;
    module.ty::<Thing>()?;
    module.function(["Thing", "new"], Thing::new)?;
    module.inst_fn(Protocol::STRING_DEBUG, Thing::string_debug)?;
    module.ty::<Incoming>()?;
    module.inst_fn(Protocol::STRING_DEBUG, Incoming::string_debug)?;
    module.inst_fn("key", Incoming::key)?;
    module.inst_fn("value", Incoming::value)?;
    module.ty::<Perform>()?;
    module.inst_fn(Protocol::STRING_DEBUG, Perform::string_debug)?;
    module.ty::<Effect>()?;
    module.inst_fn(Protocol::STRING_DEBUG, Effect::string_debug)?;
    Ok(module)
}

#[allow(dead_code)]
pub struct RuneRunner {
    scripts: HashSet<ScriptSource>,
    ctx: Context,
    runtime: Arc<RuntimeContext>,
    vm: Option<Vm>,
}

impl RuneRunner {
    pub fn new(scripts: HashSet<ScriptSource>) -> Result<Self> {
        debug!("runner:loading");
        let started = Instant::now();
        let sources = scripts
            .iter()
            .map(|script| match script {
                ScriptSource::File(path) => Ok(Source::from_path(path.as_path())?),
                ScriptSource::Entity(key, source) => Ok(Source::new(key.to_string(), source)),
            })
            .collect::<Result<Vec<_>>>()?;

        let mut sources = sources
            .into_iter()
            .fold(Sources::new(), |mut sources, source| {
                sources.insert(source);
                sources
            });

        debug!("runner:compiling");
        let mut ctx = Context::with_default_modules()?;
        ctx.install(rune_modules::time::module(true)?)?;
        ctx.install(rune_modules::json::module(true)?)?;
        ctx.install(rune_modules::rand::module(true)?)?;
        ctx.install(create_integration_module()?)?;

        let mut diagnostics = Diagnostics::new();
        let runtime: Arc<RuntimeContext> = Arc::new(ctx.runtime());
        let compiled = rune::prepare(&mut sources)
            .with_context(&ctx)
            .with_diagnostics(&mut diagnostics)
            .build();
        if !diagnostics.is_empty() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
        }

        let vm = match compiled {
            Ok(compiled) => {
                let vm = Vm::new(runtime.clone(), Arc::new(compiled));
                let elapsed = Instant::now() - started;
                info!("runner:ready {:?}", elapsed);
                Some(vm)
            }
            Err(e) => {
                warn!("{}", e);
                None
            }
        };

        Ok(Self {
            scripts,
            ctx,
            runtime,
            vm,
        })
    }

    pub fn user(&mut self) -> Result<()> {
        self.evaluate_optional_function("user", ())?;

        Ok(())
    }

    pub fn have_surroundings(&mut self, _surroundings: &Surroundings) -> Result<()> {
        self.evaluate_optional_function("have_surroundings", ())?;

        Ok(())
    }

    pub fn deliver(&mut self, incoming: &kernel::prelude::Incoming) -> Result<()> {
        self.evaluate_optional_function("deliver", (Incoming(incoming.clone()),))?;

        Ok(())
    }

    pub fn before(
        &mut self,
        perform: kernel::prelude::Perform,
    ) -> Result<Option<kernel::prelude::Perform>> {
        self.evaluate_optional_function("before", (Perform(perform.clone()),))?;

        Ok(Some(perform))
    }

    pub fn after(&mut self, effect: kernel::prelude::Effect) -> Result<kernel::prelude::Effect> {
        self.evaluate_optional_function("after", (Effect(effect.clone()),))?;

        Ok(effect)
    }

    fn evaluate_optional_function<A>(&mut self, name: &str, args: A) -> Result<rune::Value>
    where
        A: rune::runtime::Args,
    {
        match &mut self.vm {
            Some(vm) => match vm.lookup_function([name]) {
                Ok(hook_fn) => match hook_fn.call::<A, rune::Value>(args) {
                    Ok(_v) => Ok(rune::Value::Unit),
                    Err(e) => {
                        error!("rune: {}", e);
                        Ok(rune::Value::Unit)
                    }
                },
                Err(_) => Ok(rune::Value::Unit),
            },
            None => Ok(rune::Value::Unit),
        }
    }
}
