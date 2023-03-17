use crate::plugins::library::plugin::*;

#[derive(Default)]
pub struct CarryingPlugin {}

impl Plugin for CarryingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "carrying"
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
    use crate::plugins::{library::model::*, looking::model::Observe};

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug)]
    pub enum CarryingEvent {
        ItemHeld {
            living: Entry,
            item: Entry,
            area: Entry,
        },
        ItemDropped {
            living: Entry,
            item: Entry,
            area: Entry,
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

        fn observe(&self, user: &Entry) -> Result<Box<dyn Observed>> {
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            self.container = infra.ensure_optional_entity(&self.container)?;
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            self.holding = self
                .holding
                .iter()
                .map(|r| infra.ensure_entity(r))
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
                .map(|h| h.into_entry())
                .collect::<Result<Vec<_>, _>>()?;

            for held in holding {
                if is_kind(&held, &carryable.kind)? {
                    let mut combining = held.scope_mut::<Carryable>()?;

                    combining.increase_quantity(carryable.quantity)?;

                    combining.save()?;

                    return Ok(DomainOutcome::Ok);
                }
            }

            self.holding.push(item.try_into()?);

            Ok(DomainOutcome::Ok)
        }

        pub fn is_holding(&self, item: &Entry) -> Result<bool> {
            Ok(self.holding.iter().any(|i| i.key == item.key()))
        }

        pub fn stop_carrying(&mut self, item: &Entry) -> CarryingResult {
            if !self.is_holding(item)? {
                return Ok(DomainOutcome::Nope);
            }

            self.holding = self
                .holding
                .iter()
                .map(|i| -> Result<Vec<EntityRef>> {
                    if i.key == item.key() {
                        let mut carryable = item.scope_mut::<Carryable>()?;
                        if carryable.quantity > 1.0 {
                            carryable.decrease_quantity(1.0)?;

                            Ok(vec![i.clone()])
                        } else {
                            Ok(vec![])
                        }
                    } else {
                        Ok(vec![i.clone()])
                    }
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<EntityRef>>()
                .to_vec();

            Ok(DomainOutcome::Ok)
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
        fn supply(&mut self, _infra: &SessionRef) -> Result<()> {
            Ok(())
        }
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

            let (_, user, area, infra) = args.unpack();

            match infra.find_item(args, &self.item)? {
                Some(holding) => match tools::move_between(&area, &user, &holding)? {
                    DomainOutcome::Ok => Ok(Box::new(reply_done(CarryingEvent::ItemHeld {
                        living: user,
                        item: holding,
                        area,
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

            let (_, user, area, infra) = args.unpack();

            match &self.maybe_item {
                Some(item) => match infra.find_item(args, item)? {
                    Some(dropping) => match tools::move_between(&user, &area, &dropping)? {
                        DomainOutcome::Ok => {
                            Ok(Box::new(reply_done(CarryingEvent::ItemDropped {
                                living: user,
                                item: dropping,
                                area,
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

            let (_, _user, _area, infra) = args.unpack();

            match infra.find_item(args.clone(), &self.item)? {
                Some(item) => match infra.find_item(args, &self.vessel)? {
                    Some(vessel) => {
                        if tools::is_container(&vessel)? {
                            let from = tools::container_of(&item)?;
                            match tools::move_between(&from.try_into()?, &vessel, &item)? {
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

            let (_, user, _area, infra) = args.unpack();

            match infra.find_item(args.clone(), &self.vessel)? {
                Some(vessel) => {
                    if tools::is_container(&vessel)? {
                        match infra.find_item(args, &self.item)? {
                            Some(item) => match tools::move_between(&vessel, &user, &item)? {
                                DomainOutcome::Ok => Ok(Box::new(SimpleReply::Done)),
                                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                            },
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
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_separates_multiple_ground_items_when_held() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Multiple("Cool Rake", 2.0)])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_combines_multiple_items_when_together_on_ground() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let same_kind = build.make(QuickThing::Object("Cool Rake"))?;
        tools::set_quantity(&same_kind, 2.0)?;
        let (first, second) = tools::separate(same_kind, 1.0)?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Actual(first)])
            .hands(vec![QuickThing::Actual(second)])
            .try_into()?;

        let action = try_parsing(HoldActionParser {}, "hold rake")?;
        let reply = action.perform(args.clone())?;
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

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
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

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
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

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
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

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
        let (_, person, area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_puts_item_in_non_containers() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let vessel = build.build()?.named("Not A Vessel")?.into_entry()?;
        let args: ActionArgs = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .try_into()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 0);

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
            .into_entry()?;
        let args: ActionArgs = build
            .hands(vec![
                QuickThing::Object("key"),
                QuickThing::Actual(vessel.clone()),
            ])
            .try_into()?;

        let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 1);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_takes_items_out_of_containers() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let key = build.build()?.named("Key")?.into_entry()?;
        let vessel = build
            .build()?
            .named("Vessel")?
            .holding(&vec![key.clone()])?
            .into_entry()?;
        let args: ActionArgs = build
            .hands(vec![QuickThing::Actual(vessel.clone())])
            .try_into()?;

        let action = try_parsing(TakeOutActionParser {}, "take key out of vessel")?;
        let reply = action.perform(args.clone())?;
        let (_world, person, _area, _) = args.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        assert_eq!(person.scope::<Containing>()?.holding.len(), 2);
        assert_eq!(vessel.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(
            key.scope::<Location>()?.container.as_ref().unwrap().key,
            person.key()
        );

        build.close()?;

        Ok(())
    }
}
