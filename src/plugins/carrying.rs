use crate::plugins::library::plugin::*;

pub struct CarryingPlugin {}

impl ParsesActions for CarryingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::DropActionParser {}, i)
            .or_else(|_| try_parsing(parser::HoldActionParser {}, i))
            .or_else(|_| try_parsing(parser::PutInsideActionParser {}, i))
            .or_else(|_| try_parsing(parser::TakeOutActionParser {}, i))
    }
}

pub mod model {
    use crate::plugins::{library::model::*, looking::model::Observe};

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug)]
    pub enum CarryingEvent {
        ItemHeld {
            living: EntityPtr,
            item: EntityPtr,
            area: EntityPtr,
        },
        ItemDropped {
            living: EntityPtr,
            item: EntityPtr,
            area: EntityPtr,
        },
    }

    impl DomainEvent for CarryingEvent {
        fn audience(&self) -> Audience {
            match self {
                Self::ItemHeld {
                    living: _,
                    item: _,
                    area,
                } => Audience::Area(area.clone()),
                Self::ItemDropped {
                    living: _,
                    item: _,
                    area,
                } => Audience::Area(area.clone()),
            }
        }

        fn observe(&self, user: &EntityPtr) -> Result<Box<dyn Observed>> {
            Ok(match self {
                CarryingEvent::ItemHeld {
                    living,
                    item,
                    area: _area,
                } => Box::new(SimpleObservation::new(json!({ "held": {
                        "living": living.observe(user)?,
                         "item": item.observe(user)?}}))),
                CarryingEvent::ItemDropped {
                    living,
                    item,
                    area: _area,
                } => Box::new(SimpleObservation::new(json!({ "dropped": {
                        "living": living.observe(user)?,
                         "item": item.observe(user)?}}))),
            })
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Location {
        pub container: Option<LazyLoadedEntity>,
    }

    impl Scope for Location {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "location"
        }
    }

    impl Needs<Rc<dyn Infrastructure>> for Location {
        fn supply(&mut self, infra: &Rc<dyn Infrastructure>) -> Result<()> {
            self.container = infra.ensure_optional_entity(&self.container)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Containing {
        pub holding: Vec<LazyLoadedEntity>,
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

    impl Needs<Rc<dyn Infrastructure>> for Containing {
        fn supply(&mut self, infra: &Rc<dyn Infrastructure>) -> Result<()> {
            self.holding = self
                .holding
                .iter()
                .map(|r| infra.ensure_entity(r))
                .collect::<Result<Vec<_>>>()?;

            Ok(())
        }
    }

    impl Containing {
        pub fn start_carrying(&mut self, item: &EntityPtr) -> CarryingResult {
            let carryable = item.borrow().scope::<Carryable>()?;

            let holding = self
                .holding
                .iter()
                .map(|h| h.into_entity())
                .collect::<Result<Vec<_>, DomainError>>()?;

            for held in holding {
                if is_kind(&held, &carryable.kind)? {
                    let mut held = held.borrow_mut();
                    let mut combining = held.scope_mut::<Carryable>()?;

                    combining.increase_quantity(carryable.quantity)?;

                    combining.save()?;

                    return Ok(DomainOutcome::Ok);
                }
            }

            self.holding.push(item.clone().into());

            Ok(DomainOutcome::Ok)
        }

        pub fn start_carrying_entry(&mut self, item: &Entry) -> CarryingResult {
            self.start_carrying(
                &get_my_session()
                    .expect("msg")
                    .load_entity_by_key(&item.key)?
                    .unwrap(),
            )
        }

        pub fn stop_carrying_entry(&mut self, item: &Entry) -> CarryingResult {
            self.stop_carrying(
                &get_my_session()
                    .expect("msg")
                    .load_entity_by_key(&item.key)?
                    .unwrap(),
            )
        }

        pub fn stop_carrying(&mut self, item: &EntityPtr) -> CarryingResult {
            let before = self.holding.len();
            self.holding.retain(|i| i.key != item.borrow().key);
            let after = self.holding.len();
            if before == after {
                return Ok(DomainOutcome::Nope);
            }

            Ok(DomainOutcome::Ok)
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Carryable {
        kind: Kind,
        quantity: f32,
    }

    impl Default for Carryable {
        fn default() -> Self {
            let session = get_my_session().expect("No session in Entity::new_blank!");
            Self {
                kind: Kind::new(session.new_identity()),
                quantity: Default::default(),
            }
        }
    }

    fn is_kind(entity: &EntityPtr, kind: &Kind) -> Result<bool> {
        Ok(*entity.borrow().scope::<Carryable>()?.kind() == *kind)
    }

    impl Carryable {
        pub fn quantity(&self) -> f32 {
            self.quantity
        }

        pub fn decrease_quantity(&mut self, q: f32) -> Result<&mut Self, DomainError> {
            if q < 1.0 {
                Err(DomainError::Impossible)
            } else if q > self.quantity {
                Err(DomainError::Impossible)
            } else {
                self.quantity -= q;

                Ok(self)
            }
        }

        pub fn increase_quantity(&mut self, q: f32) -> Result<&mut Self> {
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
    }

    impl Scope for Carryable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "carryable"
        }
    }

    impl Needs<Rc<dyn Infrastructure>> for Carryable {
        fn supply(&mut self, _infra: &Rc<dyn Infrastructure>) -> Result<()> {
            Ok(())
        }
    }

    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(containing) = source.scope::<Containing>() {
            entity_keys.extend(containing.holding.iter().map(|er| er.key.clone()))
        }
        Ok(())
    }
}

pub mod actions {
    use crate::plugins::{carrying::model::CarryingEvent, library::actions::*};

    #[derive(Debug)]
    pub struct HoldAction {
        pub item: Item,
    }

    impl Action for HoldAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("hold {:?}!", self.item);

            let (_, user, area, infra) = args.clone();

            match infra.find_item(args, &self.item)? {
                Some(holding) => match tools::move_between(&area, &user, &holding)? {
                    DomainOutcome::Ok => Ok(Box::new(reply_done(CarryingEvent::ItemHeld {
                        living: user,
                        item: holding,
                        area: area,
                    })?)),
                    DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct DropAction {
        pub maybe_item: Option<Item>,
    }

    impl Action for DropAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("drop {:?}!", self.maybe_item);

            let (_, user, area, infra) = args.clone();

            match &self.maybe_item {
                Some(item) => match infra.find_item(args, item)? {
                    Some(dropping) => match tools::move_between(&user, &area, &dropping)? {
                        DomainOutcome::Ok => {
                            Ok(Box::new(reply_done(CarryingEvent::ItemDropped {
                                living: user,
                                item: dropping,
                                area: area,
                            })?))
                        }
                        DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                    },
                    None => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct PutInsideAction {
        pub item: Item,
        pub vessel: Item,
    }

    impl Action for PutInsideAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("put-inside {:?} -> {:?}", self.item, self.vessel);

            let (_, _user, _area, infra) = args.clone();

            match infra.find_item(args.clone(), &self.item)? {
                Some(item) => match infra.find_item(args, &self.vessel)? {
                    Some(vessel) => {
                        if tools::is_container(&vessel) {
                            let from = tools::container_of(&item)?;
                            match tools::move_between(&from, &vessel, &item)? {
                                DomainOutcome::Ok => Ok(Box::new(SimpleReply::Done)),
                                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                            }
                        } else {
                            Ok(Box::new(SimpleReply::Impossible))
                        }
                    }
                    None => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct TakeOutAction {
        pub item: Item,
        pub vessel: Item,
    }

    impl Action for TakeOutAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("take-out {:?} -> {:?}", self.item, self.vessel);

            let (_, user, _area, infra) = args.clone();

            match infra.find_item(args.clone(), &self.vessel)? {
                Some(vessel) => {
                    if tools::is_container(&vessel) {
                        match infra.find_item(args, &self.item)? {
                            Some(item) => {
                                match tools::move_between_entries(
                                    &vessel.try_into()?,
                                    &user.try_into()?,
                                    &item.try_into()?,
                                )? {
                                    DomainOutcome::Ok => Ok(Box::new(SimpleReply::Done)),
                                    DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                                }
                            }
                            None => Ok(Box::new(SimpleReply::NotFound)),
                        }
                    } else {
                        Ok(Box::new(SimpleReply::Impossible))
                    }
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::plugins::library::parser::*;

    pub struct HoldActionParser {}

    impl ParsesActions for HoldActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
                HoldAction { item: target }
            })(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct DropActionParser {}

    impl ParsesActions for DropActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
                DropAction {
                    maybe_item: Some(target),
                }
            });

            let everything = map(tag("drop"), |_| DropAction { maybe_item: None });

            let (_, action) = alt((specific, everything))(i)?;

            Ok(Box::new(action))
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

            Ok(Box::new(action))
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

            Ok(Box::new(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use super::*;
    use crate::plugins::carrying::model::Location;
    use crate::plugins::tools;
    use crate::{
        domain::{BuildActionArgs, QuickThing},
        plugins::carrying::model::Containing,
    };

    #[test]
    fn it_holds_unheld_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    #[ignore]
    fn it_separates_multiple_ground_items_when_held() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Multiple("Cool Rake", 2.0)])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    #[ignore]
    fn it_combines_multiple_items_when_together_on_ground() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let same_kind = build.make(QuickThing::Object("Cool Rake"))?;
        tools::set_quantity(&same_kind, 2.0)?;
        let (first, second) = tools::separate(same_kind, 1.0)?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Actual(first.clone())])
            .hands(vec![QuickThing::Actual(second.clone())])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_hold_unknown_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_drops_held_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .hands(vec![QuickThing::Object("Cool Rake")])
            .try_into()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_drop_unknown_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_drop_unheld_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(DropActionParser {}, "drop rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.borrow().scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_puts_item_in_non_containers() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let vessel = build.build()?.named("Not A Vessel")?.into_entity()?;
        let args: ActionArgs = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .try_into()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args;

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.borrow().scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_puts_items_in_containers() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let vessel = build
            .build()?
            .named("Vessel")?
            .holding(&vec![])?
            .into_entity()?;
        let args: ActionArgs = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .try_into()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args;

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);
        assert_eq!(vessel.borrow().scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_takes_items_out_of_containers() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let key = build.build()?.named("Key")?.into_entity()?;
        let vessel = build
            .build()?
            .named("Vessel")?
            .holding(&vec![key.clone()])?
            .into_entity()?;
        let args: ActionArgs = build
            .hands(vec![QuickThing::Actual(vessel.clone())])
            .try_into()?;

        let action = try_parsing(TakeOutActionParser {}, "take key out of vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args;

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.borrow().scope::<Containing>()?.holding.len(), 0);
        assert_eq!(
            key.borrow()
                .scope::<Location>()?
                .container
                .as_ref()
                .unwrap()
                .key,
            person.key()
        );

        build.close()?;

        Ok(())
    }
}
