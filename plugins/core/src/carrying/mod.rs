use crate::library::plugin::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct CarryingPluginFactory {}

impl PluginFactory for CarryingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(CarryingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct CarryingPlugin {}

impl Plugin for CarryingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "carrying"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for CarryingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::DropActionParser {}, i)
            .or_else(|_| try_parsing(parser::HoldActionParser {}, i))
            .or_else(|_| try_parsing(parser::PutInsideActionParser {}, i))
            .or_else(|_| try_parsing(parser::TakeOutActionParser {}, i))
    }
}

pub mod model {
    use crate::{library::model::*, tools};

    pub use kernel::common::CarryingEvent;

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Containing {
        pub(crate) holding: Vec<EntityRef>,
        pub(crate) capacity: Option<u32>,
        pub(crate) produces: HashMap<String, String>,
    }

    impl Scope for Containing {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "containing"
        }
    }

    impl Containing {
        pub fn start_carrying(&mut self, item: &Entry) -> CarryingResult {
            let carryable = item.scope::<Carryable>()?.unwrap();

            let holding = self
                .holding
                .iter()
                .map(|h| h.to_entry())
                .collect::<Result<Vec<_>, _>>()?;

            for held in holding {
                if is_kind(&held, &carryable.kind)? {
                    let mut combining = held.scope_mut::<Carryable>()?;

                    combining.increase_quantity(carryable.quantity)?;

                    combining.save()?;

                    get_my_session()?.obliterate(item)?;

                    return Ok(DomainOutcome::Ok);
                }
            }

            self.holding.push(item.try_into()?);

            Ok(DomainOutcome::Ok)
        }

        pub fn is_holding(&self, item: &Entry) -> Result<bool> {
            Ok(self.holding.iter().any(|i| *i.key() == *item.key()))
        }

        fn remove_item(&mut self, item: &Entry) -> CarryingResult {
            self.holding = self
                .holding
                .iter()
                .flat_map(|i| {
                    if *i.key() == *item.key() {
                        vec![]
                    } else {
                        vec![i.clone()]
                    }
                })
                .collect::<Vec<EntityRef>>()
                .to_vec();

            Ok(DomainOutcome::Ok)
        }

        pub fn stop_carrying(&mut self, item: &Entry) -> Result<Option<Entry>> {
            if !self.is_holding(item)? {
                return Ok(None);
            }

            let carryable = item.scope::<Carryable>()?.unwrap_or_default();
            if carryable.quantity > 1.0 {
                let (_original, separated) = tools::separate(item, 1.0)?;

                Ok(Some(separated))
            } else {
                self.remove_item(item)?;

                Ok(Some(item.clone()))
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Carryable {
        kind: Kind,
        quantity: f32,
    }

    fn is_kind(entity: &Entry, kind: &Kind) -> Result<bool> {
        Ok(*entity.scope::<Carryable>()?.unwrap().kind() == *kind)
    }

    impl Default for Carryable {
        fn default() -> Self {
            let session = get_my_session().expect("No session in Entity::new_blank!");
            Self {
                kind: Kind::new(session.new_identity()),
                quantity: 1.0,
            }
        }
    }

    impl Carryable {
        pub fn quantity(&self) -> f32 {
            self.quantity
        }

        pub fn decrease_quantity(&mut self, q: f32) -> Result<&mut Self, DomainError> {
            self.sanity_check_quantity();

            if q < 1.0 || q > self.quantity {
                Err(DomainError::Impossible)
            } else {
                self.quantity -= q;

                Ok(self)
            }
        }

        pub fn increase_quantity(&mut self, q: f32) -> Result<&mut Self> {
            self.sanity_check_quantity();

            self.quantity += q;

            Ok(self)
        }

        pub fn set_quantity(&mut self, q: f32) -> Result<&mut Self> {
            self.quantity = q;

            Ok(self)
        }

        pub fn kind(&self) -> &Kind {
            &self.kind
        }

        pub fn set_kind(&mut self, kind: &Kind) {
            self.kind = kind.clone();
        }

        // Migrate items that were initialized with 0 quantities.
        fn sanity_check_quantity(&mut self) {
            if self.quantity < 1.0 {
                self.quantity = 1.0
            }
        }
    }

    impl Scope for Carryable {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "carryable"
        }
    }
}

pub mod actions {
    use crate::{carrying::model::CarryingEvent, library::actions::*, looking::model::Observe};

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

            let (_, user, area) = surroundings.unpack();

            match session.find_item(surroundings, &self.item)? {
                Some(holding) => match tools::move_between(&area, &user, &holding)? {
                    DomainOutcome::Ok => Ok(reply_ok(
                        Audience::Area(area.key().clone()),
                        CarryingEvent::Held {
                            living: (&user).observe(&user)?.expect("No observed entity"),
                            item: (&holding).observe(&user)?.expect("No observed entity"),
                            area: (&area).observe(&user)?.expect("No observed entity"),
                        },
                    )?),
                    DomainOutcome::Nope => Ok(SimpleReply::NotFound.try_into()?),
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

            let (_, user, area) = surroundings.unpack();

            match &self.maybe_item {
                Some(item) => match session.find_item(surroundings, item)? {
                    Some(dropping) => match tools::move_between(&user, &area, &dropping)? {
                        DomainOutcome::Ok => Ok(reply_ok(
                            Audience::Area(area.key().clone()),
                            CarryingEvent::Dropped {
                                living: (&user).observe(&user)?.expect("No observed entity"),
                                item: (&dropping).observe(&user)?.expect("No observed entity"),
                                area: (&area).observe(&user)?.expect("No observed entity"),
                            },
                        )?),
                        DomainOutcome::Nope => Ok(SimpleReply::NotFound.try_into()?),
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
                                DomainOutcome::Ok => Ok(SimpleReply::Done.try_into()?),
                                DomainOutcome::Nope => Ok(SimpleReply::NotFound.try_into()?),
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
                                DomainOutcome::Ok => Ok(SimpleReply::Done.try_into()?),
                                DomainOutcome::Nope => Ok(SimpleReply::NotFound.try_into()?),
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
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct HoldActionParser {}

    impl ParsesActions for HoldActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
                HoldAction { item: target }
            })(i)?;

            Ok(Some(Box::new(action)))
        }
    }

    pub struct DropActionParser {}

    impl ParsesActions for DropActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
                DropAction {
                    maybe_item: Some(Item::Held(Box::new(target))),
                }
            });

            let everything = map(tag("drop"), |_| DropAction { maybe_item: None });

            let (_, action) = alt((specific, everything))(i)?;

            Ok(Some(Box::new(action)))
        }
    }

    pub struct TakeOutActionParser {}

    impl ParsesActions for TakeOutActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let item = map(separated_pair(tag("take"), spaces, noun), |(_, target)| {
                target
            });

            let (_, action) = map(
                separated_pair(separated_pair(item, spaces, tag("out of")), spaces, noun),
                |(item, target)| TakeOutAction {
                    item: Item::Contained(Box::new(item.0)),
                    vessel: target,
                },
            )(i)?;

            Ok(Some(Box::new(action)))
        }
    }

    pub struct PutInsideActionParser {}

    impl ParsesActions for PutInsideActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let item = map(separated_pair(tag("put"), spaces, noun), |(_, target)| {
                target
            });

            let (_, action) = map(
                separated_pair(
                    separated_pair(
                        item,
                        spaces,
                        pair(tag("inside"), opt(pair(spaces, tag("of")))),
                    ),
                    spaces,
                    noun,
                ),
                |(item, target)| PutInsideAction {
                    item: item.0,
                    vessel: target,
                },
            )(i)?;

            Ok(Some(Box::new(action)))
        }
    }
}
