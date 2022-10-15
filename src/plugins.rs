pub mod looking {
    use crate::kernel::*;
    use anyhow::Result;
    use nom::{bytes::complete::tag, combinator::map, IResult};

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Look,
    }

    fn look(i: &str) -> IResult<&str, Sentence> {
        map(tag("look"), |_| Sentence::Look)(i)
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        look(i)
    }

    pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
        match parse(i).map(|(_, sentence)| actions::evaluate(&sentence)) {
            Ok(action) => Ok(action),
            Err(_e) => Err(EvaluationError::ParseFailed),
        }
    }

    pub mod model {
        use crate::kernel::*;
        use anyhow::Result;

        pub fn discover(_source: &Entity, _entity_keys: &mut Vec<EntityKey>) -> Result<()> {
            Ok(())
        }
    }

    pub mod actions {
        use super::*;
        use anyhow::Result;
        use tracing::info;

        #[derive(Debug)]
        struct LookAction {}
        impl Action for LookAction {
            fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Reply> {
                info!("look!");

                Ok(Reply {})
            }
        }

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            match *s {
                Sentence::Look => Box::new(LookAction {}),
            }
        }
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
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err()); // TODO Weak
        }
    }
}

pub mod carrying {
    use crate::kernel::*;
    use crate::library::{noun, spaces};
    use anyhow::Result;
    use nom::{
        branch::alt, bytes::complete::tag, combinator::map, sequence::separated_pair, IResult,
    };

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
        match parse(i).map(|(_, sentence)| actions::evaluate(&sentence)) {
            Ok(action) => Ok(action),
            Err(_e) => Err(EvaluationError::ParseFailed),
        }
    }

    pub mod model {
        use crate::kernel::*;
        use anyhow::Result;
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Location {
            pub container: Option<EntityRef>,
        }

        impl Scope for Location {
            fn scope_key() -> &'static str {
                "location"
            }
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Containing {
            pub holding: Vec<EntityRef>,
            pub capacity: Option<u32>,
            pub produces: HashMap<String, String>,
        }

        impl Scope for Containing {
            fn scope_key() -> &'static str {
                "containing"
            }
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Carryable {}

        impl Scope for Carryable {
            fn scope_key() -> &'static str {
                "carryable"
            }
        }

        #[derive(Debug)]
        pub enum CarryingEvent {
            ItemHeld(Entity),
            ItemDropped(Entity),
        }

        impl DomainEvent for CarryingEvent {}

        pub type CarryingResult = DomainResult<CarryingEvent>;

        impl Containing {
            pub fn hold(&self, item: Entity) -> CarryingResult {
                CarryingResult {
                    events: vec![CarryingEvent::ItemHeld(item)],
                }
            }

            pub fn drop(&self, item: Entity) -> CarryingResult {
                CarryingResult {
                    events: vec![CarryingEvent::ItemDropped(item)],
                }
            }
        }

        pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
            if let Ok(containing) = source.scope::<Containing>() {
                entity_keys.extend(containing.holding.into_iter().map(|er| er.key))
            }
            Ok(())
        }
    }

    pub mod actions {
        use super::*;
        // use crate::kernel::*;
        use anyhow::Result;
        use tracing::info;

        #[derive(Debug)]
        struct HoldAction {
            maybe_item: Item,
        }
        impl Action for HoldAction {
            fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Reply> {
                info!("hold {:?}!", self.maybe_item);

                Ok(Reply {})
            }
        }

        #[derive(Debug)]
        struct DropAction {
            maybe_item: Option<Item>,
        }
        impl Action for DropAction {
            fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Reply> {
                info!("drop {:?}!", self.maybe_item);

                Ok(Reply {})
            }
        }

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            // TODO This could be improved.
            match &*s {
                Sentence::Hold(e) => Box::new(HoldAction {
                    maybe_item: e.clone(),
                }),
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
            assert!(output.is_err()); // TODO Weak
        }
    }
}

pub mod moving {
    use crate::kernel::*;
    use crate::library::{noun, spaces};
    use anyhow::Result;
    use nom::{bytes::complete::tag, combinator::map, sequence::separated_pair, IResult};

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Go(Item), // TODO Make this more specific.
    }

    fn go(i: &str) -> IResult<&str, Sentence> {
        map(separated_pair(tag("go"), spaces, noun), |(_, target)| {
            Sentence::Go(target)
        })(i)
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        go(i)
    }

    pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
        match parse(i).map(|(_, sentence)| actions::evaluate(&sentence)) {
            Ok(action) => Ok(action),
            Err(_e) => Err(EvaluationError::ParseFailed),
        }
    }

    pub mod model {
        use crate::kernel::*;
        use anyhow::Result;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Occupying {
            pub area: EntityRef,
        }

        impl Scope for Occupying {
            fn scope_key() -> &'static str {
                "occupying"
            }
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Occupyable {
            pub acls: Acls,
            pub occupied: Vec<EntityRef>,
            pub occupancy: u32,
        }

        impl Scope for Occupyable {
            fn scope_key() -> &'static str {
                "occupyable"
            }
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Exit {}

        impl Scope for Exit {
            fn scope_key() -> &'static str {
                "exit"
            }
        }

        pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
            if let Ok(occupyable) = source.scope::<Occupyable>() {
                entity_keys.extend(occupyable.occupied.into_iter().map(|er| er.key));
            }
            Ok(())
        }
    }

    pub mod actions {
        use super::*;
        use anyhow::Result;
        use tracing::info;

        #[derive(Debug)]
        struct GoAction {}

        impl Action for GoAction {
            fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Reply> {
                info!("go!");

                Ok(Reply {})
            }
        }

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            match *s {
                Sentence::Go(_) => Box::new(GoAction {}),
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
            assert_eq!(actual, Sentence::Go(Item::Named("west".to_owned())))
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err()); // TODO Weak
        }
    }
}

pub mod users {
    pub mod model {
        use crate::kernel::*;
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Usernames {
            pub users: HashMap<String, String>,
        }

        impl Scope for Usernames {
            fn scope_key() -> &'static str {
                "usernames"
            }
        }
    }
}
