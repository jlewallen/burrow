use anyhow::Result;
use std::rc::Rc;

use kernel::{DomainEvent, EntityKey};

pub trait Notifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn DomainEvent>) -> Result<()>;
}

#[derive(Default)]
pub struct DevNullNotifier {}

impl Notifier for DevNullNotifier {
    fn notify(&self, _audience: &EntityKey, _observed: &Rc<dyn DomainEvent>) -> Result<()> {
        Ok(())
    }
}
