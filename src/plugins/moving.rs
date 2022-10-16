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
    use crate::{domain::HasSession, kernel::*};
    use anyhow::Result;
    use once_cell::sync::Lazy;
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

    impl TryFrom<Occupying<EntityRef>> for Occupying<Lazy<Entity>> {
        type Error = anyhow::Error;

        fn try_from(value: Occupying<EntityRef>) -> Result<Self, Self::Error> {
            Ok(Occupying {
                area: Lazy::new(|| todo!()),
            })
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

    impl<'a> TryFrom<HasSession<'a, Occupyable<EntityRef>>> for Occupyable<Lazy<&Entity>> {
        type Error = anyhow::Error;

        fn try_from(value: HasSession<'a, Occupyable<EntityRef>>) -> Result<Self, Self::Error> {
            Ok(Occupyable {
                acls: value.value.acls,
                occupancy: value.value.occupancy,
                occupied: value
                    .value
                    .occupied
                    .into_iter()
                    .map(|r| -> Lazy<&Entity> {
                        /*
                        Lazy::new(|| -> &Entity {
                            return value.session.load_entity_by_ref(&r).unwrap();
                        })
                        */
                        todo!();
                    })
                    .collect(),
            })
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Exit<T: HasEntityKey = EntityRef> {
        pub area: T,
    }

    impl<T: HasEntityKey> Scope for Exit<T> {
        fn scope_key() -> &'static str {
            "exit"
        }
    }

    impl<'a> TryFrom<HasSession<'a, Exit<EntityRef>>> for Exit<Lazy<&Entity>> {
        type Error = anyhow::Error;

        fn try_from(value: HasSession<'a, Exit<EntityRef>>) -> Result<Self, Self::Error> {
            Ok(Exit {
                area: value.session.load_entity_by_ref_lazy(&value.value.area),
            })
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
