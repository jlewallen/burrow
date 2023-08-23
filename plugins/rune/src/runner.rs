use anyhow::Result;
use rune::{
    runtime::{Object, RuntimeContext, Shared},
    termcolor::{ColorChoice, StandardStream, WriteColor},
    Context, Diagnostics, Sources, Value, Vm,
};
use std::{cell::RefCell, io::Write, sync::Arc, time::Instant};
use tracing::*;

use kernel::{
    here,
    prelude::{
        Effect, EntityKey, LookupBy, OpenScopeRefMut, Perform, Raised, SchemaCollection, TaggedJson,
    },
    session::get_my_session,
};

use crate::{
    module::{AfterEffect, Bag, BeforePerform},
    sources::*,
    Behaviors, LogEntry, RuneState,
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
    state: Option<RuneState>,
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
            state: None,
            vm,
        })
    }

    pub fn call(&mut self, call: Call) -> Result<Option<PostEvaluation<rune::runtime::Value>>> {
        match call {
            Call::Handlers(raised) => {
                if let Some(handlers) = self.handlers()? {
                    Ok(handlers
                        .apply(self.state.clone(), raised.event.clone())?
                        .map(|v| self.post(v)))
                } else {
                    Ok(None)
                }
            }
            Call::Action(tagged) => {
                if let Some(actions) = self.actions()? {
                    Ok(actions
                        .apply(None, tagged.clone())?
                        .map(|v| Some(self.post(v)))
                        .flatten())
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn before(&mut self, perform: Perform) -> Result<Option<PostEvaluation<Perform>>> {
        self.invoke("before", (BeforePerform(perform.clone()),))?;

        Ok(Some(self.post(perform)))
    }

    pub fn after(&mut self, effect: Effect) -> Result<PostEvaluation<Effect>> {
        self.invoke("after", (AfterEffect(effect.clone()),))?;

        Ok(self.post(effect))
    }

    fn post<T>(&mut self, value: T) -> PostEvaluation<T> {
        PostEvaluation::new(self.owner.clone(), self.logs.take(), value)
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
pub struct PostEvaluation<T> {
    owner: Option<Owner>,
    logs: Option<Vec<LogEntry>>,
    value: T,
}

fn have_logs_changed(tail: &Vec<LogEntry>, test: Option<&Vec<LogEntry>>) -> bool {
    match tail.last() {
        Some(last) => match test {
            Some(logs) => match logs.as_slice() {
                [] => panic!(),
                [solo] => last.message == solo.message,
                _ => false,
            },
            None => false,
        },
        None => false,
    }
}
impl<T> PostEvaluation<T> {
    pub fn new(owner: Option<Owner>, logs: Option<Vec<LogEntry>>, value: T) -> Self {
        Self { owner, logs, value }
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> PostEvaluation<T>
where
    T: Simplifies,
{
    fn flush(mut self) -> Result<T> {
        let Some(owner) = self.owner else {
            warn!("flush: ownerless");
            return Ok(self.value);
        };

        let owner = get_my_session()?
            .entity(&LookupBy::Key(&EntityKey::new(&owner.key())))?
            .expect("Error getting owner");

        let state: Option<RuneState> = self
            .value
            .simplify()?
            .into_iter()
            .flat_map(|f| match f {
                Returned::Tagged(_) => None,
                Returned::State(state) => Some(state),
            })
            .last();

        let mut behaviors = owner.scope_mut::<Behaviors>()?;
        let rune = behaviors
            .langs
            .get_or_insert_with(|| panic!("Expected langs"))
            .get_mut(RUNE_EXTENSION)
            .expect("Expected rune");

        let logs = self.logs.take();
        let save_logs = if have_logs_changed(&rune.logs, logs.as_ref()) {
            rune.logs.extend(logs.unwrap_or_default());
            true
        } else {
            false
        };

        let save_state = if let Some(state) = state {
            rune.state = Some(state);
            true
        } else {
            false
        };

        if save_logs || save_state {
            behaviors.save()?;
        }

        Ok(self.value)
    }
}

impl Into<RuneReturn> for PostEvaluation<rune::runtime::Value> {
    fn into(self) -> RuneReturn {
        RuneReturn { value: self.value }
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

    fn apply(
        &self,
        state: Option<RuneState>,
        json: TaggedJson,
    ) -> Result<Option<rune::runtime::Value>> {
        let object = self.object.borrow_ref()?;
        let Some(child) = object.get(json.tag()) else {
            info!("no-handler");
            return Ok(None);
        };

        let json = json.value().clone();

        match child {
            Value::Object(object) => {
                if let Ok(json) = TaggedJson::new_from(json.into()) {
                    Self::new(object.clone()).apply(state.clone(), json)
                } else {
                    unimplemented!("unexpected handler value: {:?}", object)
                }
            }
            Value::Function(func) => {
                let bag = Bag(json);

                Ok(Some(
                    match func
                        .borrow_ref()
                        .unwrap()
                        .call::<_, rune::Value>((state, bag))
                    {
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

    fn call(&mut self, call: Call) -> Result<RuneReturn> {
        Ok(RuneReturn::new(
            self.runners
                .iter_mut()
                .map(|runner| runner.call(call.clone()))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<PostEvaluation<_>>>()
                .into_iter()
                .map(|pe| pe.flush())
                .collect::<Result<Vec<rune::runtime::Value>>>()?,
        )?)
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

    pub fn call(&self, call: Call) -> Result<RuneReturn> {
        let mut runners = self.0.borrow_mut();
        let returned = runners.call(call)?;

        Ok(returned)
    }

    pub fn before(&self, value: Perform) -> Result<Option<Perform>> {
        let mut runners = self.0.borrow_mut();

        let before = runners
            .runners
            .iter_mut()
            .fold(Some(value), |perform, runner| {
                perform.and_then(|perform| {
                    runner
                        .before(perform)
                        .expect("Error in before")
                        .map(|v| v.flush().expect("Error in flush"))
                })
            });

        Ok(before)
    }

    pub fn after(&self, value: Effect) -> Result<Effect> {
        let mut runners = self.0.borrow_mut();

        let after = runners.runners.iter_mut().fold(value, |effect, runner| {
            runner.after(effect).expect("Error in after").into_inner()
        });

        info!("after");

        Ok(after)
    }
}

pub enum Returned {
    Tagged(TaggedJson),
    State(RuneState),
}

pub struct RuneReturn {
    value: rune::runtime::Value,
}

impl RuneReturn {
    pub fn new(v: Vec<rune::runtime::Value>) -> Result<Self> {
        use anyhow::Context;
        let value = rune::runtime::to_value(v).with_context(|| here!())?;
        Ok(Self { value })
    }
}

pub trait Simplifies {
    fn simplify(&self) -> Result<Vec<Returned>>;
}

impl Simplifies for Perform {
    fn simplify(&self) -> Result<Vec<Returned>> {
        Ok(vec![])
    }
}

impl Simplifies for Effect {
    fn simplify(&self) -> Result<Vec<Returned>> {
        Ok(vec![])
    }
}

impl Simplifies for RuneReturn {
    fn simplify(&self) -> Result<Vec<Returned>> {
        use anyhow::Context;
        self.value.simplify().with_context(|| here!())
    }
}

impl Simplifies for rune::runtime::Value {
    fn simplify(&self) -> Result<Vec<Returned>> {
        use anyhow::Context;

        // Annoying that Object doesn't impl Serialize so this clone.
        match self.clone() {
            rune::Value::Object(_object) => {
                let value = serde_json::to_value(self.clone())?;
                info!("{:#?}", &value);

                let tagged = TaggedJson::new_from(value)?;
                Ok(vec![Returned::Tagged(tagged)])
            }
            rune::Value::Vec(vec) => {
                let vec = vec.borrow_ref()?;
                Ok(vec
                    .iter()
                    .map(|rr| rr.simplify())
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect())
            }
            rune::Value::Option(value) => {
                let value = value.borrow_ref().with_context(|| here!())?;
                if let Some(value) = value.clone() {
                    value.simplify()
                } else {
                    Ok(vec![])
                }
            }
            rune::Value::Any(_value) => {
                let value = self.clone();
                let state: RuneState = rune::runtime::from_value(value).with_context(|| here!())?;

                Ok(vec![Returned::State(state)])
            }
            rune::Value::EmptyTuple => Ok(vec![]),
            _ => {
                warn!("Unexpected rune return: {:?}", self);

                Ok(vec![])
            }
        }
    }
}
