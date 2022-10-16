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
    use crate::{
        kernel::*, plugins::carrying::model::Containing, plugins::moving::model::Occupyable,
    };
    use anyhow::Result;

    pub fn discover(_source: &Entity, _entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        Ok(())
    }

    #[derive(Debug)]
    pub struct ObservedArea {}

    impl From<&Entity> for Option<ObservedArea> {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug)]
    pub struct ObservedPerson {}

    impl From<&Entity> for Option<ObservedPerson> {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug)]
    pub struct ObservedEntity {}

    impl From<&Entity> for Option<ObservedEntity> {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug)]
    pub struct ObservedRoute {}

    impl From<&Entity> for Option<ObservedRoute> {
        fn from(_value: &Entity) -> Self {
            todo!()
        }
    }

    #[derive(Debug)]
    pub struct AreaObservation {
        pub area: Entity,
        pub person: Entity,
        pub living: Vec<Entity>,
        pub items: Vec<Entity>,
        pub holding: Vec<Entity>,
        pub routes: Vec<Entity>,
    }

    impl AreaObservation {
        pub fn new(user: &Entity, area: &Entity) -> Self {
            let living: Vec<Entity> =
                if let Ok(occupyable) = <&Entity as TryInto<Box<Occupyable>>>::try_into(user) {
                    vec![]
                } else {
                    vec![]
                };
            let items: Vec<Entity> =
                if let Ok(containing) = <&Entity as TryInto<Box<Containing>>>::try_into(user) {
                    vec![] // containing.holding.to_vec()
                } else {
                    vec![]
                };

            AreaObservation {
                area: area.clone(),
                person: user.clone(),
                living: living,
                items: items,
                holding: vec![],
                routes: vec![],
            }
            /*
            class AreaObservation(Observation):
                area: Entity
                person: Entity
                living: List[ObservedLiving]
                items: List[ObservedEntity]
                holding: List[ObservedEntity]
                routes: List[movement.AreaRoute]

                @staticmethod
                async def create(area: Entity, person: Entity) -> "AreaObservation":
                    occupied = area.make(occupyable.Occupyable).occupied

                    living: List[ObservedLiving] = flatten(
                        [await observe_entity(e) for e in occupied if e != person]
                    )

                    items: List[ObservedEntity] = flatten(
                        [
                            await observe_entity(e)
                            for e in area.make(carryable.Containing).holding
                            if not e.make(mechanics.Visibility).visible.hard_to_see
                            or person.make(mechanics.Visibility).can_see(e.identity)
                        ]
                    )

                    routes: List[movement.AreaRoute] = area.make(movement.Movement).available_routes

                    holding = flatten([await observe_entity(e) for e in tools.get_holding(person)])

                    return AreaObservation(area, person, living, items, holding, routes)
            */
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
    use anyhow::Result;
    use tracing::info;

    #[derive(Debug)]
    struct LookAction {}
    impl Action for LookAction {
        fn perform(&self, (_world, user, area): ActionArgs) -> Result<Box<dyn Reply>> {
            info!("look!");

            Ok(Box::new(AreaObservation::new(user, area)))
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
