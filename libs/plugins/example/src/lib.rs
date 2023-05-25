use plugins_core::dynamic::PluginRegistrar;
use tracing::{dispatcher, info, Subscriber};

plugins_core::export_plugin!(initialize);

use dispatcher::Dispatch;

fn default_plugin_setup(subscriber: Box<dyn Subscriber + Send + Sync>) {
    let dispatch = Dispatch::new(subscriber);
    match dispatcher::set_global_default(dispatch) {
        Ok(_) => {}
        Err(_) => println!("Error configuring plugin tracing"),
    };
}

#[allow(improper_ctypes_definitions)] // TODO
extern "C" fn initialize(registrar: &dyn PluginRegistrar) {
    default_plugin_setup(registrar.tracing_subscriber());

    info!("hello, world!")
}
