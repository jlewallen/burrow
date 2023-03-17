use crate::plugins::library::plugin::*;

pub struct MovingPlugin {}

impl Plugin for MovingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "moving"
    }
}

impl ParsesActions for MovingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::GoActionParser {}, i)
    }
}

pub mod model {
    use crate::{
        domain::{HookOutcome, Hooks},
        plugins::{library::model::*, looking::model::Observe},
    };

    pub trait BeforeMovingHook {
        fn before_moving(&self, surroundings: &Surroundings, to: Entity) -> Result<CanMove>;
    }

    pub trait AfterMoveHook {
        fn after_move(&self, surroundings: &Surroundings, from: Entity) -> Result<()>;
    }

    #[derive(Default)]
    pub struct MovingHooks {
        pub before_moving: Hooks<Box<dyn BeforeMovingHook>>,
        pub after_move: Hooks<Box<dyn AfterMoveHook>>,
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

    #[derive(Debug)]
    pub enum MovingEvent {
        Left { living: Entry, area: Entry },
        Arrived { living: Entry, area: Entry },
    }

    impl DomainEvent for MovingEvent {
        fn audience(&self) -> Audience {
            match self {
                Self::Left { living: _, area } => Audience::Area(area.clone()),
                Self::Arrived { living: _, area } => Audience::Area(area.clone()),
            }
        }

        fn observe(&self, user: &Entry) -> Result<Box<dyn Observed>> {
            Ok(match self {
                Self::Left {
                    living,
                    area: _area,
                } => Box::new(SimpleObservation::new(
                    json!({ "left": { "living": living.observe(user)?}}),
                )),
                Self::Arrived {
                    living,
                    area: _area,
                } => Box::new(SimpleObservation::new(
                    json!({ "arrived": { "living": living.observe(user)?}}),
                )),
            })
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            self.area = infra.ensure_entity(&self.area)?;
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
            self.occupied.retain(|i| i.key != item.key());
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            self.occupied = self
                .occupied
                .iter()
                .map(|r| infra.ensure_entity(r).unwrap())
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            self.area = infra.ensure_entity(&self.area)?;
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
        fn supply(&mut self, infra: &SessionRef) -> Result<()> {
            for route in self.routes.iter_mut() {
                route.area = infra.ensure_entity(&route.area)?;
            }
            Ok(())
        }
    }

    pub fn discover(source: &Entry, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(occupyable) = source.scope::<Occupyable>() {
            // Pretty sure this clone should be unnecessary.
            entity_keys.extend(occupyable.occupied.iter().map(|er| er.key.clone()));
        }
        if let Ok(movement) = source.scope::<Movement>() {
            for route in &movement.routes {
                entity_keys.push(route.area.key.clone());
            }
        }
        Ok(())
    }
}

pub mod actions {
    use crate::plugins::library::actions::*;
    use crate::plugins::looking::actions::*;
    use crate::plugins::moving::model::MovingEvent;

    #[derive(Debug)]
    pub struct GoAction {
        pub item: Item,
    }

    impl Action for GoAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("go {:?}!", self.item);

            let (_, living, area, infra) = args.unpack();

            match infra.find_item(args, &self.item)? {
                Some(to_area) => match tools::navigate_between(&area, &to_area, &living)? {
                    DomainOutcome::Ok => {
                        get_my_session()?.raise(Box::new(MovingEvent::Left {
                            living: living.clone(),
                            area,
                        }))?;
                        get_my_session()?.raise(Box::new(MovingEvent::Arrived {
                            living: living.clone(),
                            area: to_area,
                        }))?;

                        infra.chain(&living, Box::new(LookAction {}))
                    }
                    DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                },
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }
}

mod parser {
    use crate::plugins::library::parser::*;

    use super::actions::GoAction;

    pub struct GoActionParser {}

    impl ParsesActions for GoActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                separated_pair(tag("go"), spaces, named_place),
                |(_, target)| GoAction { item: target },
            )(i)?;

            Ok(Box::new(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use super::*;
    use crate::{
        domain::{BuildActionArgs, QuickThing},
        plugins::{looking::model::new_area_observation, tools},
    };

    #[test]
    fn it_goes_ignores_bad_matches() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let east = build.make(QuickThing::Place("East Place"))?;
        let west = build.make(QuickThing::Place("West Place"))?;
        let args: ActionArgs = build
            .route("East", QuickThing::Actual(east))
            .route("Wast", QuickThing::Actual(west))
            .try_into()?;

        let action = try_parsing(GoActionParser {}, "go north")?;
        let reply = action.perform(args.clone())?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_goes_through_correct_route_when_two_nearby() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let east = build.make(QuickThing::Place("East Place"))?;
        let west = build.make(QuickThing::Place("West Place"))?;
        let args: ActionArgs = build
            .route("East", QuickThing::Actual(east.clone()))
            .route("Wast", QuickThing::Actual(west))
            .try_into()?;

        let action = try_parsing(GoActionParser {}, "go east")?;
        let reply = action.perform(args.clone())?;
        let (_, living, area, _) = args.unpack();

        assert_eq!(
            reply.to_json()?,
            new_area_observation(&living, &east)?.to_json()?
        );

        assert_ne!(tools::area_of(&living)?.key(), area.key());
        assert_eq!(tools::area_of(&living)?.key(), east.key());

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_goes_through_routes_when_one_nearby() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let destination = build.make(QuickThing::Place("Place"))?;
        let args: ActionArgs = build
            .route("East", QuickThing::Actual(destination.clone()))
            .try_into()?;

        let action = try_parsing(GoActionParser {}, "go east")?;
        let reply = action.perform(args.clone())?;
        let (_, living, area, _) = args.unpack();

        assert_eq!(
            reply.to_json()?,
            new_area_observation(&living, &destination)?.to_json()?
        );

        assert_ne!(tools::area_of(&living)?.key(), area.key());
        assert_eq!(tools::area_of(&living)?.key(), destination.key());

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_go_unknown_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build.plain().try_into()?;

        let action = try_parsing(GoActionParser {}, "go rake")?;
        let reply = action.perform(args.clone())?;
        let (_, _person, _area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        build.close()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_go_non_routes() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Rake")])
            .try_into()?;

        let action = try_parsing(GoActionParser {}, "go rake")?;
        let reply = action.perform(args.clone())?;
        let (_, _person, _area, _) = args.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        build.close()?;

        Ok(())
    }
}
