use anyhow::Result;
use std::{cell::RefCell, rc::Rc};
use tracing::*;

use kernel::*;

use super::internal::Entities;

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

impl State {
    pub(crate) fn queue_raised(&self, raised: Raised) -> Result<()> {
        info!("{:?}", raised);

        self.raised.borrow_mut().push(RaisedEvent {
            audience: raised.audience,
            event: raised.event,
        });

        Ok(())
    }

    pub(crate) fn queue_scheduled(&self, scheduling: Scheduling) -> Result<()> {
        info!("{:?}", scheduling);

        let mut futures = self.futures.borrow_mut();

        futures.push(scheduling);

        Ok(())
    }
}

impl Performer for State {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        match perform {
            Perform::Surroundings {
                surroundings,
                action,
            } => {
                let _span = span!(Level::DEBUG, "A").entered();
                info!("action:perform {:?}", &action);
                let res = action.perform(get_my_session()?, &surroundings);
                if let Ok(effect) = &res {
                    trace!("action:effect {:?}", effect);
                    info!("action:effect");
                } else {
                    warn!("action:error {:?}", res);
                }
                res
            }
            Perform::Raised(raised) => {
                self.queue_raised(raised)?;

                Ok(Effect::Ok)
            }
            Perform::Schedule(scheduling) => {
                self.queue_scheduled(scheduling)?;

                Ok(Effect::Ok)
            }
            _ => todo!(),
        }
    }
}

#[allow(dead_code)]
pub struct ActionPerformer {
    session: SessionRef,
    surroundings: Surroundings,
    // action: Rc<dyn Action>,
}

#[allow(unused_variables)]
impl Performer for ActionPerformer {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        match perform {
            Perform::Living { living, action } => todo!(),
            Perform::Surroundings {
                surroundings,
                action,
            } => todo!(),
            Perform::Chain(_) => todo!(),
            Perform::Delivery(_) => todo!(),
            Perform::Raised(_) => todo!(),
            Perform::Schedule(_) => todo!(),
            Perform::Ping(_) => todo!(),
            _ => todo!(),
        }
    }
}
