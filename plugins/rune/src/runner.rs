use anyhow::Result;
use rune::{
    runtime::{Object, RuntimeContext, Shared},
    termcolor::{ColorChoice, StandardStream},
    Context, Diagnostics, Sources, Value, Vm,
};
use std::{sync::Arc, time::Instant};
use tracing::*;

use kernel::prelude::{Effect, Perform, SchemaCollection, TaggedJson};

use crate::{
    module::{AfterEffect, Bag, BeforePerform},
    sources::*,
    LogEntry,
};

#[derive(Default, Clone)]
struct Log {
    entries: Vec<LogEntry>,
}

impl Log {}

impl Into<Vec<LogEntry>> for Log {
    fn into(self) -> Vec<LogEntry> {
        self.entries
    }
}

impl From<Vec<LogEntry>> for Log {
    fn from(entries: Vec<LogEntry>) -> Self {
        Self { entries }
    }
}

pub struct RuneRunner {
    _ctx: Context,
    _runtime: Arc<RuntimeContext>,
    logs: Vec<LogEntry>,
    vm: Option<Vm>,
}

impl RuneRunner {
    pub fn new(schema: &SchemaCollection, script: Script) -> Result<Self> {
        debug!("runner:loading");
        let started = Instant::now();

        let library_sources = load_library_sources()?;

        let mut sources = Sources::new();
        sources.insert(script.source()?);
        for source in library_sources {
            sources.insert(source.source()?);
        }

        debug!("runner:compiling");
        let mut ctx = Context::with_default_modules()?;
        ctx.install(rune_modules::time::module(true)?)?;
        ctx.install(rune_modules::json::module(true)?)?;
        ctx.install(rune_modules::rand::module(true)?)?;
        ctx.install(super::module::create(schema, script.owner)?)?;

        let mut logs: Vec<LogEntry> = Vec::new();
        let mut diagnostics = Diagnostics::new();
        let compiled = rune::prepare(&mut sources)
            .with_context(&ctx)
            .with_diagnostics(&mut diagnostics)
            .build();
        if diagnostics.has_error() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;

            logs.extend(
                diagnostics
                    .into_diagnostics()
                    .into_iter()
                    .map(|d| match d {
                        rune::diagnostics::Diagnostic::Fatal(fatal) => fatal.to_string(),
                        rune::diagnostics::Diagnostic::Warning(warning) => warning.to_string(),
                        _ => todo!("New diagnostic!"),
                    })
                    .map(LogEntry::new_now)
                    .collect::<Vec<_>>(),
            );
        }

        let runtime: Arc<RuntimeContext> = Arc::new(ctx.runtime());
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
            _ctx: ctx,
            _runtime: runtime,
            logs,
            vm,
        })
    }

    pub fn call_handlers(&mut self, perform: Perform) -> Result<Option<rune::runtime::Value>> {
        match &perform {
            Perform::Raised(raised) => {
                if let Some(handlers) = self.handlers()? {
                    handlers.apply(raised.event.clone())
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    pub fn before(&mut self, perform: Perform) -> Result<Option<Perform>> {
        self.invoke("before", (BeforePerform(perform.clone()),))?;

        Ok(Some(perform))
    }

    pub fn after(&mut self, effect: Effect) -> Result<Effect> {
        self.invoke("after", (AfterEffect(effect.clone()),))?;

        Ok(effect)
    }

    fn invoke<A>(&mut self, name: &str, args: A) -> Result<Option<rune::Value>>
    where
        A: rune::runtime::Args,
    {
        match &mut self.vm {
            Some(vm) => match vm.lookup_function([name]) {
                Ok(func) => match func.call::<A, rune::Value>(args) {
                    rune::runtime::VmResult::Ok(v) => Ok(Some(v)),
                    rune::runtime::VmResult::Err(e) => {
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
        let Some(vm) = self.vm.as_ref() else {
            return Ok(None);
        };

        let Ok(func) = vm.lookup_function(["handlers"]) else {
            debug!("handlers-unavailable");
            return Ok(None);
        };

        match func.call::<_, rune::Value>(()) {
            rune::runtime::VmResult::Ok(obj) => match obj {
                Value::Object(obj) => Ok(Some(Handlers::new(obj))),
                _ => Ok(None),
            },
            rune::runtime::VmResult::Err(e) => {
                warn!("handlers-error {}", e);
                Ok(None)
            }
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

    fn apply(&self, json: TaggedJson) -> Result<Option<rune::runtime::Value>> {
        let handlers = self.handlers.borrow_ref()?;
        let Some(child) = handlers.get(json.tag()) else {
            info!("no-handler");
            return Ok(None);
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
                let bag = Bag(json);

                Ok(Some(
                    match func.borrow_ref().unwrap().call::<_, rune::Value>((bag,)) {
                        rune::runtime::VmResult::Ok(v) => v,
                        rune::runtime::VmResult::Err(_) => todo!(),
                    },
                ))
            }
            _ => todo!(),
        }
    }
}
