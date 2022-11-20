pub mod model {
    use crate::plugins::library::model::*;

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug)]
    pub enum CarryingEvent {
        ItemHeld(EntityPtr),
        ItemDropped(EntityPtr),
    }

    impl DomainEvent for CarryingEvent {}

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

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Location {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
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

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Containing {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
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
            self.holding.push(item.clone().into());

            Ok(DomainOutcome::Ok(vec![Box::new(CarryingEvent::ItemHeld(
                item.clone(),
            ))]))
        }

        pub fn stop_carrying(&mut self, item: &EntityPtr) -> CarryingResult {
            let before = self.holding.len();
            self.holding.retain(|i| i.key != item.borrow().key);
            let after = self.holding.len();
            if before == after {
                return Ok(DomainOutcome::Nope);
            }

            Ok(DomainOutcome::Ok(vec![Box::new(
                CarryingEvent::ItemDropped(item.clone()),
            )]))
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    struct Carryable {}

    impl Scope for Carryable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "carryable"
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Carryable {
        fn supply(&mut self, _infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            Ok(())
        }
    }

    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(containing) = source.scope::<Containing>() {
            // TODO Pretty sure this clone should be unnecessary, can we clone into from an iterator?
            entity_keys.extend(containing.holding.iter().map(|er| er.key.clone()))
        }
        Ok(())
    }
}

pub mod actions {
    use super::parser::{parse, Sentence};
    use crate::plugins::library::actions::*;

    #[derive(Debug)]
    struct HoldAction {
        item: Item,
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
                    DomainOutcome::Ok(_) => Ok(Box::new(SimpleReply::Done)),
                    DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    struct DropAction {
        maybe_item: Option<Item>,
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
                        DomainOutcome::Ok(_) => Ok(Box::new(SimpleReply::Done)),
                        DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                    },
                    None => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    struct PutInsideAction {
        item: Item,
        vessel: Item,
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
                                DomainOutcome::Ok(_) => Ok(Box::new(SimpleReply::Done)),
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
    struct TakeOutAction {
        item: Item,
        vessel: Item,
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
                            Some(item) => match tools::move_between(&vessel, &user, &item)? {
                                DomainOutcome::Ok(_) => Ok(Box::new(SimpleReply::Done)),
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

    pub fn evaluate(i: &str) -> EvaluationResult {
        Ok(parse(i).map(|(_, sentence)| evaluate_sentence(&sentence))?)
    }

    fn evaluate_sentence(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Hold(e) => Box::new(HoldAction { item: e.clone() }),
            Sentence::Drop(e) => Box::new(DropAction {
                maybe_item: e.clone(),
            }),
            Sentence::PutInside(item, vessel) => Box::new(PutInsideAction {
                item: item.clone(),
                vessel: vessel.clone(),
            }),
            Sentence::TakeOut(item, vessel) => Box::new(TakeOutAction {
                item: item.clone(),
                vessel: vessel.clone(),
            }),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
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

            let action = HoldAction {
                item: Item::Named("rake".into()),
            };
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

            let action = HoldAction {
                item: Item::Named("rake".into()),
            };
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

            let action = DropAction {
                maybe_item: Some(Item::Named("rake".into())),
            };
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

            let action = DropAction {
                maybe_item: Some(Item::Named("rake".into())),
            };
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

            let action = DropAction {
                maybe_item: Some(Item::Named("rake".into())),
            };
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

            let action = PutInsideAction {
                item: Item::Named("key".to_owned()),
                vessel: Item::Named("vessel".to_owned()),
            };
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

            let action = PutInsideAction {
                item: Item::Named("key".to_owned()),
                vessel: Item::Named("vessel".to_owned()),
            };
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
                .holding(&vec![key])?
                .into_entity()?;
            let args: ActionArgs = build
                .hands(vec![QuickThing::Actual(vessel.clone())])
                .try_into()?;

            let action = TakeOutAction {
                item: Item::Contained(Box::new(Item::Named("key".to_owned()))),
                vessel: Item::Named("vessel".to_owned()),
            };
            let reply = action.perform(args.clone())?;
            let (_world, person, _area, _) = args;

            insta::assert_json_snapshot!(reply.to_json()?);

            assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 2);
            assert_eq!(vessel.borrow().scope::<Containing>()?.holding.len(), 0);

            build.close()?;

            Ok(())
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Hold(Item),
        Drop(Option<Item>),
        PutInside(Item, Item),
        TakeOut(Item, Item),
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        alt((hold, drop, put_inside, take_out))(i)
    }

    fn hold(i: &str) -> IResult<&str, Sentence> {
        map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
            Sentence::Hold(target)
        })(i)
    }

    fn drop(i: &str) -> IResult<&str, Sentence> {
        let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
            Sentence::Drop(Some(target))
        });

        let everything = map(tag("drop"), |_| Sentence::Drop(None));

        alt((specific, everything))(i)
    }

    fn put_inside(i: &str) -> IResult<&str, Sentence> {
        let item = map(separated_pair(tag("put"), spaces, noun), |(_, target)| {
            target
        });

        map(
            separated_pair(separated_pair(item, spaces, tag("inside of")), spaces, noun),
            |(item, target)| Sentence::PutInside(item.0, target),
        )(i)
    }

    fn take_out(i: &str) -> IResult<&str, Sentence> {
        let item = map(separated_pair(tag("take"), spaces, noun), |(_, target)| {
            target
        });

        map(
            separated_pair(separated_pair(item, spaces, tag("out of")), spaces, noun),
            |(item, target)| Sentence::TakeOut(Item::Contained(Box::new(item.0)), target),
        )(i)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_hold_noun_correctly() {
            let (remaining, actual) = parse("hold rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Hold(Item::Named("rake".into())));
        }

        #[test]
        fn it_parses_solo_drop_correctly() {
            let (remaining, actual) = parse("drop").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Drop(None));
        }

        #[test]
        fn it_parses_drop_noun_correctly() {
            let (remaining, actual) = parse("drop rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Drop(Some(Item::Named("rake".into()))));
        }

        #[test]
        fn it_parses_put_x_inside_of_y() {
            let (remaining, actual) = parse("put key inside of vessel").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(
                actual,
                Sentence::PutInside(Item::Named("key".into()), Item::Named("vessel".into()))
            );
        }

        #[test]
        fn it_parses_take_x_out_of_y() {
            let (remaining, actual) = parse("take key out of vessel").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(
                actual,
                Sentence::TakeOut(
                    Item::Contained(Box::new(Item::Named("key".into()))),
                    Item::Named("vessel".into())
                )
            );
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let actual = parse("hello");
            assert!(actual.is_err());
        }
    }
}
