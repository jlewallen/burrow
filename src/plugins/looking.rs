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
    Ok(parse(i).map(|(_, sentence)| actions::evaluate(&sentence))?)
}

pub mod model {
    use std::{cell::RefCell, ops::Deref, rc::Rc};

    use crate::{
        kernel::*,
        plugins::carrying::model::Containing,
        plugins::moving::model::{Movement, Occupyable},
    };
    use anyhow::Result;
    use serde::Serialize;

    pub fn discover(_source: &Entity, _entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        Ok(())
    }

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
    impl TryFrom<&DynamicEntityRef> for ObservedEntity {
        type Error = DomainError;

        fn try_from(value: &DynamicEntityRef) -> Result<Self, Self::Error> {
            match value {
                DynamicEntityRef::Entity(re) => {
                    let e: Rc<RefCell<Entity>> = re.into_entity()?;
                    let e = e.borrow();

                    Ok(Self {
                        key: e.key.clone(),
                        name: e.name(),
                        desc: e.desc(),
                    })
                }
                _ => Err(DomainError::DanglingEntity),
            }
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

    impl ToJson for AreaObservation {
        fn to_json(&self) -> Result<String> {
            Ok(serde_json::to_string(self)?)
        }
    }

    impl AreaObservation {
        pub fn new(user: EntityPtr, area: EntityPtr) -> Result<Self> {
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
                living: living,
                items: items,
                carrying: carrying,
                routes: routes,
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
}

pub mod actions {
    use super::model::*;
    use super::*;
    use tracing::info;

    #[derive(Debug)]
    struct LookAction {}
    impl Action for LookAction {
        fn perform(&self, (_world, user, area, _infra): ActionArgs) -> ReplyResult {
            info!("look!");

            Ok(Box::new(AreaObservation::new(user, area)?))
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
        assert!(output.is_err()); // TODO Weak assertion.
    }
}
