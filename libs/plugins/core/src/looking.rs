use crate::library::plugin::*;

#[derive(Default)]
pub struct LookingPluginFactory {}

impl PluginFactory for LookingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(LookingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct LookingPlugin {}

impl Plugin for LookingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "looking"
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for LookingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::LookActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;
    use crate::{
        carrying::model::{Carryable, Containing},
        moving::model::{Movement, Occupyable},
    };

    pub fn qualify_name(quantity: f32, name: &str) -> String {
        use indefinite::*;
        use inflection::*;
        if quantity > 1.0 {
            let pluralized = plural::<_, String>(name);
            format!("{} {}", quantity, &pluralized)
        } else {
            indefinite(name)
        }
    }

    pub trait ObserveHook<T> {
        fn observe(
            &self,
            surroundings: &Surroundings,
            user: &Entry,
            target: &Entry,
        ) -> Result<Option<T>>;
    }

    pub trait Observe<T> {
        fn observe(&self, user: &Entry) -> Result<T>;
    }

    impl Observe<ObservedEntity> for &Entry {
        fn observe(&self, _user: &Entry) -> Result<ObservedEntity> {
            let name = self.name()?;
            let carryable = self.scope::<Carryable>()?;
            let qualified = name.as_ref().map(|n| qualify_name(carryable.quantity(), n));

            Ok(ObservedEntity {
                key: self.key().to_string(),
                name,
                qualified,
                desc: self.desc()?,
            })
        }
    }

    pub fn new_entity_observation(user: &Entry, entity: &Entry) -> Result<EntityObservation> {
        Ok(EntityObservation {
            entity: entity.observe(user)?,
        })
    }

    pub fn new_inside_observation(user: &Entry, vessel: &Entry) -> Result<InsideObservation> {
        let mut items = vec![];
        if let Ok(containing) = vessel.scope::<Containing>() {
            for lazy_entity in &containing.holding {
                let entity = &lazy_entity.into_entry()?;
                items.push(entity.observe(user)?);
            }
        }

        Ok(InsideObservation {
            vessel: vessel.observe(user)?,
            items,
        })
    }

    pub fn new_area_observation(user: &Entry, area: &Entry) -> Result<AreaObservation> {
        let mut living: Vec<ObservedEntity> = vec![];
        if let Ok(occupyable) = area.scope::<Occupyable>() {
            for entity in &occupyable.occupied {
                living.push((&entity.into_entry()?).observe(user)?);
            }
        }

        let mut items = vec![];
        if let Ok(containing) = area.scope::<Containing>() {
            for entity in &containing.holding {
                items.push((&entity.into_entry()?).observe(user)?);
            }
        }

        let mut carrying = vec![];
        if let Ok(containing) = user.scope::<Containing>() {
            for entity in &containing.holding {
                carrying.push((&entity.into_entry()?).observe(user)?);
            }
        }

        let mut routes = vec![];
        if let Ok(movement) = user.scope::<Movement>() {
            for route in &movement.routes {
                routes.push((&route.area.into_entry()?).observe(user)?);
            }
        }

        Ok(AreaObservation {
            area: area.observe(user)?,
            person: user.observe(user)?,
            living,
            items,
            carrying,
            routes,
        })
    }
}

pub mod actions {
    use anyhow::Context;

    use super::model::*;
    use crate::library::actions::*;

    #[derive(Debug)]
    pub struct LookAction {}

    impl Action for LookAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, user, area) = surroundings.unpack();

            Ok(Box::new(
                new_area_observation(&user, &area).with_context(|| "Observing area")?,
            ))
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

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, user, _area) = surroundings.unpack();

            match session.find_item(surroundings, &self.item)? {
                Some(target) => {
                    if tools::is_container(&target)? {
                        Ok(Box::new(new_inside_observation(&user, &target)?))
                    } else {
                        Ok(Box::new(SimpleReply::Impossible))
                    }
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct LookAtAction {
        pub item: Item,
    }

    impl Action for LookAtAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, user, _area) = surroundings.unpack();

            match session.find_item(surroundings, &self.item)? {
                Some(target) => Ok(Box::new(new_entity_observation(&user, &target)?)),
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }
}

pub mod parser {
    use crate::library::parser::*;

    use super::actions::{LookAction, LookAtAction, LookInsideAction};

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

            let at = map(
                separated_pair(separated_pair(tag("look"), spaces, tag("at")), spaces, noun),
                |(_, nearby)| Box::new(LookAtAction { item: nearby }) as Box<dyn Action>,
            );

            let area = map(tag("look"), |_| Box::new(LookAction {}) as Box<dyn Action>);

            let (_, action) = alt((inside, at, area))(i)?;

            Ok(action)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::model::*;
    use super::parser::LookActionParser;
    use super::*;
    use crate::{library::plugin::try_parsing, BuildSurroundings, QuickThing};

    #[test]
    fn it_looks_in_empty_area() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_in_area_with_items_on_ground() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .build()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_in_area_with_items_on_ground_and_a_route() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let destination = build.make(QuickThing::Place("Place"))?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .route("East Exit", QuickThing::Actual(destination))
            .build()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_in_area_with_items_on_ground_and_holding_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let destination = build.make(QuickThing::Place("Place"))?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Boring Shovel")])
            .hands(vec![QuickThing::Object("Cool Rake")])
            .route("East Exit", QuickThing::Actual(destination))
            .build()?;

        let action = try_parsing(LookActionParser {}, "look")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_look_inside_non_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Object("Not A Box")]).build()?;

        let action = try_parsing(LookActionParser {}, "look inside box")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_inside_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build
            .entity()?
            .named("Vessel")?
            .holding(&vec![build.make(QuickThing::Object("Key"))?])?
            .into_entry()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

        let action = try_parsing(LookActionParser {}, "look inside vessel")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_look_at_not_found_entities() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build.entity()?.named("Hammer")?.into_entry()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

        let action = try_parsing(LookActionParser {}, "look at shovel")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_at_entities() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build.entity()?.named("Hammer")?.into_entry()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

        let action = try_parsing(LookActionParser {}, "look at hammer")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn qualify_name_basics() {
        // Not going to test all of indefinite's behavior here, just build edge
        // cases in our integrating logic.
        assert_eq!(qualify_name(1.0, "box"), "a box");
        assert_eq!(qualify_name(2.0, "box"), "2 boxes");
        assert_eq!(qualify_name(1.0, "person"), "a person");
        assert_eq!(qualify_name(2.0, "person"), "2 people");
        assert_eq!(qualify_name(1.0, "orange"), "an orange");
        assert_eq!(qualify_name(2.0, "orange"), "2 oranges");
        assert_eq!(qualify_name(1.0, "East Exit"), "an East Exit");
    }
}
