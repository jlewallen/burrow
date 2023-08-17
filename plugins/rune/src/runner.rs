use anyhow::Result;
use rune::{
    runtime::{Object, Protocol, RuntimeContext, Shared},
    termcolor::{ColorChoice, StandardStream},
    Context, Diagnostics, Source, Sources, Value, Vm,
};
use std::{collections::HashSet, sync::Arc, time::Instant};
use tracing::*;

use kernel::{
    common::Json,
    prelude::{Effect, JsonValue, Perform, TaggedJson},
};

use crate::sources::*;

pub struct RuneRunner {
    _scripts: HashSet<ScriptSource>,
    _ctx: Context,
    _runtime: Arc<RuntimeContext>,
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
                ScriptSource::System(source) => Ok(Source::new("system".to_string(), source)),
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
        ctx.install(glue::create_integration_module()?)?;

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
            _scripts: scripts,
            _ctx: ctx,
            _runtime: runtime,
            vm,
        })
    }

    pub fn user(&mut self) -> Result<()> {
        self.evaluate_optional_function("user", ())?;

        Ok(())
    }

    pub fn before(&mut self, perform: Perform) -> Result<Option<Perform>> {
        match &perform {
            Perform::Raised(raised) => {
                if let Some(handlers) = self.handlers()? {
                    handlers.apply(raised.event.clone())?;
                }
            }
            _ => {}
        }

        self.evaluate_optional_function("before", (glue::BeforePerform(perform.clone()),))?;

        Ok(Some(perform))
    }

    pub fn after(&mut self, effect: Effect) -> Result<Effect> {
        self.evaluate_optional_function("after", (glue::AfterEffect(effect.clone()),))?;

        Ok(effect)
    }

    fn evaluate_optional_function<A>(&mut self, name: &str, args: A) -> Result<Option<rune::Value>>
    where
        A: rune::runtime::Args,
    {
        match &mut self.vm {
            Some(vm) => match vm.lookup_function([name]) {
                Ok(func) => match func.call::<A, rune::Value>(args) {
                    Ok(v) => Ok(Some(v)),
                    Err(e) => {
                        error!("rune: {}", e);
                        Ok(None)
                    }
                },
                Err(_) => Ok(None),
            },
            None => Ok(None),
        }
    }

    fn handlers(&mut self) -> Result<Option<Handlers>> {
        let vm = self.vm.as_ref().unwrap();

        let Ok(func) = vm.lookup_function(["handlers"]) else {
            debug!("handlers-unavailable");
            return Ok(None);
        };

        let Ok(obj)  = func.call::<_, rune::Value>(()) else {
            warn!("handlers-error");
            return Ok(None);
        };

        match obj {
            Value::Object(obj) => Ok(Some(Handlers::new(obj))),
            _ => Ok(None),
        }
    }
}

pub struct Handlers {
    handlers: Shared<Object>,
}

impl Default for Handlers {
    fn default() -> Self {
        Self {
            handlers: Shared::new(Object::default()),
        }
    }
}

impl Handlers {
    fn new(handlers: Shared<Object>) -> Self {
        Self { handlers }
    }

    fn apply(&self, json: TaggedJson) -> Result<()> {
        let handlers = self.handlers.borrow_ref()?;
        let Some(child) = handlers.get(json.tag()) else {
            info!("no-handler");
            return Ok(());
        };

        let json = json.value().clone();

        match child {
            Value::Object(object) => {
                if let Ok(json) = TaggedJson::new_from(json.into()) {
                    Handlers::new(object.clone()).apply(json)
                } else {
                    unimplemented!("unexpected handler value: {:?}", object)
                }
            }
            Value::Function(func) => {
                let bag = glue::Bag(json);

                func.borrow_ref().unwrap().call::<_, rune::Value>((bag,))?;

                Ok(())
            }
            _ => todo!(),
        }
    }
}

mod glue {
    use kernel::{
        prelude::{DomainError, EntityKey, EntityPtr, IntoEntityPtr, LookupBy},
        session::get_my_session,
    };
    use serde::Deserialize;

    use super::*;

    #[derive(rune::Any, Debug)]
    pub(super) struct BeforePerform(pub(super) Perform);

    impl BeforePerform {
        #[inline]
        fn string_debug(&self, s: &mut String) -> std::fmt::Result {
            use std::fmt::Write;
            write!(s, "{:?}", self.0)
        }
    }

    #[derive(rune::Any, Debug)]
    pub(super) struct AfterEffect(pub(super) Effect);

    impl AfterEffect {
        #[inline]
        fn string_debug(&self, s: &mut String) -> std::fmt::Result {
            use std::fmt::Write;
            write!(s, "{:?}", self.0)
        }
    }

    #[derive(Debug, rune::Any)]
    pub(super) struct Bag(pub(super) Json);

    impl Bag {
        #[inline]
        fn string_debug(&self, s: &mut String) -> std::fmt::Result {
            use std::fmt::Write;
            write!(s, "{:?}", self)
        }

