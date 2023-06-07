use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf};

use anyhow::Context;
use plugins_core::library::plugin::*;

use wasmer::{
    imports, Function, FunctionEnv, FunctionEnvMut, Instance, Memory, Module, Store, WasmPtr,
};

#[derive(Default)]
pub struct WasmRunner {}

impl WasmRunner {
    pub fn new() -> Self {
        Self::default()
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

pub type Runners = Arc<RefCell<Vec<WasmRunner>>>;

#[derive(Default)]
pub struct WasmPlugin {
    runners: Runners,
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

    Ok(cwd.join("libs/plugins/wasm/assets"))
}

#[derive(Default)]
struct MyEnv {
    pub memory: Option<Memory>,
    pub inbox: VecDeque<Arc<[u8]>>,
    pub outbox: Vec<Box<[u8]>>,
}

impl WasmPlugin {}

impl Plugin for WasmPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "wasm"
    }

    fn initialize(&mut self) -> Result<()> {
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
        let env = FunctionEnv::new(&mut store, MyEnv::default());

        fn agent_send(mut env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) {
            let (data, store) = env.data_and_store_mut();
            let view = data.memory.as_ref().expect("Memory error").view(&store);
            let bytes = msg.slice(&view, len).expect("Slice error");
            let bytes = bytes.read_to_vec().expect("Read message error").into();

            data.outbox.push(bytes);
        }

        fn agent_recv(mut env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) -> u32 {
            let (data, store) = env.data_and_store_mut();
            let view = data.memory.as_ref().expect("Memory error").view(&store);

            let Some(sending) = data.inbox.pop_front() else { return 0 };

            assert!(sending.len() < len as usize);

            let values = msg.slice(&view, sending.len() as u32).expect("Slice error");
            for i in 0..sending.len() {
                values
                    .index(i as u64)
                    .write(sending[i])
                    .expect("Write error");
            }

            sending.len() as u32
        }

        fn get_string(mut env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) -> String {
            let (data, store) = env.data_and_store_mut();
            let view = data.memory.as_ref().expect("No memory").view(&store);
            msg.read_utf8_string(&view, len).expect("No string")
        }

        fn console_info(env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) {
            info!("{}", &get_string(env, msg, len));
        }

        fn console_warn(env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) {
            warn!("{}", &get_string(env, msg, len));
        }

        fn console_error(env: FunctionEnvMut<MyEnv>, msg: WasmPtr<u8>, len: u32) {
            error!("{}", &get_string(env, msg, len));
        }

        let imports = imports! {
            "burrow" => {
                "console_info" => Function::new_typed_with_env(&mut store, &env, console_info),
                "console_warn" => Function::new_typed_with_env(&mut store, &env, console_warn),
                "console_error" => Function::new_typed_with_env(&mut store, &env, console_error),
                "agent_send" => Function::new_typed_with_env(&mut store, &env, agent_send),
                "agent_recv" => Function::new_typed_with_env(&mut store, &env, agent_recv),
            }
        };

        let module = Module::new(&store, wasm_bytes)?;
        let instance = Instance::new(&mut store, &module, &imports)?;
        info!("instance {:?}", instance);

        {
            let mut env_mut = env.as_mut(&mut store);
            env_mut.memory = Some(instance.exports.get_memory("memory")?.clone());
        }

        let agent_initialize = instance.exports.get_function("agent_initialize")?;

        agent_initialize.call(&mut store, &[])?;

        info!("done");

        Ok(())
    }

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()> {
        hooks::register(hooks, &self.runners)
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
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
