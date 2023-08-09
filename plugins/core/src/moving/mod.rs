use crate::library::plugin::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct MovingPluginFactory {}

impl PluginFactory for MovingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(MovingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MovingPlugin {}

impl Plugin for MovingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "moving"
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

impl ParsesActions for MovingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::GoActionParser {}, i)
    }
}

pub mod model {
    use crate::library::model::*;

    pub trait BeforeMovingHook {
        fn before_moving(&self, surroundings: &Surroundings, to_area: &Entry) -> Result<CanMove>;
    }

    impl BeforeMovingHook for MovingHooks {
        fn before_moving(&self, surroundings: &Surroundings, to_area: &Entry) -> Result<CanMove> {
            Ok(self
                .before_moving
                .instances
                .borrow()
                .iter()
                .map(|h| h.before_moving(surroundings, to_area))
                .collect::<Result<Vec<CanMove>>>()?
                .iter()
                .fold(CanMove::default(), |c, h| c.fold(h)))
        }
    }

    pub trait AfterMoveHook {
        fn after_move(&self, surroundings: &Surroundings, from_area: &Entry) -> Result<()>;
    }

    impl AfterMoveHook for MovingHooks {
        fn after_move(&self, surroundings: &Surroundings, from_area: &Entry) -> Result<()> {
            self.after_move
                .instances
                .borrow()
                .iter()
                .map(|h| h.after_move(surroundings, from_area))
                .collect::<Result<Vec<()>>>()?;

            Ok(())
        }
    }

    #[derive(Default)]
    pub struct MovingHooks {
        pub before_moving: Hooks<Box<dyn BeforeMovingHook>>,
        pub after_move: Hooks<Box<dyn AfterMoveHook>>,
    }

    impl HooksSet for MovingHooks {
        fn hooks_key() -> &'static str
        where
            Self: Sized,
        {
            "moving"
        }
    }

    #[derive(Clone, Default)]
    pub enum CanMove {
        #[default]
        Allow,
        Prevent,
    }

    impl HookOutcome for CanMove {
        fn fold(&self, other: &Self) -> Self {
            match (self, other) {
                (_, CanMove::Prevent) => CanMove::Prevent,
                (CanMove::Prevent, _) => CanMove::Prevent,
                (_, _) => CanMove::Allow,
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Occupying {
        pub area: EntityRef,
    }

    impl Scope for Occupying {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "occupying"
        }
    }

    impl Needs<SessionRef> for Occupying {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.area = session.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Occupyable {
        pub acls: Acls,
        pub occupied: Vec<EntityRef>,
        pub occupancy: u32,
    }

    impl Occupyable {
        pub fn stop_occupying(&mut self, item: &Entry) -> Result<DomainOutcome> {
            let before = self.occupied.len();
            self.occupied.retain(|i| *i.key() != *item.key());
            let after = self.occupied.len();
            if before == after {
                return Ok(DomainOutcome::Nope);
            }

            Ok(DomainOutcome::Ok)
        }

        pub fn start_occupying(&mut self, item: &Entry) -> Result<DomainOutcome> {
            self.occupied.push(item.try_into()?);

            Ok(DomainOutcome::Ok)
        }
    }

    impl Scope for Occupyable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "occupyable"
        }
    }

    impl Needs<SessionRef> for Occupyable {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.occupied = self
                .occupied
                .iter()
                .map(|r| session.ensure_entity(r).unwrap())
                .collect();
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Exit {
        pub area: EntityRef,
    }

    impl Scope for Exit {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "exit"
        }
    }

    impl Needs<SessionRef> for Exit {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.area = session.ensure_entity(&self.area)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AreaRoute {
        pub area: EntityRef,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Movement {
        pub routes: Vec<AreaRoute>,
    }

    impl Scope for Movement {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "movement"
        }
    }

    impl Needs<SessionRef> for Movement {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            for route in self.routes.iter_mut() {
                route.area = session.ensure_entity(&route.area)?;
            }
            Ok(())
        }
    }
}

pub mod actions {
    use crate::library::actions::*;
    use crate::looking::actions::*;
    use crate::looking::model::Observe;
    use crate::moving::model::{AfterMoveHook, BeforeMovingHook, CanMove, MovingHooks};

    #[action]
    pub struct GoAction {
        pub item: Item,
    }

    impl Action for GoAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("go {:?}!", self.item);

            let (_, living, area) = surroundings.unpack();

            match session.find_item(surroundings, &self.item)? {
                Some(to_area) => {
                    let can = session.hooks().invoke::<MovingHooks, CanMove, _>(|h| {
                        h.before_moving(surroundings, &to_area)
                    })?;

                    match can {
                        CanMove::Allow => {
                            match tools::navigate_between(&area, &to_area, &living)? {
                                DomainOutcome::Ok => {
                                    session.hooks().invoke::<MovingHooks, (), _>(|h| {
                                        h.after_move(surroundings, &area)
                                    })?;

                                    session.raise(
                                        Audience::Area(area.key().clone()),
                                        Box::new(MovingEvent::Left {
                                            living: (&living)
                                                .observe(&living)?
                                                .expect("No observed entity"),
                                            area: (&area)
                                                .observe(&living)?
                                                .expect("No observed entity"),
                                        }),
                                    )?;
                                    session.raise(
                                        Audience::Area(to_area.key().clone()),
                                        Box::new(MovingEvent::Arrived {
                                            living: (&living)
                                                .observe(&living)?
                                                .expect("No observed entity"),
                                            area: (&to_area)
                                                .observe(&living)?
                                                .expect("No observed entity"),
                                        }),
                                    )?;

                                    session.perform(Perform::Living {
                                        living,
                                        action: PerformAction::Instance(Rc::new(LookAction {})),
                                    })
                                }
                                DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                            }
                        }
                        CanMove::Prevent => Ok(SimpleReply::Prevented.into()),
                    }
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }
}

mod parser {
    use crate::library::parser::*;

    use super::actions::GoAction;

    pub struct GoActionParser {}

    impl ParsesActions for GoActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                separated_pair(tag("go"), spaces, named_place),
                |(_, target)| GoAction { item: target },
            )(i)?;

            Ok(Some(Box::new(action)))
        }
    }
}