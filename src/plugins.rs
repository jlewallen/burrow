pub mod looking {
    use crate::kernel::{Action, EvaluationError};
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
            Err(_e) => Err(EvaluationError::ParseError), // TODO Weak
        }
    }

    pub mod model {
        use crate::kernel::EntityKey;

        pub fn discover(_entity_keys: &mut Vec<EntityKey>) {}
    }

    pub mod actions {
        use super::*;
        use crate::kernel::*;
        use anyhow::Result;
        use tracing::info;

        struct LookAction {}
        impl Action for LookAction {
            fn perform(&self, args: ActionArgs) -> Result<Reply> {
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
    use crate::library::{noun, spaces, Item};
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
            Err(_e) => Err(EvaluationError::ParseError), // TODO Weak
        }
    }

    pub mod model {
        use crate::kernel::*;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Carrying {
            pub holding: Vec<EntityRef>,
        }

        impl Scope for Carrying {
            fn scope_key() -> &'static str {
                "carrying"
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

        impl Carrying {
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

        pub fn discover(_entity_keys: &mut Vec<EntityKey>) {}
    }

    pub mod actions {
        use super::*;
        // use crate::kernel::*;
        use anyhow::Result;
        use tracing::info;

        struct HoldAction {
            sentence: Sentence,
        }
        impl Action for HoldAction {
            fn perform(&self, args: ActionArgs) -> Result<Reply> {
                info!("hold {:?}!", self.sentence);

                Ok(Reply {})
            }
        }

        struct DropAction {
            sentence: Sentence,
        }
        impl Action for DropAction {
            fn perform(&self, args: ActionArgs) -> Result<Reply> {
                info!("drop {:?}!", self.sentence);

                Ok(Reply {})
            }
        }

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            // TODO This could be improved.
            match *s {
                Sentence::Hold(ref _e) => Box::new(HoldAction {
                    sentence: s.clone(),
                }),
                Sentence::Drop(ref _e) => Box::new(DropAction {
                    sentence: s.clone(),
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
            assert_eq!(actual, Sentence::Hold(Item::Described("rake".to_owned())))
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
            assert_eq!(
                actual,
                Sentence::Drop(Some(Item::Described("rake".to_owned())))
            )
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err()); // TODO Weak
        }
    }
}

pub mod moving {
    use crate::kernel::{Action, EvaluationError};
    use crate::library::{noun, spaces, Item};
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
            Err(_e) => Err(EvaluationError::ParseError), // TODO Weak
        }
    }

    pub mod model {
        use crate::kernel::*;
        use serde::{Deserialize, Serialize};

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
        pub struct Exit {}

        impl Scope for Exit {
            fn scope_key() -> &'static str {
                "exit"
            }
        }

        #[derive(Debug, Serialize, Deserialize)]
        pub struct Here {
            pub here: Vec<EntityRef>,
        }

        impl Scope for Here {
            fn scope_key() -> &'static str {
                "here"
            }
        }

        pub fn discover(_entity_keys: &mut Vec<EntityKey>) {}
    }

    pub mod actions {
        use super::*;
        use crate::kernel::*;
        use anyhow::Result;
        use tracing::info;

        struct GoAction {}
        impl Action for GoAction {
            fn perform(&self, args: ActionArgs) -> Result<Reply> {
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
            assert_eq!(actual, Sentence::Go(Item::Described("west".to_owned())))
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
