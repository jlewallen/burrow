use crate::library::actions::*;
use crate::looking::actions::*;
use crate::looking::model::Observe;
use crate::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

#[action]
pub struct GoAction {
    pub item: Item,
}

impl Action for GoAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("go {:?}!", self.item);

        let (_, living, area) = surroundings.unpack();

        match session.find_item(surroundings, &self.item)? {
            Some(to_area) => {
                let can = session.hooks().invoke::<MovingHooks, CanMove, _>(|h| {
                    h.before_moving(surroundings, &to_area)
                })?;

                match can {
                    CanMove::Allow => match tools::navigate_between(&area, &to_area, &living)? {
                        DomainOutcome::Ok => {
                            session.hooks().invoke::<MovingHooks, (), _>(|h| {
                                h.after_move(surroundings, &area)
                            })?;

                            let excluding = living.key();
                            let hearing_arrive: Vec<_> = tools::get_occupant_keys(&to_area)?
                                .into_iter()
                                .filter(|v| *v != excluding)
                                .collect();

                            session.raise(
                                Audience::Area(area.key().clone()),
                                Raising::TaggedJson(
                                    MovingEvent::Left {
                                        living: (&living)
                                            .observe(&living)?
                                            .expect("No observed entity"),
                                        area: (&area)
                                            .observe(&living)?
                                            .expect("No observed entity"),
                                    }
                                    .to_tagged_json()?,
                                ),
                            )?;
                            session.raise(
                                Audience::Individuals(hearing_arrive),
                                Raising::TaggedJson(
                                    MovingEvent::Arrived {
                                        living: (&living)
                                            .observe(&living)?
                                            .expect("No observed entity"),
                                        area: (&to_area)
                                            .observe(&living)?
                                            .expect("No observed entity"),
                                    }
                                    .to_tagged_json()?,
                                ),
                            )?;

                            session.perform(Perform::Living {
                                living,
                                action: PerformAction::Instance(Rc::new(LookAction {})),
                            })
                        }
                        DomainOutcome::Nope => Ok(SimpleReply::NotFound.try_into()?),
                    },
                    CanMove::Prevent => Ok(SimpleReply::Prevented.try_into()?),
                }
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct RouteAction {}

impl Action for RouteAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
        todo!()
    }
}
