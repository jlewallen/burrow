use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf};

use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Instance, Memory, Module, Store, Value, WasmPtr,
};

use plugins_core::library::plugin::*;
use plugins_rpc::SessionServices;
use plugins_rpc_proto::{Payload, Sender, ServerProtocol, Services, DEFAULT_DEPTH};
use wasm_sys::ipc::WasmMessage;

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
        let messages: Vec<WasmMessage> = messages
            .into_iter()
            .map(|b| Ok(WasmMessage::from_bytes(&b)?))
            .collect::<Result<Vec<_>>>()?;

        let services = SessionServices::new_for_my_session()?;
        for message in messages.into_iter() {
            info!("(server) {:?}", message);

            match message {
                WasmMessage::Query(q) => {
                    let mut sender: Sender<Payload> = Default::default();
                    let mut server = ServerProtocol::new();
                    server.apply(&q, &mut sender, &services)?;

                    for payload in sender.into_iter() {
                        trace!("(to-agent) {:?}", &payload);
                        self.send(WasmMessage::Payload(payload).to_bytes()?)?;
                    }
                }
                _ => unimplemented!("Wasm server received {:?}", message),
            }
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
        self.send(WasmMessage::Payload(plugins_rpc_proto::Payload::Initialize).to_bytes()?)?;
        self.call_agent(&AgentCall::Initialize)
    }

    fn have_surroundings(&mut self, surroundings: plugins_rpc_proto::Surroundings) -> Result<()> {
        let services = SessionServices::new_for_my_session()?;
        let keys = match &surroundings {
            plugins_rpc_proto::Surroundings::Living {
                world,
                living,
                area,
            } => vec![world.clone(), living.clone(), area.clone()],
        };
        let lookups: Vec<_> = keys
            .into_iter()
            .map(|k| plugins_rpc_proto::LookupBy::Key(k))
            .collect();
        let resolved = services.lookup(DEFAULT_DEPTH, &lookups)?;
        for resolved in resolved {
            self.send(WasmMessage::Payload(Payload::Resolved(vec![resolved])).to_bytes()?)?;
        }

        self.send(WasmMessage::Payload(Payload::Surroundings(surroundings)).to_bytes()?)?;
        self.tick()
    }
}

#[derive(Default)]
pub struct WasmPluginFactory {}

impl PluginFactory for WasmPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::<WasmPlugin>::default())
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

pub type Runners = Arc<RefCell<Vec<WasmRunner>>>;

#[derive(Default)]
pub struct WasmPlugin {
    runners: Runners,
}

fn create_runners() -> Result<Vec<WasmRunner>> {
    let cwd = std::env::current_dir()?;
    let path = get_assets_path()?.join("plugin_example_wasm.wasm");
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

    let mut env_mut = env.as_mut(&mut store);
    env_mut.memory = Some(instance.exports.get_memory("memory")?.clone());

    Ok(vec![WasmRunner::new(store, env, instance)])
}

fn get_assets_path() -> Result<PathBuf> {
    let mut cwd = std::env::current_dir()?;
    loop {
        if cwd.join(".git").exists() {
            break;
        }

        cwd = match cwd.parent() {
            Some(cwd) => cwd.to_path_buf(),
            None => {
                return Err(anyhow::anyhow!("Error locating assets path"));
            }
        };
    }

    Ok(cwd.join("plugins/wasm/assets"))
}

impl WasmPlugin {}

const KEY: &'static str = "wasm";

impl Plugin for WasmPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        KEY
    }

    fn key(&self) -> &'static str {
        KEY
    }

    fn initialize(&mut self) -> Result<()> {
        let mut runners = self.runners.borrow_mut();
        runners.extend(create_runners()?);
        for runner in runners.iter_mut() {
            runner.initialize()?;
        }

        Ok(())
    }

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()> {
        hooks::register(hooks, &self.runners)
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let mut runners = self.runners.borrow_mut();
        for runner in runners.iter_mut() {
            runner.have_surroundings(surroundings.try_into()?)?;
        }

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
