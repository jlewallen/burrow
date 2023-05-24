use kernel::{EvaluationResult, ManagedHooks, ParsesActions, Plugin};

use crate::library::plugin::*;

#[derive(Default)]
pub struct DynamicPluginFactory {}

impl PluginFactory for DynamicPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(DynamicPlugin {}))
    }
}

#[derive(Default)]
pub struct DynamicPlugin {}

impl DynamicPlugin {
    fn call_dynamic(&self, name: &str) -> Result<u32, Box<dyn std::error::Error>> {
        unsafe {
            let lib = libloading::Library::new("target/debug/libplugins_example.dylib")?;
            let func: libloading::Symbol<unsafe extern "C" fn() -> u32> =
                lib.get(name.as_bytes())?;
            Ok(func())
        }
    }
}

pub static CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
// pub static RUSTC_VERSION: &str = env!("RUSTC_VERSION");

#[derive(Copy, Clone)]
pub struct PluginDeclaration {
    // pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub register: unsafe extern "C" fn(&mut dyn PluginRegistrar),
}

pub trait PluginRegistrar {
    fn register_function(&mut self, name: &str, function: Box<dyn Function>);
}

pub trait Function {
    fn call(&self, args: &[f64]) -> Result<f64, InvocationError>;

    /// Help text that may be used to display information about this function.
    fn help(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InvocationError {
    InvalidArgumentCount { expected: usize, found: usize },
    Other { msg: String },
}

#[macro_export]
macro_rules! export_plugin {
    ($register:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static plugin_declaration: $crate::dynamic::PluginDeclaration =
            $crate::dynamic::PluginDeclaration {
                core_version: $crate::dynamic::CORE_VERSION,
                // rustc_version: $crate::RUSTC_VERSION,
                register: $register,
            };
    };
}

impl Plugin for DynamicPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "dynamic"
    }

    fn initialize(&mut self) -> Result<()> {
        match self.call_dynamic("initialize") {
            Ok(v) => info!("{:?}", v),
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
}

impl ParsesActions for DynamicPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

pub mod model {
    use crate::library::model::*;

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
    use crate::library::tests::*;
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
