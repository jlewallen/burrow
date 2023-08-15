use crate::library::actions::*;
use crate::looking::actions::*;
use crate::looking::model::Observe;
use crate::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

use super::model::Occupyable;

#[action]
pub struct GoAction {
    pub item: Item,
}

impl GoAction {
    fn navigate(
        &self,
        session: SessionRef,
        living: EntityPtr,
        area: EntityPtr,
        to_area: EntityPtr,
        surroundings: &Surroundings,
    ) -> ReplyResult {
        let can = session
            .hooks()
            .invoke::<MovingHooks, CanMove, _>(|h| h.before_moving(surroundings, &to_area))?;

        match can {
            CanMove::Allow => match tools::navigate_between(&area, &to_area, &living)? {
                DomainOutcome::Ok => {
                    session
                        .hooks()
                        .invoke::<MovingHooks, (), _>(|h| h.after_move(surroundings, &area))?;

                    let excluding = living.key();
                    let hearing_arrive: Vec<_> = tools::get_occupant_keys(&to_area)?
                        .into_iter()
                        .filter(|v| *v != excluding)
                        .collect();

                    session.raise(
                        Audience::Area(area.key().clone()),
                        Raising::TaggedJson(
                            MovingEvent::Left {
                                living: (&living).observe(&living)?.expect("No observed entity"),
                                area: (&area).observe(&living)?.expect("No observed entity"),
                            }
                            .to_tagged_json()?,
                        ),
                    )?;
                    session.raise(
                        Audience::Individuals(hearing_arrive),
                        Raising::TaggedJson(
                            MovingEvent::Arrived {
                                living: (&living).observe(&living)?.expect("No observed entity"),
                                area: (&to_area).observe(&living)?.expect("No observed entity"),
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
}

impl Action for GoAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("go {:?}!", self.item);

        let (_, living, area) = surroundings.unpack();

        if let Some(occupyable) = area.scope::<Occupyable>()? {
            if let Some(to_area) = occupyable.find_route(&self.item)? {
                return self.navigate(session, living, area, to_area, surroundings);
            }
        }

        match session.find_item(surroundings, &self.item)? {
            Some(to_area) => self.navigate(session, living, area, to_area, surroundings),
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct ShowRoutesAction {}

impl Action for ShowRoutesAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, _, area) = surroundings.unpack();
        let Some(occupyable) = area.scope::<Occupyable>()? else {
            return Ok(SimpleReply::NotFound.try_into()?);
        };
        let Some(routes) = &occupyable.routes else {
            return Ok(SimpleReply::NotFound.try_into()?);
        };

        let reply = TaggedJson::new("routes".to_owned(), serde_json::to_value(routes)?.into());

        Ok(Effect::Reply(EffectReply::TaggedJson(reply)))
    }
}

#[action]
pub struct AddRouteAction {
    pub name: String,
    pub destination: Item,
}

impl Action for AddRouteAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, _living, area) = surroundings.unpack();

        let Some(destination) = session.find_item(surroundings, &self.destination)? else {
             return Ok(SimpleReply::NotFound.try_into()?);
        };

        tools::add_route(&area, &self.name, &destination)?;

        Ok(SimpleReply::Done.try_into()?)
    }
}

#[action]
pub struct RemoveRouteAction {
    pub name: String,
}

impl Action for RemoveRouteAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, _living, area) = surroundings.unpack();
        let mut occupyable = area.scope_mut::<Occupyable>()?;
        if !occupyable.remove_route(&self.name)? {
            return Ok(SimpleReply::NotFound.try_into()?);
        }

        occupyable.save()?;

        Ok(SimpleReply::Done.try_into()?)
    }
}
