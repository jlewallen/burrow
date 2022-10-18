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
    use std::rc::Weak;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Occupying {
        pub area: DynamicEntityRef,
    }

    impl Scope for Occupying {
        fn scope_key() -> &'static str {
            "occupying"
        }
    }

    impl PrepareWithInfrastructure for Occupying {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            self.area = infra.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Occupyable {
        pub acls: Acls,
        pub occupied: Vec<DynamicEntityRef>,
        pub occupancy: u32,
    }

    impl Scope for Occupyable {
        fn scope_key() -> &'static str {
            "occupyable"
        }
    }

    impl PrepareWithInfrastructure for Occupyable {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            self.occupied = self
                .occupied
                .iter()
                .map(|r| infra.ensure_entity(r).unwrap())
                .collect();
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Exit {
        pub area: DynamicEntityRef,
    }

    impl Scope for Exit {
        fn scope_key() -> &'static str {
            "exit"
        }
    }

    impl PrepareWithInfrastructure for Exit {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            self.area = infra.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AreaRoute {
        pub area: DynamicEntityRef,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Movement {
        pub routes: Vec<AreaRoute>,
    }

    impl Scope for Movement {
        fn scope_key() -> &'static str {
            "movement"
        }
    }

    impl PrepareWithInfrastructure for Movement {
        fn prepare_with(&mut self, infra: &Weak<dyn Infrastructure>) -> Result<()> {
            let infra = infra.upgrade().ok_or(DomainError::NoInfrastructure)?;
            for route in self.routes.iter_mut() {
                route.area = infra.ensure_entity(&route.area)?;
            }
            Ok(())
        }
    }

    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(occupyable) = source.scope::<Occupyable>() {
            // Pretty sure this clone should be unnecessary.
            entity_keys.extend(occupyable.occupied.iter().map(|er| er.key().clone()));
        }
        if let Ok(movement) = source.scope::<Movement>() {
            for route in movement.routes {
                entity_keys.push(route.area.into());
            }
        }
        Ok(())
    }
}

pub mod actions {
    use super::*;
    use tracing::info;

    #[derive(Debug)]
    struct GoAction {}

    impl Action for GoAction {
        fn perform(&self, (_world, _user, _area): ActionArgs) -> ReplyResult {
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
