use anyhow::Result;
use rune::{
    runtime::{Object, RuntimeContext, Shared},
    termcolor::{ColorChoice, StandardStream, WriteColor},
    Context, Diagnostics, Sources, Value, Vm,
};
use std::{cell::RefCell, io::Write, sync::Arc, time::Instant};
use tracing::*;

use kernel::{
    prelude::{
        Effect, EntityKey, LookupBy, OpenScopeRefMut, Perform, Raised, SchemaCollection, TaggedJson,
    },
    session::get_my_session,
};

use crate::{
    module::{AfterEffect, Bag, BeforePerform},
    sources::*,
    Behaviors, LogEntry,
};

#[derive(Default, Clone)]
struct Log {
    entries: Vec<LogEntry>,
}

#[derive(Default)]
struct StreamedLines {
    data: Vec<u8>,
}

impl StreamedLines {
    fn entries(self) -> Vec<LogEntry> {
        vec![LogEntry::new_now(
            String::from_utf8(self.data)
                .expect("non-utf8 streamed lines")
                .trim(),
        )]
    }
}

impl std::io::Write for StreamedLines {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.data.extend(buf);

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl WriteColor for StreamedLines {
    fn supports_color(&self) -> bool {
        false
    }

    fn set_color(&mut self, _spec: &rune::termcolor::ColorSpec) -> std::io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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
    owner: Option<Owner>,
    logs: Option<Vec<LogEntry>>,
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

        let owner = script.owner.clone();

        debug!("runner:compiling");
        let mut ctx = Context::with_default_modules()?;
        ctx.install(rune_modules::time::module(true)?)?;
        ctx.install(rune_modules::json::module(true)?)?;
        ctx.install(rune_modules::rand::module(true)?)?;
        ctx.install(super::module::create(schema, script.owner)?)?;

        let mut diagnostics = Diagnostics::new();
        let compiled = rune::prepare(&mut sources)
            .with_context(&ctx)
            .with_diagnostics(&mut diagnostics)
            .build();

        let logs = if diagnostics.has_error() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
            writer.flush()?;

            let mut lines = StreamedLines::default();
            diagnostics.emit(&mut lines, &sources)?;
            lines.flush()?;

            Some(lines.entries())
        } else {
            Some(vec![LogEntry::new_now("compiled!".to_owned())])
        };

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
            owner,
            logs,
            vm,
        })
    }

    pub fn owner(&self) -> Option<&Owner> {
        self.owner.as_ref()
    }

    pub fn logs(&mut self) -> Option<Vec<LogEntry>> {
        self.logs.take()
    }

    pub fn call(&mut self, call: Call) -> Result<Option<Value>> {
        match call {
            Call::Handlers(raised) => {
                if let Some(handlers) = self.handlers()? {
                    handlers.apply(raised.event.clone())
                } else {
                    Ok(None)
                }
            }
            Call::Action(tagged) => {
                if let Some(actions) = self.actions()? {
                    actions.apply(tagged.clone())
                } else {
                    Ok(None)
                }
            }
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

    fn handlers(&mut self) -> Result<Option<FunctionTree>> {
        self.lookup_function_tree("handlers")
    }

    fn actions(&mut self) -> Result<Option<FunctionTree>> {
        self.lookup_function_tree("actions")
    }

    fn lookup_function_tree(&mut self, name: &str) -> Result<Option<FunctionTree>> {
        let Some(vm) = self.vm.as_ref() else {
            return Ok(None);
        };

        let Ok(func) = vm.lookup_function([name]) else {
            return Ok(None);
        };

        match func.call::<_, rune::Value>(()) {
            rune::runtime::VmResult::Ok(obj) => match obj {
                Value::Object(obj) => Ok(Some(FunctionTree::new(obj))),
                _ => Ok(None),
            },
            rune::runtime::VmResult::Err(e) => {
                warn!("handlers-error {}", e);
                Ok(None)
            }
        }
    }
}

#[derive(Clone)]
pub enum Call {
    Handlers(Raised),
    Action(TaggedJson),
}

pub struct FunctionTree {
    object: Shared<Object>,
}

impl Default for FunctionTree {
    fn default() -> Self {
        Self {
            object: Shared::new(Object::default()),
        }
    }
}

impl FunctionTree {
    fn new(object: Shared<Object>) -> Self {
        Self { object }
    }

    fn apply(&self, json: TaggedJson) -> Result<Option<rune::runtime::Value>> {
        let object = self.object.borrow_ref()?;
        let Some(child) = object.get(json.tag()) else {
            info!("no-handler");
            return Ok(None);
        };

        let json = json.value().clone();

        match child {
            Value::Object(object) => {
                if let Ok(json) = TaggedJson::new_from(json.into()) {
                    Self::new(object.clone()).apply(json)
                } else {
                    unimplemented!("unexpected handler value: {:?}", object)
                }
            }
            Value::Function(func) => {
                let bag = Bag(json);

                Ok(Some(
                    match func.borrow_ref().unwrap().call::<_, rune::Value>((bag,)) {
                        rune::runtime::VmResult::Ok(v) => v,
                        rune::runtime::VmResult::Err(e) => {
                            warn!("{:?}", e);

                            rune::Value::EmptyTuple
                        }
                    },
                ))
            }
            _ => todo!(),
        }
    }
}

#[derive(Default)]
pub struct Runners {
    schema: Option<SchemaCollection>,
    runners: Vec<RuneRunner>,
}

impl Runners {
    fn add_runners_for(&mut self, scripts: impl Iterator<Item = Script>) -> Result<()> {
        for script in scripts {
            self.runners
                .push(RuneRunner::new(self.schema.as_ref().unwrap(), script)?);
        }

        Ok(())
    }

    fn call(&mut self, call: Call) -> Result<Vec<Value>> {
        let from_handler = self
            .runners
            .iter_mut()
            .map(|runner| runner.call(call.clone()))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        self.flush()?;

        Ok(from_handler)
    }

    fn flush(&mut self) -> Result<()> {
        for runner in self.runners.iter_mut() {
            if let Some(owner) = runner.owner().cloned() {
                if let Some(logs) = runner.logs() {
                    flush_logs(owner, logs)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct SharedRunners(Arc<RefCell<Runners>>);

impl SharedRunners {
    pub fn new(runners: Arc<RefCell<Runners>>) -> Self {
        Self(runners)
    }
}

impl SharedRunners {
    pub fn weak(&self) -> std::sync::Weak<RefCell<Runners>> {
        Arc::downgrade(&self.0)
    }

    pub fn add_runners_for(&self, scripts: impl Iterator<Item = Script>) -> Result<()> {
        let mut runners = self.0.borrow_mut();
        runners.add_runners_for(scripts)
    }

    pub fn initialize(&self, schema: &SchemaCollection) {
        let mut slf = self.0.borrow_mut();
        slf.schema = Some(schema.clone())
    }

    pub fn call(&self, call: Call) -> Result<Vec<rune::runtime::Value>> {
        let mut runners = self.0.borrow_mut();
        runners.call(call)
    }

    pub fn before(&self, value: Perform) -> Result<Option<Perform>> {
        let mut runners = self.0.borrow_mut();

        let before = runners
            .runners
            .iter_mut()
            .fold(Some(value), |perform, runner| {
                perform.and_then(|perform| runner.before(perform).expect("Error in before"))
            });

        runners.flush()?;

        Ok(before)
    }

    pub fn after(&self, value: Effect) -> Result<Effect> {
        let mut runners = self.0.borrow_mut();

        let after = runners.runners.iter_mut().fold(value, |effect, runner| {
            runner.after(effect).expect("Error in after")
        });

        runners.flush()?;

        info!("after");

        Ok(after)
    }
}

fn flush_logs(owner: Owner, logs: Vec<LogEntry>) -> Result<()> {
    let Some(owner) = get_my_session()?.entity(&LookupBy::Key(&EntityKey::new(&owner.key())))? else {
        panic!("error getting owner");
    };

    let mut behaviors = owner.scope_mut::<Behaviors>()?;
    let Some (rune) = behaviors
        .langs
        .get_or_insert_with(|| panic!("expected langs"))
        .get_mut(RUNE_EXTENSION) else {
        panic!("expected rune");
    };

    let skipping = match rune.logs.last() {
        Some(last) => match logs.as_slice() {
            [] => panic!(),
            [solo] => last.message == solo.message,
            _ => false,
        },
        None => false,
    };

    if !skipping {
        rune.logs.extend(logs);
        behaviors.save()?;
    }

    Ok(())
}
