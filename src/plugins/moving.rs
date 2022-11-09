use anyhow::Result;
use nom::{bytes::complete::tag, combinator::map, sequence::separated_pair, IResult};

use super::library::*;
use crate::kernel::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Sentence {
    Go(Item), // TODO Make this more specific.
}

fn go(i: &str) -> IResult<&str, Sentence> {
    map(
        separated_pair(tag("go"), spaces, named_place),
        |(_, target)| Sentence::Go(target),
    )(i)
}

fn parse(i: &str) -> IResult<&str, Sentence> {
    go(i)
}

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    Ok(parse(i).map(|(_, sentence)| actions::evaluate(&sentence))?)
}

pub mod model {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    use crate::kernel::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Occupying {
        pub area: LazyLoadedEntity,
    }

    impl Scope for Occupying {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "occupying"
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Occupying {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            self.area = infra.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Occupyable {
        pub acls: Acls,
        pub occupied: Vec<LazyLoadedEntity>,
        pub occupancy: u32,
    }

    impl Occupyable {
        pub fn stop_occupying(&mut self, item: EntityPtr) -> Result<DomainOutcome> {
            let before = self.occupied.len();
            self.occupied.retain(|i| i.key != item.borrow().key);
            let after = self.occupied.len();
            if before == after {
                return Ok(DomainOutcome::Nope);
            }

            Ok(DomainOutcome::Ok(vec![]))
        }

        pub fn start_occupying(&mut self, item: EntityPtr) -> Result<DomainOutcome> {
            self.occupied.push(item.into());

            Ok(DomainOutcome::Ok(vec![]))
        }
    }

    impl Scope for Occupyable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "occupyable"
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Occupyable {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            self.occupied = self
                .occupied
                .iter()
                .map(|r| infra.ensure_entity(r).unwrap())
                .collect();
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Exit {
        pub area: LazyLoadedEntity,
    }

    impl Scope for Exit {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "exit"
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Exit {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            self.area = infra.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AreaRoute {
        pub area: LazyLoadedEntity,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Movement {
        pub routes: Vec<AreaRoute>,
    }

    impl Scope for Movement {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "movement"
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Movement {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            for route in self.routes.iter_mut() {
                route.area = infra.ensure_entity(&route.area)?;
            }
            Ok(())
        }
    }

    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(occupyable) = source.scope::<Occupyable>() {
            // Pretty sure this clone should be unnecessary.
            entity_keys.extend(occupyable.occupied.iter().map(|er| er.key.clone()));
        }
        if let Ok(movement) = source.scope::<Movement>() {
            for route in &movement.routes {
                entity_keys.push(route.area.key.clone());
            }
        }
        Ok(())
    }
}

pub mod actions {
    use super::*;
    use crate::plugins::tools;
    use tracing::info;

    #[derive(Debug)]
    struct GoAction {
        item: Item,
    }

    impl Action for GoAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("go {:?}!", self.item);

            let (_, user, area, infra) = args.clone();

            match infra.find_item(args, &self.item)? {
                Some(to_area) => match tools::navigate_between(area, to_area, user)? {
                    DomainOutcome::Ok(_) => Ok(Box::new(SimpleReply::Done)),
                    DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Go(e) => Box::new(GoAction { item: e.clone() }),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::{BuildActionArgs, QuickThing};

        #[test]
        fn it_goes_through_routes() -> Result<()> {
            let args: ActionArgs = BuildActionArgs::new()?
                .ground(vec![QuickThing::Route(
                    "East".to_string(),
                    Box::new(QuickThing::Place("Place".to_string())),
                )])
                .try_into()?;

            let action = GoAction {
                item: Item::Route("east".to_string()),
            };
            let reply = action.perform(args.clone())?;
            let (_, person, area, _) = args.clone();

            assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

            assert_ne!(tools::area_of(&person)?.key(), area.key());

            Ok(())
        }

        #[test]
        fn it_fails_to_go_unknown_items() -> Result<()> {
            let args: ActionArgs = BuildActionArgs::new()?.plain().try_into()?;

            let action = GoAction {
                item: Item::Route("rake".to_string()),
            };
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

            Ok(())
        }

        #[test]
        fn it_fails_to_go_non_routes() -> Result<()> {
            let args: ActionArgs = BuildActionArgs::new()?
                .ground(vec![QuickThing::Object("Cool Rake".to_string())])
                .try_into()?;

            let action = GoAction {
                item: Item::Route("rake".to_string()),
            };
            let reply = action.perform(args.clone())?;
            let (_, _person, _area, _) = args.clone();

            assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_go_noun_correctly() {
        let (remaining, actual) = parse("go west").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Go(Item::Route("west".to_owned())))
    }

    #[test]
    fn it_errors_on_unknown_text() {
        let output = parse("hello");
        assert!(output.is_err()); // TODO Weak assertion.
    }
}
