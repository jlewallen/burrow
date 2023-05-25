use plugins_core::dynamic::PluginRegistrar;
use tracing::info;

plugins_core::export_plugin!(initialize);

#[allow(improper_ctypes_definitions)] // TODO
extern "C" fn initialize(_registrar: &mut dyn PluginRegistrar) {
    println!("hello, world?");
    info!("hello, world!")
}
