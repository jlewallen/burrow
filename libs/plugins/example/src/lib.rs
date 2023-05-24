use plugins_core::dynamic::{Function, InvocationError, PluginRegistrar};

plugins_core::export_plugin!(initialize);

extern "C" fn initialize(_registrar: &mut dyn PluginRegistrar) {
    // registrar.register_function("random", Box::new(Random));
}
