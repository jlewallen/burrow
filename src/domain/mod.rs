mod internal;

pub mod build;
#[allow(clippy::module_inception)]
pub mod domain;
pub mod dynamic;
pub mod finding;
pub mod hooks;
pub mod perform;
pub mod session;

pub use build::*;
pub use domain::*;
pub use dynamic::*;
pub use finding::*;
pub use hooks::*;
pub use session::*;

pub mod notificiations {
    use crate::kernel::EntityKey;
    use anyhow::Result;
    use replies::Observed;
    use std::rc::Rc;

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
}

pub use notificiations::*;
