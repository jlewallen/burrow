use anyhow::Context as _;
use anyhow::Result;
use plugins_core::sched::model::DateTime;
use plugins_core::sched::model::Utc;
use rune::{
    runtime::{Object, RuntimeContext, Shared},
    termcolor::{ColorChoice, StandardStream, WriteColor},
    Context, Diagnostics, Sources, Value, Vm,
};
use serde::ser::SerializeMap;
use serde::Deserialize;
use serde::Serialize;
use std::{cell::RefCell, io::Write, sync::Arc, time::Instant};
use tracing::*;

use kernel::{
    here,
    prelude::{
        Effect, EntityKey, EntityPtr, JsonValue, LookupBy, OpenScopeRefMut, Perform, Raised,
        SchemaCollection, TaggedJson,
    },
    session::get_my_session,
};

use crate::{
    module::{AfterEffect, Bag, BeforePerform},
    sources::*,
    Behaviors, RuneState,
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LogEntry {
    pub time: DateTime<Utc>,
    pub message: String,
}

impl LogEntry {
    pub fn new_now(message: impl Into<String>) -> Self {
        Self {
            time: Utc::now(),
            message: message.into(),
        }
    }
}

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

pub type RuneValue = rune::Value;

enum State {
    None,
    Loaded(JsonValue),
    Created(RuneValue),
}

pub struct RuneRunner {
    #[allow(dead_code)]
    source: String,
    owner: Option<Owner>,
    state: State,
    vm: Option<Vm>,
}

impl RuneRunner {
    pub fn new(schema: &SchemaCollection, script: Script) -> Result<Self> {
        let started = Instant::now();

        let library_sources = load_library_sources()?;

        let source = script.describe();

        let mut sources = Sources::new();
        sources.insert(script.source()?);
        for source in library_sources {
            sources.insert(source.source()?);
        }

        let owner = script.owner.clone();

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

        if diagnostics.has_error() {
            let mut writer = StandardStream::stderr(ColorChoice::Always);
            diagnostics.emit(&mut writer, &sources)?;
            writer.flush()?;

            let mut lines = StreamedLines::default();
            diagnostics.emit(&mut lines, &sources)?;
            lines.flush()?;

            for entry in lines.entries().into_iter() {
                info!("diagnostics {}", &entry.message);
            }
        };

        let runtime: Arc<RuntimeContext> = Arc::new(ctx.runtime());
        let vm = match compiled {
            Ok(compiled) => {
                let vm = Vm::new(runtime.clone(), Arc::new(compiled));
                let elapsed = Instant::now() - started;
                info!(source = source, "runner:ready {:?}", elapsed);
                Some(vm)
            }
            Err(e) => {
                warn!("{}", e);
                None
            }
        };

        let state = script
            .state
            .map(|s| State::Loaded(s))
            .unwrap_or(State::None);

        Ok(Self {
            source,
            owner,
            state,
            vm,
        })
    }

    pub fn call(&mut self, call: Call) -> Result<Option<PostEvaluation<rune::runtime::Value>>> {
        match call {
            Call::Handlers(raised) => {
                if let Some(handlers) = self.handlers()? {
                    Ok(handlers
                        .apply(self.get_or_load_state()?, raised.event.clone())?
                        .map(|v| self.post(v)))
                } else {
                    Ok(None)
                }
            }
            Call::Action(tagged) => {
                if let Some(actions) = self.actions()? {
                    Ok(actions
                        .apply::<RuneValue>(None, tagged.clone())?
                        .map(|v| Some(self.post(v)))
                        .flatten())
                } else {
                    Ok(None)
                }
            }
            Call::Register => Ok(self
                .invoke("register", ())?
                .map(|v| Some(self.post(v)))
                .flatten()),
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
        PostEvaluation::new(self.owner.clone(), value)
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

    fn get_or_load_state(&mut self) -> Result<Option<RuneValue>> {
        match &self.state {
            State::None => Ok(None),
            State::Loaded(loaded) => {
                if let Some(loaded) = self.load_state(Some(loaded.clone()))? {
                    self.state = State::Created(loaded.clone());
                    Ok(Some(loaded))
                } else {
                    Ok(None)
                }
            }
            State::Created(loaded) => Ok(Some(loaded.clone())),
        }
    }

    fn load_state(&self, loaded: Option<JsonValue>) -> Result<Option<RuneValue>> {
        if let Some(loaded) = &loaded {
            if let Some(vm) = &self.vm {
                match vm.lookup_function(["create_state"]) {
                    Ok(state_fn) => match state_fn.call::<_, rune::Value>(()) {
                        rune::runtime::VmResult::Ok(value) => {
                            Ok(Some(update_state_in_place(value.clone(), loaded)?))
                        }
                        rune::runtime::VmResult::Err(e) => {
                            warn!("{:?}", e);

                            Ok(None)
                        }
                    },
                    Err(_) => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}

fn update_state_in_place(value: RuneValue, setting: &JsonValue) -> Result<RuneValue> {
    debug!("state:updating {:?} {:?}", &value, setting);

    match &value {
        Value::Struct(truct) => {
            let try_get: Value = serde_json::from_value(setting.clone())?;
            let state_object = try_get.into_object().into_result()?;
            let copying = state_object.borrow_ref()?;
            let mut receiving = truct.borrow_mut()?;
            for (key, value) in copying.clone().into_iter() {
                receiving.data_mut().insert(key.clone(), value);
            }
        }
        _ => todo!("Rune state should be a struct."),
    }

    Ok(value)
}

#[derive(Clone)]
pub struct PostEvaluation<T> {
    owner: Option<Owner>,
    value: T,
}

impl<T> PostEvaluation<T> {
    fn new(owner: Option<Owner>, value: T) -> Self {
        Self { owner, value }
    }

    fn into_inner(self) -> T {
        self.value
    }

    fn owner(&self) -> Result<Option<EntityPtr>> {
        let Some(owner) = &self.owner else {
            return Ok(None);
        };

        Ok(Some(
            get_my_session()?
                .entity(&LookupBy::Key(&EntityKey::new(&owner.key())))?
                .expect("Error getting owner"),
        ))
    }
}

impl<T> PostEvaluation<T>
where
    T: Simplifies,
{
    pub(super) fn flush(self) -> Result<T> {
        let Some(owner) = self.owner()? else {
            debug!("flush: ownerless");
            return Ok(self.value);
        };

        let mut behaviors = owner.scope_mut::<Behaviors>()?;
        let rune = behaviors.get_rune_mut()?;

        let mut save = false;
        if let Some(state) = self.value.simplify_state()? {
            info!("state:final {:?}", state.value);
            rune.state = state.value;
            save = true;
        }

        if save {
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
    Register,
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

    fn apply<S>(&self, state: Option<S>, json: TaggedJson) -> Result<Option<rune::runtime::Value>>
    where
        S: Clone + rune::ToValue,
    {
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

    pub fn schema(&self) -> Option<SchemaCollection> {
        let runners = self.0.borrow();
        runners.schema.clone()
    }

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
        let value = rune::runtime::to_value(v).with_context(|| here!())?;
        Ok(Self { value })
    }
}

pub trait Simplifies {
    fn simplify(&self) -> Result<Vec<Returned>>;

    fn simplify_state(&self) -> Result<Option<RuneState>> {
        Ok(self
            .simplify()
            .with_context(|| here!())?
            .into_iter()
            .flat_map(|f| match f {
                Returned::Tagged(_) => None,
                Returned::State(state) => Some(state),
            })
            .last())
    }
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
        self.value.simplify().with_context(|| here!())
    }
}

impl Simplifies for rune::runtime::Value {
    fn simplify(&self) -> Result<Vec<Returned>> {
        match self.clone() {
            rune::Value::Object(_object) => {
                let value = serde_json::to_value(self.clone())?;
                info!("object {:#?}", &value);

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
            rune::Value::Any(value) => {
                let value = value.borrow_ref().with_context(|| here!())?;
                if value.is::<RuneState>() {
                    if let Some(value) = value.downcast_borrow_ref::<RuneState>() {
                        Ok(vec![Returned::State(value.clone())])
                    } else {
                        Ok(vec![])
                    }
                } else {
                    Ok(vec![])
                }
            }
            rune::Value::EmptyTuple => Ok(vec![]),
            rune::Value::Struct(value) => {
                let value = value.borrow_ref()?;
                let data = value.data();
                let serialized = serde_json::to_value(ObjectSerializer {
                    object: data.clone(),
                })?;

                Ok(vec![Returned::State(RuneState {
                    value: Some(serialized),
                })])
            }
            rune::Value::Type(ty) => {
                warn!("Unexpected rune type: {:?}", ty);

                Ok(vec![])
            }
            _ => {
                warn!("Unexpected rune return: {:?}", self);

                Ok(vec![])
            }
        }
    }
}

struct ObjectSerializer {
    object: Object,
}

impl Serialize for ObjectSerializer {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_map(Some(self.object.len()))?;

        for (key, value) in &self.object {
            serializer.serialize_entry(key, value)?;
        }

        serializer.end()
    }
}
