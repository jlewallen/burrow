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
    use serde::{de::DeserializeOwned, Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Occupying<T: HasEntityKey> {
        pub area: T,
    }

    impl<T: HasEntityKey> Scope for Occupying<T> {
        fn scope_key() -> &'static str {
            "occupying"
        }
    }

    impl<T: HasEntityKey + DeserializeOwned> TryFrom<&Entity> for Box<Occupying<T>> {
        type Error = DomainError;

        fn try_from(value: &Entity) -> Result<Self, Self::Error> {
            Ok(value.scope::<Occupying<T>>()?)
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Occupyable<T: HasEntityKey = EntityRef> {
        pub acls: Acls,
        pub occupied: Vec<T>,
        pub occupancy: u32,
    }

    impl<T: HasEntityKey> Scope for Occupyable<T> {
        fn scope_key() -> &'static str {
            "occupyable"
        }
    }

    impl<T: HasEntityKey + DeserializeOwned> TryFrom<&Entity> for Box<Occupyable<T>> {
        type Error = DomainError;

        fn try_from(value: &Entity) -> Result<Self, Self::Error> {
            Ok(value.scope::<Occupyable<T>>()?)
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Exit {}

    impl Scope for Exit {
        fn scope_key() -> &'static str {
            "exit"
        }
    }

    impl TryFrom<&Entity> for Box<Exit> {
        type Error = DomainError;

        fn try_from(value: &Entity) -> Result<Self, Self::Error> {
            value.scope::<Exit>()
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
        fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Box<dyn Reply>> {
            info!("go!");

            Ok(Box::new(SimpleReply::Done))
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
