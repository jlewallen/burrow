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

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
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
    use thiserror::Error;

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
        fn observe(&self, user: &Entry) -> Result<Option<T>>;
    }

    impl Observe<ObservedEntity> for &Entry {
        fn observe(&self, _user: &Entry) -> Result<Option<ObservedEntity>> {
            let quantity = {
                let carryable = self.scope::<Carryable>()?;
                carryable.quantity()
            };
            let key = self.key().to_string();
            let myself = self.entity().borrow();
            let name = myself.name();
            let desc = myself.desc();
            let qualified = name.as_ref().map(|n| qualify_name(quantity, n));
            Ok(Some(ObservedEntity {
                key,
                name,
                qualified,
                desc,
            }))
        }
    }

    pub fn new_entity_observation(
        user: &Entry,
        entity: &Entry,
    ) -> Result<Option<EntityObservation>> {
        Ok(entity
            .observe(user)?
            .map(|entity| EntityObservation { entity }))
    }

    pub fn new_inside_observation(
        user: &Entry,
        vessel: &Entry,
    ) -> Result<Option<InsideObservation>> {
        let mut items = Vec::new();
        if let Ok(containing) = vessel.scope::<Containing>() {
            for lazy_entity in &containing.holding {
                let entity = &lazy_entity.to_entry()?;
                items.push(entity.observe(user)?);
            }
        }

        Ok(vessel.observe(user)?.map(|vessel| InsideObservation {
            vessel,
            items: items.into_iter().flatten().collect(),
        }))
    }

    pub fn new_area_observation(user: &Entry, area: &Entry) -> Result<AreaObservation> {
        let mut living: Vec<ObservedEntity> = vec![];
        if let Ok(occupyable) = area.scope::<Occupyable>() {
            for entity in &occupyable.occupied {
                if let Some(observed) = (&entity.to_entry()?).observe(user)? {
                    living.push(observed);
                }
            }
        }

        let mut items = vec![];
        if let Ok(containing) = area.scope::<Containing>() {
            for entity in &containing.holding {
                items.push((&entity.to_entry()?).observe(user)?);
            }
        }

        let mut carrying = vec![];
        if let Ok(containing) = user.scope::<Containing>() {
            for entity in &containing.holding {
                carrying.push((&entity.to_entry()?).observe(user)?);
            }
        }

        let mut routes = vec![];
        if let Ok(movement) = user.scope::<Movement>() {
            for route in &movement.routes {
                routes.push((&route.area.to_entry()?).observe(user)?);
            }
        }

        Ok(AreaObservation {
            area: area
                .observe(user)?
                .ok_or(LookError::InvisibleSurroundingArea)?,
            person: user.observe(user)?.ok_or(LookError::InvisibleSelf)?,
            living,
            items: items.into_iter().flatten().collect(),
            carrying: carrying.into_iter().flatten().collect(),
            routes: routes.into_iter().flatten().collect(),
        })
    }

    #[derive(Error, Debug)]
    pub enum LookError {
        #[error("Invisible surrounding area")]
        InvisibleSurroundingArea,
        #[error("Invisible self")]
        InvisibleSelf,
    }
}

pub mod actions {
    use anyhow::Context;

    use super::model::*;
    use crate::library::actions::*;

    #[action]
    pub struct LookAction {}

    impl Action for LookAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, user, area) = surroundings.unpack();

            Ok(new_area_observation(&user, &area)
                .with_context(|| "Observing area")?
                .into())
        }
    }

    #[action]
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
                        match new_inside_observation(&user, &target)? {
                            Some(observation) => Ok(observation.into()),
                            None => Ok(SimpleReply::NotFound.into()),
                        }
                    } else {
                        Ok(SimpleReply::Impossible.into())
                    }
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
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
                Some(target) => match new_entity_observation(&user, &target)? {
                    Some(observation) => Ok(observation.into()),
                    None => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
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

            Ok(Some(action))
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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_look_inside_non_containers() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Object("Not A Box")]).build()?;

        let action = try_parsing(LookActionParser {}, "look inside box")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_look_at_not_found_entities() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build.entity()?.named("Hammer")?.into_entry()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

        let action = try_parsing(LookActionParser {}, "look at shovel")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_looks_at_entities() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let vessel = build.entity()?.named("Hammer")?.into_entry()?;
        let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

        let action = try_parsing(LookActionParser {}, "look at hammer")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_tagged_json()?);

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
