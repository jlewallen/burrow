use anyhow::{anyhow, Context, Result};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Instance, Memory, Module, Store, Value, WasmPtr,
};

use plugins_core::library::plugin::*;
use plugins_rpc::{have_surroundings, Querying, SessionServices};
use wasm_sys::prelude::{Payload, Query, WasmMessage};

pub struct WasmRunner {
    store: Store,
    env: FunctionEnv<AgentEnv>,
    instance: Instance,
}

enum AgentCall {
    Initialize,
    Tick,
}

impl WasmRunner {
    fn new(store: Store, env: FunctionEnv<AgentEnv>, instance: Instance) -> Self {
        Self {
            store,
            env,
            instance,
        }
    }

    fn is_inbox_empty(&self) -> bool {
        let env = self.env.as_ref(&self.store);
        env.inbox.is_empty()
    }

    fn send<T>(&mut self, message: T) -> Result<()>
    where
        T: Into<Arc<[u8]>>,
    {
        let env_mut = self.env.as_mut(&mut self.store);
        env_mut.inbox.push_back(message.into());
        Ok(())
    }

    fn call_agent(&mut self, call: &AgentCall) -> Result<()> {
        match call {
            AgentCall::Initialize => {
                self.instance
                    .exports
                    .get_function("agent_initialize")?
                    .call(&mut self.store, &[])?;
            }
            AgentCall::Tick => self.call_agent_tick()?,
        }

        let outbox = std::mem::take(&mut self.env.as_mut(&mut self.store).outbox);

        self.process_queries(outbox)
    }

    fn process_queries(&mut self, messages: Vec<Box<[u8]>>) -> Result<()> {
        let services = SessionServices::new_for_my_session(None)?;
        let messages: Vec<Query> = messages
            .into_iter()
            .map(|b| Ok(WasmMessage::from_bytes(&b)?))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|m| match m {
                WasmMessage::Query(query) => query,
                WasmMessage::Payload(_) => unimplemented!(),
            })
            .collect();

        let querying = Querying::new();
        for payload in querying.process(messages, &services)? {
            self.send(WasmMessage::Payload(payload).to_bytes()?)?;
        }

        Ok(())
    }

    fn call_agent_tick(&mut self) -> Result<()> {
        let state = {
            let env = self.env.as_ref(&mut self.store);
            env.state.ok_or_else(|| anyhow!("Module missing state."))?
        };

        self.instance
            .exports
            .get_function("agent_tick")?
            .call(&mut self.store, &[Value::I32(state)])?;

        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        while !self.is_inbox_empty() {
            self.call_agent(&AgentCall::Tick)?;
        }

        Ok(())
    }

    fn initialize(&mut self) -> Result<()> {
        self.send(WasmMessage::Payload(Payload::Initialize).to_bytes()?)?;
        self.call_agent(&AgentCall::Initialize)
    }

    fn have_surroundings(&mut self, surroundings: &kernel::Surroundings) -> Result<()> {
        let services = SessionServices::new_for_my_session(None)?;
        let messages: Vec<Vec<u8>> = have_surroundings(surroundings, &services)?
            .into_iter()
            .map(|m| Ok(WasmMessage::Payload(m).to_bytes()?))
            .collect::<Result<Vec<_>>>()?;

        for message in messages.into_iter() {
            self.send(message)?;
        }
        self.tick()
    }
}

pub type Runners = Arc<Mutex<RefCell<Vec<WasmRunner>>>>;

pub struct WasmPluginFactory {
    runners: Runners,
}

impl WasmPluginFactory {
    pub fn new(path: &Path) -> Result<Self> {
        let runners = Arc::new(Mutex::new(RefCell::new(create_runners(path)?)));

        Ok(Self { runners })
    }
}

impl PluginFactory for WasmPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(WasmPlugin::new(Arc::clone(&self.runners))))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct AgentEnv {
    pub memory: Option<Memory>,
    pub inbox: VecDeque<Arc<[u8]>>,
    pub outbox: Vec<Box<[u8]>>,
    pub state: Option<i32>,
}

#[derive(Default)]
pub struct WasmPlugin {
    runners: Runners,
}