        fn get(&self, key: &str) -> Option<LocalEntity> {
            self.0
                .tagged(key)
                .map(|r| r.value().clone().into_inner())
                // Just return NONE here instead of unwrapping.
                .and_then(|r| KeyOnly::from_json(r).ok())
                .map(|r| r.to_entity())
                // Right now we can be reasonable sure that there are no dangling EntityRef's
                // nearby. This is still a bummer, though.
                .map(|r| r.unwrap())
                .map(|r| LocalEntity(r))
        }

        fn item(&self) -> Option<LocalEntity> {
            self.get("item")
        }

        fn area(&self) -> Option<LocalEntity> {
            self.get("area")
        }

        fn living(&self) -> Option<LocalEntity> {
            self.get("living")
        }
    }

    #[derive(Debug, rune::Any)]
    pub(super) struct LocalEntity(EntityPtr);

    impl LocalEntity {
        #[inline]
        fn string_debug(&self, s: &mut String) -> std::fmt::Result {
            use std::fmt::Write;
            write!(s, "{:?}", self.0)
        }

        fn key(&self) -> String {
            self.0.key().key_to_string().to_owned()
        }

        fn name(&self) -> String {
            self.0.name().expect("Error getting name").unwrap()
        }
    }

    pub(super) fn create_integration_module() -> Result<rune::Module> {
        let mut module = rune::Module::default();
        module.function(["info"], |s: &str| {
            info!(target: "RUNE", "{}", s);
        })?;
        module.function(["debug"], |s: &str| {
            debug!(target: "RUNE", "{}", s);
        })?;
        module.ty::<BeforePerform>()?;
        module.inst_fn(Protocol::STRING_DEBUG, BeforePerform::string_debug)?;
        module.ty::<AfterEffect>()?;
        module.inst_fn(Protocol::STRING_DEBUG, AfterEffect::string_debug)?;
        module.ty::<Bag>()?;
        module.inst_fn(Protocol::STRING_DEBUG, Bag::string_debug)?;
        module.inst_fn("area", Bag::area)?;
        module.inst_fn("item", Bag::item)?;
        module.inst_fn("living", Bag::living)?;
        module.ty::<LocalEntity>()?;
        module.inst_fn(Protocol::STRING_DEBUG, LocalEntity::string_debug)?;
        module.inst_fn("key", LocalEntity::key)?;
        module.inst_fn("name", LocalEntity::name)?;
        Ok(module)
    }

    #[derive(Debug, Deserialize)]
    struct KeyOnly {
        key: EntityKey,
    }

    impl KeyOnly {
        fn from_json(value: JsonValue) -> Result<Self, serde_json::Error> {
            serde_json::from_value(value)
        }
    }

    impl IntoEntityPtr for KeyOnly {
        fn to_entity(&self) -> Result<EntityPtr, DomainError> {
            if !self.key.valid() {
                return Err(DomainError::InvalidKey);
            }
            get_my_session()?
                .entity(&LookupBy::Key(&self.key))?
                .ok_or(DomainError::DanglingEntity)
        }
    }
}

#[cfg(test)]
mod tests {
    use kernel::prelude::{Audience, Raised};
    use serde_json::json;

    use super::*;

    #[test]
    pub fn test_handlers_apply() -> Result<()> {
        let source = r#"
            pub fn held(bag) { }

            pub fn dropped(bag) { }

            pub fn left(bag) { }

            pub fn arrived(bag) { }

            pub fn handlers() {
                #{
                    "carrying": #{
                        "held": held,
                        "dropped": dropped
                    },
                    "moving": #{
                        "left": left,
                        "arrived": arrived
                    }
                }
            }
        "#;

        let mut runner = RuneRunner::new([ScriptSource::System(source.to_owned())].into())?;

        runner.before(Perform::Raised(Raised::new(
            Audience::Nobody, // Unused
            "UNUSED".to_owned(),
            TaggedJson::new_from(json!({
                "carrying": {
                    "dropped": {
                        "item": {
                            "name": "Dropped Item",
                            "key": "E-0"
                        }
                    }
                }
            }))?,
        )))?;

        Ok(())
    }

    #[test]
    pub fn test_missing_handler() -> Result<()> {
        let source = r#"
            pub fn handlers() {
                #{ }
            }
        "#;

        let mut runner = RuneRunner::new([ScriptSource::System(source.to_owned())].into())?;

        runner.before(Perform::Raised(Raised::new(
            Audience::Nobody, // Unused
            "UNUSED".to_owned(),
            TaggedJson::new_from(json!({
                "carrying": {
                    "dropped": {
                        "item": {
                            "name": "Dropped Item",
                            "key": "E-0"
                        }
                    }
                }
            }))?,
        )))?;

        Ok(())
    }

    #[test]
    pub fn test_missing_handlers_completely() -> Result<()> {
        let source = r#" "#;

        let mut runner = RuneRunner::new([ScriptSource::System(source.to_owned())].into())?;

        runner.before(Perform::Raised(Raised::new(
            Audience::Nobody, // Unused
            "UNUSED".to_owned(),
            TaggedJson::new_from(json!({
                "carrying": {
                    "dropped": {
                        "item": {
                            "name": "Dropped Item",
                            "key": "E-0"
                        }
                    }
                }
            }))?,
        )))?;

        Ok(())
    }
}
