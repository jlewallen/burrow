use crate::{library::actions::*, tools::container_of};

#[action]
pub struct MoveAction {
    pub item: Item,
    pub destination: Item,
}

impl Action for MoveAction {
    fn is_read_only() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        match session.find_item(surroundings, &self.item)? {
            Some(item) => match session.find_item(surroundings, &self.destination)? {
                Some(destination) => {
                    let moving_from = container_of(&item)?;

                    tools::move_between(&moving_from, &destination, &item)?;

                    Ok(SimpleReply::Done.try_into()?)
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}
