use dispatcher::Dispatch;
use plugins_dynlib::PluginRegistrar;
use tracing::{dispatcher, info};

plugins_dynlib::export_plugin!(initialize);

fn default_plugin_setup(registrar: &dyn PluginRegistrar) {
    if !dispatcher::has_been_set() {
        let subscriber = registrar.tracing_subscriber();
        let dispatch = Dispatch::new(subscriber);
        match dispatcher::set_global_default(dispatch) {
            Ok(_) => {}
            Err(_) => println!("Error configuring plugin tracing"),
        };
    }
}

#[allow(improper_ctypes_definitions)] // TODO
extern "C" fn initialize(registrar: &dyn PluginRegistrar) {
    default_plugin_setup(registrar);

    info!("hello, world!")
}
