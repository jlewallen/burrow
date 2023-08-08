use crate::library::plugin::*;

#[cfg(test)]
mod tests;

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

    pub enum Unqualified<'a> {
        Quantity(f32, &'a str),
        Living(&'a str),
    }

    impl<'a> Unqualified<'a> {
        pub fn qualify(self) -> String {
            match self {
                Unqualified::Quantity(quantity, name) => {
                    use indefinite::*;
                    use inflection::*;
                    if quantity > 1.0 {
                        let pluralized = plural::<_, String>(name);
                        format!("{} {}", quantity, &pluralized)
                    } else {
                        indefinite(name)
                    }
                }
                Unqualified::Living(name) => name.to_owned(),
            }
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
                if let Some(carryable) = self.maybe_scope::<Carryable>()? {
                    Some(carryable.quantity())
                } else {
                    None
                }
            };
            let key = self.key().to_string();
            let observing = self.entity().borrow();
            let name = observing.name();
            let desc = observing.desc();
            let qualified = name
                .as_ref()
                .map(|n| match quantity {
                    Some(quantity) => Unqualified::Quantity(quantity, n),
                    None => Unqualified::Living(n),
                })
                .map(|u| u.qualify());
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
