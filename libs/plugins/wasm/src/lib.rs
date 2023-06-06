use anyhow::Result;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf};

use anyhow::Context;
use plugins_core::library::plugin::*;

use wasmer::{imports, Instance, Module, Store};

// wai_bindgen_rust::import!("../examples/wasm/agent.wai");
// use wai_bindgen_wasmer::wasmer::{imports, Instance, Module, Store};
// wai_bindgen_wasmer::export!("../examples/wasm/agent.wai");

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
        let imports = imports! {};
        let module = Module::new(&store, wasm_bytes)?;
        // let agent = agent::Agent::instantiate(&mut store, &module, &mut imports).with_context(|| anyhow!("Instantiate"))?;

        let instance = Instance::new(&mut store, &module, &imports)?;
        info!("instance {:?}", instance);

        // agent.0.hello(&mut store, None)?;

        info!("done");

        /*
        let add_one = instance
            .exports
            .get_function("add_one")
            .with_context(|| anyhow!("Get `add_one`"))?;
        let result = add_one.call(&mut store, &[Value::I32(42)])?;
        assert_eq!(result[0], Value::I32(43));
        */

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
