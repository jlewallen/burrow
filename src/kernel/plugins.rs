pub trait Plugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized;
}

pub trait PluginHooks {}
