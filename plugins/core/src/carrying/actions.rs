use crate::{carrying::model::Carrying, library::actions::*, looking::model::Observe};

#[action]
pub struct HoldAction {
    pub item: Item,
}

impl Action for HoldAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("hold {:?}!", self.item);

        let (_, actor, area) = surroundings.unpack();

        match session.find_item(surroundings, &self.item)? {
            Some(holding) => match tools::move_between(&area, &actor, holding.clone())? {
                true => Ok(reply_ok(
                    actor.clone(),
                    Audience::Area(area.key().clone()),
                    Carrying::Held {
                        actor: (&actor).observe(&actor)?.expect("No observed entity"),
                        item: (&holding.entity()?)
                            .observe(&actor)?
                            .expect("No observed entity"),
                        area: (&area).observe(&actor)?.expect("No observed entity"),
                    },
                )?),
                false => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct DropAction {
    pub maybe_item: Option<Item>,
}

impl Action for DropAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("drop {:?}!", self.maybe_item);

        let (_, actor, area) = surroundings.unpack();

        match &self.maybe_item {
            Some(item) => match session.find_item(surroundings, item)? {
                Some(dropping) => match tools::move_between(&actor, &area, dropping.clone())? {
                    true => Ok(reply_ok(
                        actor.clone(),
                        Audience::Area(area.key().clone()),
                        Carrying::Dropped {
                            actor: (&actor).observe(&actor)?.expect("No observed entity"),
                            item: (&dropping.entity()?)
                                .observe(&actor)?
                                .expect("No observed entity"),
                            area: (&area).observe(&actor)?.expect("No observed entity"),
                        },
                    )?),
                    false => Ok(SimpleReply::NotFound.try_into()?),
                },
                None => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct PutInsideAction {
    pub item: Item,
    pub vessel: Item,
}

impl Action for PutInsideAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("put-inside {:?} -> {:?}", self.item, self.vessel);

        let (_, _user, _area) = surroundings.unpack();

        match session.find_item(surroundings, &self.item)? {
            Some(item) => match session.find_item(surroundings, &self.vessel)? {
                Some(vessel) => {
                    let vessel = vessel.one()?;
                    if tools::is_container(&vessel)? {
                        let from = tools::container_of(&item.clone().one()?)?;
                        match tools::move_between(&from, &vessel, item)? {
                            true => Ok(SimpleReply::Done.try_into()?),
                            false => Ok(SimpleReply::NotFound.try_into()?),
                        }
                    } else {
                        Ok(SimpleReply::Impossible.try_into()?)
                    }
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct TakeOutAction {
    pub item: Item,
    pub vessel: Item,
}

impl Action for TakeOutAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("take-out {:?} -> {:?}", self.item, self.vessel);

        let (_, user, _area) = surroundings.unpack();

        match session.find_item(surroundings, &self.vessel)? {
            Some(vessel) => {
                let vessel = vessel.one()?;
                if tools::is_container(&vessel)? {
                    match session.find_item(surroundings, &self.item)? {
                        Some(item) => match tools::move_between(&vessel, &user, item)? {
                            true => Ok(SimpleReply::Done.try_into()?),
                            false => Ok(SimpleReply::NotFound.try_into()?),
                        },
                        None => Ok(SimpleReply::NotFound.try_into()?),
                    }
                } else {
                    Ok(SimpleReply::Impossible.try_into()?)
                }
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct GiveToAction {
    pub item: Item,
    pub receiver: Item,
}

impl Action for GiveToAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("give-to {:?} -> {:?}", self.item, self.receiver);

        let (_, user, _area) = surroundings.unpack();

        // I think there are very interesting permission related implications
        // here. For example, limiting third party access to your hands except
        // for key individuals.
        match session.find_item(surroundings, &self.item)? {
            Some(item) => match session.find_item(surroundings, &self.receiver)? {
                Some(receiver) => match tools::move_between(&user, &receiver.one()?, item)? {
                    true => Ok(SimpleReply::Done.try_into()?),
                    false => Ok(SimpleReply::NotFound.try_into()?),
                },
                None => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct TradeAction {
    pub giving: Item,
    pub giver: Item,
    pub receiving: Item,
    pub receiver: Item,
}

impl Action for TradeAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("trade {:?} -> {:?}", self.giving, self.receiving);

        let (_, target, area) = surroundings.unpack();

        // TODO Target removes giving
        // TODO Receiver removes receiving
        // TODO Target adds receiving
        // TODO Receiver adds giving

        let giver = session.find_item(surroundings, &self.giver)?;
        let giving = session.find_item(surroundings, &self.giving)?;
        let receiving = session.find_item(surroundings, &self.receiving)?;
        let receiver = session.find_item(surroundings, &self.receiver)?;

        println!("{:?} (giver) -> {:?}", self.giver, giver);
        println!("{:?} (giving) -> {:?}", self.giving, giving);
        println!("{:?} (receiving) -> {:?}", self.receiving, receiving);
        println!("{:?} (receiver) -> {:?}", self.receiver, receiver);

        match (giving, receiving, giver, receiver) {
            (None, None, None, None) => todo!(),
            (None, None, None, Some(_)) => todo!(),
            (None, None, Some(_), None) => todo!(),
            (None, None, Some(_), Some(_)) => todo!(),
            (None, Some(_), None, None) => todo!(),
            (None, Some(_), None, Some(_)) => todo!(),
            (None, Some(_), Some(_), None) => todo!(),
            (None, Some(_), Some(_), Some(_)) => todo!(),
            (Some(_), None, None, None) => todo!(),
            (Some(_), None, None, Some(_)) => todo!(),
            (Some(_), None, Some(_), None) => todo!(),
            (Some(giving), None, Some(giver), Some(receiver)) => todo!(),
            (Some(_), Some(_), None, None) => todo!(),
            (Some(_), Some(_), None, Some(_)) => todo!(),
            (Some(_), Some(_), Some(_), None) => todo!(),
            (Some(giving), Some(receiving), Some(giver), Some(receiver)) => todo!(),
        }
    }
}
