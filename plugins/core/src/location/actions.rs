use crate::library::actions::*;

use super::Location;

#[action]
pub struct RelocateAction {
    pub item: Item,
    pub destination: Item,
}

impl Action for RelocateAction {
    fn is_read_only(&self) -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(surroundings, &self.item)? {
            Some(item) => match session.find_item(surroundings, &self.destination)? {
                Some(destination) => {
                    let destination = destination.one()?;
                    match Location::get(&item.clone().one()?)? {
                        Some(location) => {
                            tools::move_between(&location.to_entity()?, &destination, item)?
                        }
                        None => tools::start_carrying(&destination, item)?,
                    };

                    Ok(SimpleReply::Done.try_into()?)
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}
