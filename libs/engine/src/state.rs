use anyhow::Result;
use std::{cell::RefCell, rc::Rc};

use super::internal::Entities;
use kernel::*;

pub struct RaisedEvent {
    pub(crate) audience: Audience,
    pub(crate) event: Rc<dyn DomainEvent>,
}

#[derive(Default)]
pub struct State {
    pub(crate) entities: Rc<Entities>,
    pub(crate) raised: Rc<RefCell<Vec<RaisedEvent>>>,
    pub(crate) futures: Rc<RefCell<Vec<Scheduling>>>,
    pub(crate) destroyed: RefCell<Vec<EntityKey>>,
}

// TODO Move request_fn calls in StandardPerform to call this.
impl Performer for State {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        match perform {
            Perform::Living {
                living: _,
                action: _,
            } => todo!(),
            Perform::Chain(_) => todo!(),
            Perform::Raised(_) => todo!(),
            Perform::Schedule(_) => todo!(),
            _ => todo!(),
        }
    }
}
