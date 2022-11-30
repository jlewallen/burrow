use crate::plugins::library::plugin::*;

pub struct LookingPlugin {}

impl ParsesActions for LookingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LookActionParser {}, i)
    }
}

pub mod model {
    use shared_replies::InsideObservation;

    use crate::plugins::library::model::*;
    use crate::{
        plugins::carrying::model::Containing,
        plugins::moving::model::{Movement, Occupyable},
    };

    trait Observe<T> {
        fn observe(&self, user: &EntityPtr) -> Result<T>;
    }

    impl Observe<ObservedEntity> for &EntityPtr {
        fn observe(&self, _user: &EntityPtr) -> Result<ObservedEntity> {
            let e = self.borrow();
            Ok(ObservedEntity {
                key: e.key.to_string(),
                name: e.name(),
                desc: e.desc(),
            })
        }
    }

    pub fn new_inside_observation(
        user: &EntityPtr,
        vessel: &EntityPtr,
    ) -> Result<InsideObservation> {
        let mut items = vec![];
        if let Ok(containing) = vessel.borrow().scope::<Containing>() {
            for lazy_entity in &containing.holding {
                let entity = &lazy_entity.into_entity()?;
                items.push(entity.observe(user)?);
            }
        }

        Ok(InsideObservation {
            vessel: vessel.observe(user)?,
            items,
        })
    }

    pub fn new_area_observation(user: &EntityPtr, area: &EntityPtr) -> Result<AreaObservation> {
        // I feel like there's a lot of unnecessary copying going on here.

        let mut living: Vec<ObservedEntity> = vec![];
        if let Ok(occupyable) = area.borrow().scope::<Occupyable>() {
            for entity in &occupyable.occupied {
                living.push((&entity.into_entity()?).observe(user)?);
            }
        }

        let mut items = vec![];
        if let Ok(containing) = area.borrow().scope::<Containing>() {
            for entity in &containing.holding {
                items.push((&entity.into_entity()?).observe(user)?);
            }
        }

        let mut carrying = vec![];
        if let Ok(containing) = user.borrow().scope::<Containing>() {
            for entity in &containing.holding {
                carrying.push((&entity.into_entity()?).observe(user)?);
            }
        }

        let mut routes = vec![];
        if let Ok(movement) = user.borrow().scope::<Movement>() {
            for route in &movement.routes {
                routes.push((&route.area.into_entity()?).observe(user)?);
            }
        };

        Ok(AreaObservation {
            area: area.observe(user)?,
            person: user.observe(user)?,
            living,
            items,
            carrying,
            routes,
        })
    }

    pub fn discover(_source: &Entity, _entity_keys: &mut [EntityKey]) -> Result<()> {
        Ok(())
    }
}

pub mod actions {
    use super::model::*;
    use crate::plugins::library::actions::*;

    #[derive(Debug)]
    pub struct LookAction {}

    impl Action for LookAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, (_world, user, area, _infra): ActionArgs) -> ReplyResult {
            info!("look!");

            Ok(Box::new(new_area_observation(&user, &area)?))
        }
    }

    #[derive(Debug)]
    pub struct LookInsideAction {
        pub item: Item,
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
                        Ok(Box::new(new_inside_observation(&user, &target)?))
                    } else {
                        Ok(Box::new(SimpleReply::Impossible))
                    }
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    use super::actions::{LookAction, LookInsideAction};

    pub struct LookActionParser {}

    impl ParsesActions for LookActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let inside = map(
                separated_pair(
                    separated_pair(tag("look"), spaces, tag("inside")),
                    spaces,
                    noun,
                ),
                |(_, nearby)| Box::new(LookInsideAction { item: nearby }) as Box<dyn Action>,
            );

            let area = map(tag("look"), |_| Box::new(LookAction {}) as Box<dyn Action>);

            let (_, action) = alt((inside, area))(i)?;

            Ok(action)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::LookActionParser;
    use super::*;
    use crate::{
        domain::{BuildActionArgs, QuickThing},
        plugins::library::plugin::try_parsing,
    };

    #[test]
    fn it_looks_in_empty_area() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build.plain().try_into()?;

        let action = try_parsing(LookActionParser {}, "look")?;
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
            .ground(vec![QuickThing::Object("Cool Rake")])
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .try_into()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(args.clone())?;
        let (_, _person, _area, _) = args.clone();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_in_area_with_items_on_ground_and_a_route() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let destination = build.make(QuickThing::Place("Place"))?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .route("East Exit", QuickThing::Actual(destination))
            .try_into()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(args.clone())?;
        let (_, _person, _area, _) = args.clone();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_in_area_with_items_on_ground_and_holding_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let destination = build.make(QuickThing::Place("Place"))?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .hands(vec![QuickThing::Object("Cool Rake")])
            .route("East Exit", QuickThing::Actual(destination))
            .try_into()?;

        let action = try_parsing(LookActionParser {}, "look")?;
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
            .hands(vec![QuickThing::Object("Not A Box")])
            .try_into()?;

        let action = try_parsing(LookActionParser {}, "look inside box")?;
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
            .holding(&vec![build.make(QuickThing::Object("Key"))?])?
            .into_entity()?;
        let args: ActionArgs = build.hands(vec![QuickThing::Actual(vessel)]).try_into()?;

        let action = try_parsing(LookActionParser {}, "look inside vessel")?;
        let reply = action.perform(args.clone())?;
        let (_, _person, _area, _) = args.clone();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }
}
