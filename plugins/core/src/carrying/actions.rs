use crate::{carrying::model::Carrying, library::actions::*, looking::model::Observe};

#[action]
pub struct HoldAction {
    pub item: Item,
}

impl Action for HoldAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("hold {:?}!", self.item);

        let (_, living, area) = surroundings.unpack();

        match session.find_item(surroundings, &self.item)? {
            Some(holding) => match tools::move_between(&area, &living, &holding)? {
                true => Ok(reply_ok(
                    living.clone(),
                    Audience::Area(area.key().clone()),
                    Carrying::Held {
                        living: (&living).observe(&living)?.expect("No observed entity"),
                        item: (&holding).observe(&living)?.expect("No observed entity"),
                        area: (&area).observe(&living)?.expect("No observed entity"),
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
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("drop {:?}!", self.maybe_item);

        let (_, living, area) = surroundings.unpack();

        match &self.maybe_item {
            Some(item) => match session.find_item(surroundings, item)? {
                Some(dropping) => match tools::move_between(&living, &area, &dropping)? {
                    true => Ok(reply_ok(
                        living.clone(),
                        Audience::Area(area.key().clone()),
                        Carrying::Dropped {
                            living: (&living).observe(&living)?.expect("No observed entity"),
                            item: (&dropping).observe(&living)?.expect("No observed entity"),
                            area: (&area).observe(&living)?.expect("No observed entity"),
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
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("put-inside {:?} -> {:?}", self.item, self.vessel);

        let (_, _user, _area) = surroundings.unpack();

        match session.find_item(surroundings, &self.item)? {
            Some(item) => match session.find_item(surroundings, &self.vessel)? {
                Some(vessel) => {
                    if tools::is_container(&vessel)? {
                        let from = tools::container_of(&item)?;
                        match tools::move_between(&from, &vessel, &item)? {
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
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("take-out {:?} -> {:?}", self.item, self.vessel);

        let (_, user, _area) = surroundings.unpack();

        match session.find_item(surroundings, &self.vessel)? {
            Some(vessel) => {
                if tools::is_container(&vessel)? {
                    match session.find_item(surroundings, &self.item)? {
                        Some(item) => match tools::move_between(&vessel, &user, &item)? {
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
