use std::rc::Rc;

use crate::library::actions::*;
use crate::looking::actions::*;
use crate::looking::model::Observe;
use crate::moving::model::Route;

use super::model::Occupyable;

#[action]
pub struct GoAction {
    pub item: Item,
}

impl GoAction {
    fn navigate(
        &self,
        session: SessionRef,
        actor: EntityPtr,
        area: EntityPtr,
        to_area: EntityPtr,
    ) -> ReplyResult {
        match tools::navigate_between(&area, &to_area, &actor)? {
            true => {
                let excluding = actor.key();
                let hearing_arrive: Vec<_> = tools::get_occupant_keys(&to_area)?
                    .into_iter()
                    .filter(|v| *v != excluding)
                    .collect();

                session.raise(
                    Some(actor.clone()),
                    Audience::Area(area.key().clone()),
                    Raising::TaggedJson(
                        Moving::Left {
                            actor: (&actor).observe(&actor)?.expect("No observed entity"),
                            area: (&area).observe(&actor)?.expect("No observed entity"),
                        }
                        .to_tagged_json()?,
                    ),
                )?;
                session.raise(
                    Some(actor.clone()),
                    Audience::Individuals(hearing_arrive),
                    Raising::TaggedJson(
                        Moving::Arrived {
                            actor: (&actor).observe(&actor)?.expect("No observed entity"),
                            area: (&to_area).observe(&actor)?.expect("No observed entity"),
                        }
                        .to_tagged_json()?,
                    ),
                )?;

                Ok(session.perform(Perform::Actor {
                    actor,
                    action: PerformAction::Instance(Rc::new(LookAction {})),
                })?)
            }
            false => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

impl Action for GoAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("go {:?}!", self.item);

        let (_, actor, area) = surroundings.unpack();

        if let Some(occupyable) = area.scope::<Occupyable>()? {
            match &self.item {
                Item::Route(route) => match occupyable.find_route(&route) {
                    Some(route) => match route {
                        Route::Simple(to_area) => {
                            let to_area = to_area.destination().to_entity()?;
                            self.navigate(session, actor, area, to_area)
                        }
                        Route::Deactivated(reason, _) => {
                            Ok(SimpleReply::Prevented(Some(reason.clone())).try_into()?)
                        }
                    },
                    None => {
                        match session.find_item(surroundings, &Item::Named(route.to_owned()))? {
                            Some(maybe) => {
                                if maybe.scope::<Occupyable>()?.is_some() {
                                    self.navigate(session, actor, area, maybe)
                                } else {
                                    Ok(SimpleReply::NotFound.try_into()?)
                                }
                            }
                            None => Ok(SimpleReply::NotFound.try_into()?),
                        }
                    }
                },
                Item::Gid(_) => match session.find_item(surroundings, &self.item)? {
                    Some(to_area) => self.navigate(session, actor, area, to_area),
                    None => Ok(SimpleReply::NotFound.try_into()?),
                },
                _ => panic!("Occupyable::find_route expecting Item::Route or Item::Gid"),
            }
        } else {
            Ok(SimpleReply::NotFound.try_into()?)
        }
    }
}

#[action]
pub struct ShowRoutesAction {}

impl Action for ShowRoutesAction {
    fn is_read_only(&self) -> bool {
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
    pub area: Item,
    pub name: String,
    pub destination: Item,
}

impl Action for AddRouteAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(&surroundings, &self.area)? {
            Some(area) => {
                let Some(destination) = session.find_item(surroundings, &self.destination)? else {
                    return Ok(SimpleReply::NotFound.try_into()?);
                };

                tools::add_route(&area, &self.name, &destination)?;

                Ok(SimpleReply::Done.try_into()?)
            }
            None => Ok(SimpleReply::Done.try_into()?),
        }
    }
}

#[action]
pub struct RemoveRouteAction {
    pub area: Item,
    pub name: String,
}

impl Action for RemoveRouteAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(&surroundings, &self.area)? {
            Some(area) => {
                let mut occupyable = area.scope_mut::<Occupyable>()?;
                if !occupyable.remove_route(&self.name) {
                    return Ok(SimpleReply::NotFound.try_into()?);
                }

                occupyable.save()?;

                Ok(SimpleReply::Done.try_into()?)
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct ActivateRouteAction {
    pub name: String,
}

impl Action for ActivateRouteAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, _, area) = surroundings.unpack();

        let mut occupyable = area.scope_mut::<Occupyable>()?;
        occupyable.activate(&self.name);
        occupyable.save()?;

        Ok(SimpleReply::Done.try_into()?)
    }
}

#[action]
pub struct DeactivateRouteAction {
    pub name: String,
    pub reason: String,
}

impl Action for DeactivateRouteAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, _, area) = surroundings.unpack();

        let mut occupyable = area.scope_mut::<Occupyable>()?;
        occupyable.deactivate(&self.name, &self.reason);
        occupyable.save()?;

        Ok(SimpleReply::Done.try_into()?)
    }
}
