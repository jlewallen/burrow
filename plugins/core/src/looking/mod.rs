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
    use crate::tools;
    use crate::{
        carrying::model::{Carryable, Containing},
        moving::model::Occupyable,
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
            user: &EntityPtr,
            target: &EntityPtr,
        ) -> Result<Option<T>>;
    }

    fn observe_all(
        entries: Option<Vec<EntityPtr>>,
        user: &EntityPtr,
    ) -> Result<Option<Vec<ObservedEntity>>> {
        let Some(observing) = entries else {
            return Ok(None)
        };

        let mut observed = Vec::new();
        for e in &observing {
            if let Some(e) = e.observe(user)? {
                observed.push(e);
            }
        }
        Ok(Some(observed))
    }

    pub trait Observe<T> {
        fn observe(&self, user: &EntityPtr) -> Result<Option<T>>;
    }

    impl Observe<ObservedEntity> for &EntityPtr {
        fn observe(&self, _user: &EntityPtr) -> Result<Option<ObservedEntity>> {
            let quantity = self.scope::<Carryable>()?.map(|c| c.quantity());
            let key = self.key().to_string();
            let gid = self.gid().into();
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
                gid,
                name,
                qualified,
                desc,
            }))
        }
    }

    pub fn new_entity_observation(
        user: &EntityPtr,
        entity: &EntityPtr,
    ) -> Result<Option<EntityObservation>> {
        let wearing = observe_all(tools::worn_by(entity)?, user)?;
        Ok(entity
            .observe(user)?
            .map(|entity| EntityObservation { entity, wearing }))
    }

    pub fn new_inside_observation(
        user: &EntityPtr,
        vessel: &EntityPtr,
    ) -> Result<Option<InsideObservation>> {
        let mut items = Vec::new();
        if let Ok(Some(containing)) = vessel.scope::<Containing>() {
            for lazy_entity in &containing.holding {
                let entity = &lazy_entity.to_entity()?;
                items.push(entity.observe(user)?);
            }
        }

        Ok(vessel.observe(user)?.map(|vessel| InsideObservation {
            vessel,
            items: items.into_iter().flatten().collect(),
        }))
    }

    pub fn new_area_observation(user: &EntityPtr, area: &EntityPtr) -> Result<AreaObservation> {
        let mut living: Vec<ObservedEntity> = vec![];
        if let Ok(Some(occupyable)) = area.scope::<Occupyable>() {
            for entity in &occupyable.occupied {
                if let Some(observed) = (&entity.to_entity()?).observe(user)? {
                    living.push(observed);
                }
            }
        }

        let mut items = vec![];
        if let Ok(Some(containing)) = area.scope::<Containing>() {
            for entity in &containing.holding {
                items.push((&entity.to_entity()?).observe(user)?);
            }
        }

        let mut carrying = vec![];
        if let Ok(Some(containing)) = user.scope::<Containing>() {
            for entity in &containing.holding {
                carrying.push((&entity.to_entity()?).observe(user)?);
            }
        }

        let routes: Vec<ObservedEntity> = vec![];
        /*
        if let Ok(Some(movement)) = user.scope::<Movement>() {
            for route in &movement.routes {
                routes.push((&route.area.to_entity()?).observe(user)?);
            }
        }
        */

        Ok(AreaObservation {
            area: area
                .observe(user)?
                .ok_or(LookError::InvisibleSurroundingArea)?,
            person: user.observe(user)?.ok_or(LookError::InvisibleSelf)?,
            living,
            items: items.into_iter().flatten().collect(),
            carrying: carrying.into_iter().flatten().collect(),
            routes,
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
                .try_into()?)
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
                            Some(observation) => Ok(observation.try_into()?),
                            None => Ok(SimpleReply::NotFound.try_into()?),
                        }
                    } else {
                        Ok(SimpleReply::Impossible.try_into()?)
                    }
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
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
                    Some(observation) => Ok(observation.try_into()?),
                    None => Ok(SimpleReply::NotFound.try_into()?),
                },
                None => Ok(SimpleReply::NotFound.try_into()?),
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
                separated_pair(
                    separated_pair(tag("look"), spaces, tag("at")),
                    spaces,
                    noun_or_specific,
                ),
                |(_, nearby)| Box::new(LookAtAction { item: nearby }) as Box<dyn Action>,
            );

            let area = map(tag("look"), |_| Box::new(LookAction {}) as Box<dyn Action>);

            let (_, action) = alt((inside, at, area))(i)?;

            Ok(Some(action))
        }
    }
}
