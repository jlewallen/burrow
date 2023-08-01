use crate::library::plugin::*;

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

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub enum CarryingEvent {
        ItemHeld {
            living: EntityRef,
            item: ObservedEntity,
            area: EntityRef,
        },
        ItemDropped {
            living: EntityRef,
            item: ObservedEntity,
            area: EntityRef,
        },
    }

    impl DomainEvent for CarryingEvent {}

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Location {
        pub container: Option<EntityRef>,
    }

    impl Scope for Location {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "location"
        }
    }

    impl Needs<SessionRef> for Location {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.container = session.ensure_optional_entity(&self.container)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Containing {
        pub holding: Vec<EntityRef>,
        pub capacity: Option<u32>,
        pub produces: HashMap<String, String>,
    }

    impl Scope for Containing {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "containing"
        }
    }

    impl Needs<SessionRef> for Containing {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.holding = self
                .holding
                .iter()
                .map(|r| session.ensure_entity(r))
                .collect::<Result<Vec<_>, DomainError>>()?;

            Ok(())
        }
    }

    impl Containing {
        pub fn start_carrying(&mut self, item: &Entry) -> CarryingResult {
            let carryable = item.scope::<Carryable>()?;

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

            let carryable = item.scope_mut::<Carryable>()?;
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
        Ok(*entity.scope::<Carryable>()?.kind() == *kind)
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
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "carryable"
        }
    }

    impl Needs<SessionRef> for Carryable {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
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
                    DomainOutcome::Ok => Ok(reply_done(
                        Audience::Area(area.key().clone()),
                        CarryingEvent::ItemHeld {
                            living: user.entity_ref(),
                            item: (&holding).observe(&user)?.expect("No observed entity"),
                            area: area.entity_ref(),
                        },
                    )?
                    .into()),
                    DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
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
                        DomainOutcome::Ok => Ok(reply_done(
                            Audience::Area(area.key().clone()),
                            CarryingEvent::ItemDropped {
                                living: user.entity_ref(),
                                item: (&dropping).observe(&user)?.expect("No observed entity"),
                                area: area.entity_ref(),
                            },
                        )?
                        .into()),
                        DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                    },
                    None => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
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
                            match tools::move_between(&from.try_into()?, &vessel, &item)? {
                                DomainOutcome::Ok => Ok(SimpleReply::Done.into()),
                                DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                            }
                        } else {
                            Ok(SimpleReply::Impossible.into())
                        }
                    }
                    None => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
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
                                DomainOutcome::Ok => Ok(SimpleReply::Done.into()),
                                DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                            },
                            None => Ok(SimpleReply::NotFound.into()),
                        }
                    } else {
                        Ok(SimpleReply::Impossible.into())
                    }
                }
                None => Ok(SimpleReply::NotFound.into()),
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

#[cfg(test)]
mod tests {
    use super::parser::*;
    use super::*;
    use crate::carrying::model::Containing;
    use crate::carrying::model::Location;
    use crate::library::tests::*;

    #[test]
    fn it_holds_unheld_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .build()?;

        let (_, person, area) = surroundings.unpack();
        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_separates_multiple_ground_items_when_held() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Multiple("Cool Rake", 2.0)])
            .build()?;

        let (_, person, area) = surroundings.unpack();
        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        let held = &person.scope::<Containing>()?.holding;
        let ground = &area.scope::<Containing>()?.holding;
        assert_eq!(held.len(), 1);
        assert_eq!(ground.len(), 1);

        let held_keys: HashSet<_> = held.iter().map(|i| i.key().clone()).collect();
        let ground_keys: HashSet<_> = ground.iter().map(|i| i.key().clone()).collect();
        let common_keys: HashSet<_> = held_keys.intersection(&ground_keys).collect();
        assert_eq!(common_keys.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_combines_multiple_items_when_together_on_ground() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let same_kind = build.make(QuickThing::Object("Cool Rake"))?;
        tools::set_quantity(&same_kind, 2.0)?;
        let (first, second) = tools::separate(&same_kind, 1.0)?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Actual(first.clone())])
            .hands(vec![QuickThing::Actual(second)])
            .build()?;

        let (_, person, area) = surroundings.unpack();
        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_hold_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, person, area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_drops_held_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Object("Cool Rake")]).build()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, person, area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_drop_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, person, area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_drop_unheld_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, person, area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_puts_item_in_non_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build.entity()?.named("Not A Vessel")?.into_entry()?;
        let (session, surroundings) = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .build()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_puts_items_in_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build
            .entity()?
            .named("Vessel")?
            .holding(&vec![])?
            .into_entry()?;
        let (session, surroundings) = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .build()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_takes_items_out_of_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let key = build.entity()?.named("Key")?.into_entry()?;
        let vessel = build
            .entity()?
            .named("Vessel")?
            .holding(&vec![key.clone()])?
            .into_entry()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Actual(vessel.clone())])
            .build()?;

        let action = try_parsing(TakeOutActionParser {}, "take key out of vessel")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(
            *key.scope::<Location>()?.container.as_ref().unwrap().key(),
            *person.key()
        );

        build.close()?;

        Ok(())
    }
}
