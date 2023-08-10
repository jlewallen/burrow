use anyhow::Result;

use kernel::{EntityKey, TaggedJson};

pub trait Notifier {
    fn notify(&self, audience: &EntityKey, observed: &TaggedJson) -> Result<()>;
}

#[derive(Default)]
pub struct DevNullNotifier {}

impl Notifier for DevNullNotifier {
    fn notify(&self, _audience: &EntityKey, _observed: &TaggedJson) -> Result<()> {
        Ok(())
    }
}
