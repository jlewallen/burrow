use crate::kernel::*;
use crate::library::{noun, spaces};
use anyhow::Result;
use nom::{branch::alt, bytes::complete::tag, combinator::map, sequence::separated_pair, IResult};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Sentence {
    Hold(Item),
    Drop(Option<Item>),
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

pub fn parse(i: &str) -> IResult<&str, Sentence> {
    alt((hold, drop))(i)
}

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    Ok(parse(i).map(|(_, sentence)| actions::evaluate(&sentence))?)
}

pub mod model {
    use crate::kernel::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::{collections::HashMap, rc::Weak};

    pub type CarryingResult = DomainResult;

    #[derive(Debug)]
    pub enum CarryingEvent {
        ItemHeld(EntityPtr),
        ItemDropped(EntityPtr),
    }

    impl DomainEvent for CarryingEvent {}

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Location {
        pub container: Option<DynamicEntityRef>,
    }

    impl Scope for Location {
        fn scope_key() -> &'static str {
            "location"
        }
    }

    impl PrepareWithInfrastructure for Location {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            self.container = infra.ensure_optional_entity(&self.container)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Containing {
        pub holding: Vec<DynamicEntityRef>,
        pub capacity: Option<u32>,
        pub produces: HashMap<String, String>,
    }

    impl Scope for Containing {
        fn scope_key() -> &'static str {
            "containing"
        }
    }

    impl PrepareWithInfrastructure for Containing {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            self.holding = self
                .holding
                .iter()
                .map(|r| infra.ensure_entity(r).unwrap())
                .collect();
            Ok(())
        }
    }

    impl Containing {
        pub fn hold(&self, item: EntityPtr) -> CarryingResult {
            CarryingResult {
                events: vec![Box::new(CarryingEvent::ItemHeld(item))],
            }
        }

        pub fn drop(&self, item: EntityPtr) -> CarryingResult {
            CarryingResult {
                events: vec![Box::new(CarryingEvent::ItemDropped(item))],
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Carryable {}

    impl Scope for Carryable {
        fn scope_key() -> &'static str {
            "carryable"
        }
    }

    impl PrepareWithInfrastructure for Carryable {
        fn prepare_with(&mut self, _infra: &Weak<dyn Infrastructure>) -> Result<()> {
            Ok(())
        }
    }
    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        // TODO Pretty sure this clone should be unnecessary, can we clone into from an iterator?
        if let Ok(containing) = source.scope::<Containing>() {
            entity_keys.extend(containing.holding.iter().map(|er| er.key().clone()))
        }
        Ok(())
    }
}

pub mod actions {
    use crate::plugins::carrying::model::Containing;

    use super::*;
    use tracing::info;

    #[derive(Debug)]
    struct HoldAction {
        item: Item,
    }

    impl Action for HoldAction {
        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("hold {:?}!", self.item);

            let (_, user, _, infra) = args.clone();
            let holding = infra.find_item(args, &self.item)?;

            match holding {
                Some(holding) => {
                    info!("holding {:?}!", holding);
                    let containing = user.borrow().scope::<Containing>()?;

                    containing.hold(holding);

                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    struct DropAction {
        maybe_item: Option<Item>,
    }

    impl Action for DropAction {
        fn perform(&self, (_world, _user, _area, _infra): ActionArgs) -> ReplyResult {
            info!("drop {:?}!", self.maybe_item);

            Ok(Box::new(SimpleReply::Done))
        }
    }

    pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
        // TODO Look into this clone, perhaps other ways of cleaning this up.
        match s {
            Sentence::Hold(e) => Box::new(HoldAction { item: e.clone() }),
            Sentence::Drop(e) => Box::new(DropAction {
                maybe_item: e.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_hold_noun_correctly() {
        let (remaining, actual) = parse("hold rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Hold(Item::Named("rake".to_owned())))
    }

    #[test]
    fn it_parses_solo_drop_correctly() {
        let (remaining, actual) = parse("drop").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Drop(None))
    }

    #[test]
    fn it_parses_drop_noun_correctly() {
        let (remaining, actual) = parse("drop rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Drop(Some(Item::Named("rake".to_owned()))))
    }

    #[test]
    fn it_errors_on_unknown_text() {
        let output = parse("hello");
        assert!(output.is_err()); // TODO Weak assertion.
    }
}
