pub mod model {
    use crate::plugins::library::model::*;
    use crate::{
        plugins::carrying::model::Containing,
        plugins::moving::model::{Movement, Occupyable},
    };

    #[derive(Debug, Serialize)]
    pub struct ObservedArea {}

    impl From<&Entity> for ObservedArea {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ObservedPerson {}

    impl From<&Entity> for ObservedPerson {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ObservedRoute {}

    impl From<&Entity> for ObservedRoute {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ObservedEntity {
        pub key: EntityKey,
        pub name: Option<String>,
        pub desc: Option<String>,
    }

    impl From<Box<Entity>> for ObservedEntity {
        fn from(value: Box<Entity>) -> Self {
            Self {
                key: value.key.clone(),
                name: value.name(),
                desc: value.desc(),
            }
        }
    }

    impl From<&Entity> for ObservedEntity {
        fn from(value: &Entity) -> Self {
            Self {
                key: value.key.clone(),
                name: value.name(),
                desc: value.desc(),
            }
        }
    }

    // TODO This seems unnececssary, how can I help the compiler deduce the
    // proper chain of TryFrom/From to get here?
    impl TryFrom<&LazyLoadedEntity> for ObservedEntity {
        type Error = DomainError;

        fn try_from(value: &LazyLoadedEntity) -> Result<Self, Self::Error> {
            let entity = value.into_entity()?;
            let e = entity.borrow();

            Ok(Self {
                key: e.key.clone(),
                name: e.name(),
                desc: e.desc(),
            })
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AreaObservation {
        pub area: ObservedEntity,
        pub person: ObservedEntity,
        pub living: Vec<ObservedEntity>,
        pub items: Vec<ObservedEntity>,
        pub carrying: Vec<ObservedEntity>,
        pub routes: Vec<ObservedEntity>,
    }

    impl AreaObservation {
        pub fn new(user: &EntityPtr, area: &EntityPtr) -> Result<Self> {
            // I feel like there's a lot of unnecessary copying going on here.

            let mut living: Vec<ObservedEntity> = vec![];
            if let Ok(occupyable) = area.borrow().scope::<Occupyable>() {
                for entity in &occupyable.occupied {
                    living.push(entity.try_into()?);
                }
            }

            let mut items = vec![];
            if let Ok(containing) = area.borrow().scope::<Containing>() {
                for entity in &containing.holding {
                    items.push(entity.try_into()?);
                }
            }

            let mut carrying = vec![];
            if let Ok(containing) = user.borrow().scope::<Containing>() {
                for entity in &containing.holding {
                    carrying.push(entity.try_into()?);
                }
            }

            let mut routes = vec![];
            if let Ok(movement) = user.borrow().scope::<Movement>() {
                for route in &movement.routes {
                    routes.push((&route.area).try_into()?);
                }
            };

            Ok(AreaObservation {
                area: area.borrow().deref().into(),
                person: user.borrow().deref().into(),
                living,
                items,
                carrying,
                routes,
            })
        }
    }

    impl Reply for AreaObservation {
        fn to_markdown(&self) -> Result<Markdown> {
            let mut md = Markdown::new(Vec::new());
            md.write("")?;
            Ok(md)
        }
    }

    impl ToJson for AreaObservation {
        fn to_json(&self) -> Result<Value> {
            Ok(json!({ "areaObservation": serde_json::to_value(self)? }))
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct InsideObservation {
        pub vessel: ObservedEntity,
        pub items: Vec<ObservedEntity>,
    }

    impl InsideObservation {
        pub fn new(_user: &EntityPtr, vessel: &EntityPtr) -> Result<Self> {
            let mut items = vec![];
            if let Ok(containing) = vessel.borrow().scope::<Containing>() {
                for entity in &containing.holding {
                    items.push(entity.try_into()?);
                }
            }

            Ok(InsideObservation {
                vessel: vessel.borrow().deref().into(),
                items,
            })
        }
    }

    impl Reply for InsideObservation {
        fn to_markdown(&self) -> Result<Markdown> {
            let mut md = Markdown::new(Vec::new());
            md.write("")?;
            Ok(md)
        }
    }

    impl ToJson for InsideObservation {
        fn to_json(&self) -> Result<Value> {
            Ok(json!({ "insideObservation": serde_json::to_value(self)? }))
        }
    }

    pub fn discover(_source: &Entity, _entity_keys: &mut [EntityKey]) -> Result<()> {
        Ok(())
    }
}

pub mod actions {
    use super::model::*;
    use super::parser::{parse, Sentence};
    use crate::plugins::library::actions::*;

    #[derive(Debug)]
    pub struct LookAction {}

    impl Action for LookAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, (_world, user, area, _infra): ActionArgs) -> ReplyResult {
            info!("look!");

            Ok(Box::new(AreaObservation::new(&user, &area)?))
        }
    }

    #[derive(Debug)]
    pub struct LookInsideAction {
        item: Item,
    }

    impl Action for LookInsideAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("look inside!");

            let (_, user, _area, infra) = args.clone();

            match infra.find_item(args, &self.item)? {
                Some(target) => {
                    if tools::is_container(&target) {
                        Ok(Box::new(InsideObservation::new(&user, &target)?))
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
        match *s {
            Sentence::Look => Box::new(LookAction {}),
            Sentence::LookInside(_) => todo!(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::{BuildActionArgs, QuickThing};

        #[test]
        fn it_looks_in_empty_area() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build.plain().try_into()?;

            let action = LookAction {};
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }

        #[test]
        fn it_looks_in_area_with_items_on_ground() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Rake".to_string())])
                .ground(vec![QuickThing::Object("Boring Shovel".to_string())])
                .try_into()?;

            let action = LookAction {};
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }

        #[test]
        fn it_looks_in_area_with_items_on_ground_and_a_route() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let destination = build.make(QuickThing::Place("Place".to_string()))?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Rake".to_string())])
                .ground(vec![QuickThing::Object("Boring Shovel".to_string())])
                .route("East Exit", QuickThing::Actual(destination.clone()))
                .try_into()?;

            let action = LookAction {};
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }

        #[test]
        fn it_looks_in_area_with_items_on_ground_and_holding_items() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let destination = build.make(QuickThing::Place("Place".to_string()))?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Boring Shovel".to_string())])
                .hands(vec![QuickThing::Object("Cool Rake".to_string())])
                .route("East Exit", QuickThing::Actual(destination.clone()))
                .try_into()?;

            let action = LookAction {};
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }

        #[test]
        fn it_fails_to_look_inside_non_containers() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .hands(vec![QuickThing::Object("Not A Box".to_string())])
                .try_into()?;

            let action = LookInsideAction {
                item: Item::Named("box".to_owned()),
            };
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }

        #[test]
        fn it_looks_inside_containers() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let vessel = build
                .build()?
                .named("Vessel")?
                .holding(&vec![build.make(QuickThing::Object("Key".to_string()))?])?
                .into_entity()?;
            let args: ActionArgs = build.hands(vec![QuickThing::Actual(vessel)]).try_into()?;

            let action = LookInsideAction {
                item: Item::Named("vessel".to_owned()),
            };
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            insta::assert_json_snapshot!(reply.to_json()?);

            build.close()?;

            Ok(())
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    // TODO Underneath, Above, Behond, etc... 'Physically Relative'
    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Look,
        LookInside(Item),
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        look(i)
    }

    fn look(i: &str) -> IResult<&str, Sentence> {
        let inside = map(
            separated_pair(
                separated_pair(tag("look"), spaces, tag("inside")),
                spaces,
                noun,
            ),
            |(_, nearby)| Sentence::LookInside(nearby),
        );

        let area = map(tag("look"), |_| Sentence::Look);

        alt((inside, area))(i)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_look_correctly() {
            let (remaining, actual) = parse("look").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Look)
        }

        #[test]
        fn it_parses_look_inside_correctly() {
            let (remaining, actual) = parse("look inside box").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::LookInside(Item::Named("box".to_owned())))
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err());
        }
    }
}
