use anyhow::Result;
use std::rc::Rc;

use kernel::{EvaluationResult, ManagedHooks, ParsesActions, Plugin, PluginFactory};
use libloading::Library;
use tracing::{dispatcher::get_default, Subscriber};

use plugins_core::library::plugin::*;

#[derive(Default)]
pub struct DynamicPluginFactory {}

impl PluginFactory for DynamicPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::<DynamicPlugin>::default())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

struct DynamicRegistrar {
    #[allow(dead_code)]
    library: Rc<Library>,
}

impl DynamicRegistrar {
    fn new(library: Rc<Library>) -> Self {
        Self { library }
    }
}

impl PluginRegistrar for DynamicRegistrar {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync> {
        Box::new(PluginSubscriber::new())
    }
}

struct LoadedLibrary {
    library: Rc<Library>,
}

impl LoadedLibrary {
    fn register(&self) -> Result<()> {
        unsafe {
            let sym = self
                .library
                .get::<*mut PluginDeclaration>(b"plugin_declaration\0")?;
            let decl = sym.read();
            let mut registrar = DynamicRegistrar::new(Rc::clone(&self.library));

            info!("registering");

            (decl.register)(&mut registrar);
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct DynamicPlugin {
    libraries: Vec<LoadedLibrary>,
}

impl DynamicPlugin {
    fn load(&self, path: &str) -> Result<LoadedLibrary> {
        unsafe {
            let _span = span!(Level::INFO, "regdyn", lib = path).entered();

            info!("loading");

            let library = Rc::new(libloading::Library::new(path)?);

            Ok(LoadedLibrary { library })
        }
    }

    fn open_dynamic(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        let filename = libloading::library_filename("plugin_example_shared");
        let path = format!("target/debug/{}", filename.to_string_lossy());
        if std::fs::metadata(&path).is_ok() {
            let library = self.load(&path)?;
            library.register()?;

            self.libraries.push(library);
        }

        Ok(0)
    }
}

// pub static RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub static CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Copy, Clone)]
pub struct PluginDeclaration {
    // pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub register: unsafe extern "C" fn(&dyn PluginRegistrar),
}

pub trait PluginRegistrar {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync>;
}

#[macro_export]
macro_rules! export_plugin {
    ($register:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            core_version: $crate::CORE_VERSION,
            // rustc_version: $crate::RUSTC_VERSION,
            register: $register,
        };
    };
}

const KEY: &'static str = "dynamic";

impl Plugin for DynamicPlugin {
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
        match self.open_dynamic() {
            Ok(v) => trace!("{:?}", v),
            Err(e) => warn!("Error: {:?}", e),
        }

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for DynamicPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

struct PluginSubscriber {}

impl PluginSubscriber {
    fn new() -> Self {
        Self {}
    }
}

impl Subscriber for PluginSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        get_default(|d| d.enabled(metadata))
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        get_default(|d| d.new_span(span))
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        get_default(|d| d.record(span, values))
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        get_default(|d| d.record_follows_from(span, follows))
    }

    fn event(&self, event: &tracing::Event<'_>) {
        get_default(|d| d.event(event))
    }

    fn enter(&self, span: &span::Id) {
        get_default(|d| d.enter(span))
    }

    fn exit(&self, span: &span::Id) {
        get_default(|d| d.exit(span))
    }
}

pub mod model {
    use plugins_core::library::model::*;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DynamicReply {}

    impl Reply for DynamicReply {}

    impl ToJson for DynamicReply {
        fn to_json(&self) -> Result<Value, serde_json::Error> {
            serde_json::to_value(self)
        }
    }
}

pub mod actions {
    // use crate::{library::actions::*, looking::actions::LookAction};
}

pub mod parser {
    // use crate::library::parser::*;
}

#[cfg(test)]
mod tests {
    use plugins_core::library::tests::*;
    // use super::parser::*;
    use super::*;

    #[test]
    fn it_dynamic() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (_session, _surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        Ok(())
    }
}
