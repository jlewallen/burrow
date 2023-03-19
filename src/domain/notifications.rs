use anyhow::Result;
use std::rc::Rc;

use replies::Observed;

use crate::kernel::EntityKey;

pub trait Notifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn Observed>) -> Result<()>;
}

#[derive(Default)]
pub struct DevNullNotifier {}

impl Notifier for DevNullNotifier {
    fn notify(&self, _audience: &EntityKey, _observed: &Rc<dyn Observed>) -> Result<()> {
        Ok(())
    }
}
