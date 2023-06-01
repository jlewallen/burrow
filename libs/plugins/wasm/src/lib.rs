use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use plugins_core::library::plugin::*;

// Not super happy about Clone here, this is so we can store them mapped to
// RuneRunners and makes building that hash easier. Maybe, move to generating a
// key from this and using that.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ScriptSource {
    File(PathBuf),
    Entity(EntityKey, String),
}

#[allow(dead_code)]
pub struct WasmRunner {}

impl WasmRunner {
    pub fn new(_sources: HashSet<ScriptSource>) -> Self {
        Self {}
    }
}

#[derive(Default)]
pub struct WasmPluginFactory {}

impl PluginFactory for WasmPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::<WasmPlugin>::default())
    }
}

pub type Runners = Arc<RefCell<HashMap<ScriptSource, WasmRunner>>>;

#[derive(Default)]
pub struct WasmPlugin {
    runners: Runners,
}

impl WasmPlugin {
    fn add_runners_for(&self, sources: impl Iterator<Item = ScriptSource>) -> Result<()> {
        let mut runners = self.runners.borrow_mut();
        for source in sources {
            if !runners.contains_key(&source) {
                runners.insert(source.clone(), self.create_runner(source)?);
            }
        }

        Ok(())
    }

    fn create_runner(&self, source: ScriptSource) -> Result<WasmRunner> {
        Ok(WasmRunner::new(HashSet::from([source])))
    }
}

impl Plugin for WasmPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "wasm"
    }

    fn initialize(&mut self) -> Result<()> {
        self.add_runners_for(vec![].into_iter())?;

        for (_, _runner) in self.runners.borrow_mut().iter_mut() {
            // runner.user()?;
        }

        Ok(())
    }

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()> {
        hooks::register(hooks, &self.runners)
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
        // self.add_runners_for(sources::load_sources_from_surroundings(surroundings)?.into_iter())?;

        for (_, _runner) in self.runners.borrow_mut().iter_mut() {
            // runner.have_surroundings(surroundings)?;
        }

        Ok(())
    }
}

mod hooks {
    use super::*;
    use plugins_core::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

    pub fn register(hooks: &ManagedHooks, runners: &Runners) -> Result<()> {
        hooks.with::<MovingHooks, _>(|h| {
            let rune_moving_hooks = Box::new(RuneMovingHooks {
                _runners: Runners::clone(runners),
            });
            h.before_moving.register(rune_moving_hooks.clone());
            h.after_move.register(rune_moving_hooks);
            Ok(())
        })
    }

    #[derive(Clone)]
    struct RuneMovingHooks {
        _runners: Runners,
    }

    impl BeforeMovingHook for RuneMovingHooks {
        fn before_moving(&self, _surroundings: &Surroundings, _to_area: &Entry) -> Result<CanMove> {
            Ok(CanMove::Allow)
        }
    }

    impl AfterMoveHook for RuneMovingHooks {
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

pub mod actions {
    // use kernel::*;
}

mod parser {
    // use kernel::*;
    // use plugins_core::library::parser::*;
}
