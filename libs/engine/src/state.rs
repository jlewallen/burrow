use anyhow::Result;
use std::{cell::RefCell, rc::Rc};
use tracing::*;

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
            Perform::Living {
                living: _,
                action: _,
            } => {
                todo!()
            }
            Perform::Chain(_action) => {
                /*
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
                */
                todo!()
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