fn create_runners(path: &Path) -> Result<Vec<WasmRunner>> {
    let cwd = std::env::current_dir()?;
    let path = path.join("plugin_example_wasm.wasm");
    if std::fs::metadata(&path).is_err() {
        info!("no wasm in {:?}", path);
        return Ok(vec![]);
    }

    let wasm_bytes = std::fs::read(&path).with_context(|| {
        format!(
            "Opening wasm: {} (from {})",
            &path.display(),
            &cwd.display()
        )
    })?;

    let mut store = Store::default();
    let env = FunctionEnv::new(&mut store, AgentEnv::default());

    fn agent_send(mut env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        let (data, store) = env.data_and_store_mut();
        let view = data.memory.as_ref().expect("Memory error").view(&store);
        let bytes = msg.slice(&view, len).expect("Slice error");
        let bytes = bytes.read_to_vec().expect("Read message error").into();

        data.outbox.push(bytes);
    }

    fn agent_recv(mut env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) -> u32 {
        let (data, store) = env.data_and_store_mut();
        let view = data.memory.as_ref().expect("Memory error").view(&store);

        let Some(sending) = data.inbox.pop_front() else { return 0 };

        if sending.len() as u32 > len {
            return sending.len() as u32;
        }

        let values = msg.slice(&view, sending.len() as u32).expect("Slice error");
        for i in 0..sending.len() {
            values
                .index(i as u64)
                .write(sending[i])
                .expect("Write error");
        }

        sending.len() as u32
    }

    fn agent_store(mut env: FunctionEnvMut<AgentEnv>, ptr: i32) {
        let (data, _store) = env.data_and_store_mut();

        data.state = Some(ptr);
    }

    fn get_string(mut env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) -> String {
        let (data, store) = env.data_and_store_mut();
        let view = data.memory.as_ref().expect("No memory").view(&store);
        msg.read_utf8_string(&view, len).expect("No string")
    }

    fn console_info(env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        info!("(agent) {}", &get_string(env, msg, len));
    }

    fn console_debug(env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        debug!("(agent) {}", &get_string(env, msg, len));
    }

    fn console_trace(env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        trace!("(agent) {}", &get_string(env, msg, len));
    }

    fn console_warn(env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        warn!("(agent) {}", &get_string(env, msg, len));
    }

    fn console_error(env: FunctionEnvMut<AgentEnv>, msg: WasmPtr<u8>, len: u32) {
        error!("(agent) {}", &get_string(env, msg, len));
    }

    let imports = imports! {
        "burrow" => {
            "console_info" => Function::new_typed_with_env(&mut store, &env, console_info),
            "console_debug" => Function::new_typed_with_env(&mut store, &env, console_debug),
            "console_trace" => Function::new_typed_with_env(&mut store, &env, console_trace),
            "console_warn" => Function::new_typed_with_env(&mut store, &env, console_warn),
            "console_error" => Function::new_typed_with_env(&mut store, &env, console_error),
            "agent_store" => Function::new_typed_with_env(&mut store, &env, agent_store),
            "agent_send" => Function::new_typed_with_env(&mut store, &env, agent_send),
            "agent_recv" => Function::new_typed_with_env(&mut store, &env, agent_recv),
        }
    };

    let module = Module::new(&store, wasm_bytes)?;
    info!("module:ready");

    let instance = Instance::new(&mut store, &module, &imports)?;
    trace!("instance {:?}", instance);

    let env_mut = env.as_mut(&mut store);
    env_mut.memory = Some(instance.exports.get_memory("memory")?.clone());

    Ok(vec![WasmRunner::new(store, env, instance)])
}

impl WasmPlugin {
    fn new(runners: Runners) -> Self {
        Self { runners }
    }
}

impl Plugin for WasmPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "wasm"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        let locked = self.runners.lock().map_err(|_| anyhow!("Lock error"))?;
        let mut runners = locked.borrow_mut();
        for runner in runners.iter_mut() {
            runner.initialize()?;
        }

        Ok(())
    }

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()> {
        hooks::register(hooks, &self.runners)
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let locked = self.runners.lock().map_err(|_| anyhow!("Lock error"))?;
        let mut runners = locked.borrow_mut();
        for runner in runners.iter_mut() {
            runner.have_surroundings(surroundings)?;
        }

        Ok(())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

mod hooks {
    use super::*;
    use plugins_core::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

    pub fn register(hooks: &ManagedHooks, runners: &Runners) -> Result<()> {
        hooks.with::<MovingHooks, _>(|h| {
            let rune_moving_hooks = Box::new(WasmMovingHooks {
                _runners: Runners::clone(runners),
            });
            h.before_moving.register(rune_moving_hooks.clone());
            h.after_move.register(rune_moving_hooks);
            Ok(())
        })
    }

    #[derive(Clone)]
    struct WasmMovingHooks {
        _runners: Runners,
    }

    impl BeforeMovingHook for WasmMovingHooks {
        fn before_moving(&self, _surroundings: &Surroundings, _to_area: &Entry) -> Result<CanMove> {
            Ok(CanMove::Allow)
        }
    }

    impl AfterMoveHook for WasmMovingHooks {
        fn after_move(&self, _surroundings: &Surroundings, _from_area: &Entry) -> Result<()> {
            Ok(())
        }
    }
}

impl ParsesActions for WasmPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

impl Evaluator for WasmPlugin {
    fn evaluate(&self, perform: &dyn Performer, consider: Evaluation) -> Result<Option<Effect>> {
        self.evaluate_parsed_action(perform, consider)
    }
}
