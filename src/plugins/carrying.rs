use crate::kernel::*;
use crate::library::{noun, spaces};
use anyhow::Result;
use nom::{branch::alt, bytes::complete::tag, combinator::map, sequence::separated_pair, IResult};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Sentence {
    Hold(Item),
    Drop(Option<Item>),
}

fn hold(i: &str) -> IResult<&str, Sentence> {
    map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
        Sentence::Hold(target)
    })(i)
}

fn drop(i: &str) -> IResult<&str, Sentence> {
    let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
        Sentence::Drop(Some(target))
    });

    let everything = map(tag("drop"), |_| Sentence::Drop(None));

    alt((specific, everything))(i)
}

pub fn parse(i: &str) -> IResult<&str, Sentence> {
    alt((hold, drop))(i)
}

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    Ok(parse(i).map(|(_, sentence)| actions::evaluate(&sentence))?)
}

pub mod model {
    use crate::kernel::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug)]
    pub enum CarryingEvent {
        ItemHeld(EntityPtr),
        ItemDropped(EntityPtr),
    }

    impl DomainEvent for CarryingEvent {}

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Location {
        pub container: Option<LazyLoadedEntity>,
    }

    impl Scope for Location {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "location"
        }
    }

    impl Default for Location {
        fn default() -> Self {
            Self {
                container: Default::default(),
            }
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Location {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            self.container = infra.ensure_optional_entity(&self.container)?;
            Ok(())
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Containing {
        pub holding: Vec<LazyLoadedEntity>,
        pub capacity: Option<u32>,
        pub produces: HashMap<String, String>,
    }

    impl Scope for Containing {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "containing"
        }
    }

    impl Default for Containing {
        fn default() -> Self {
            Self {
                holding: Default::default(),
                capacity: Default::default(),
                produces: Default::default(),
            }
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Containing {
        fn supply(&mut self, infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            self.holding = self
                .holding
                .iter()
                .map(|r| infra.ensure_entity(r))
                .collect::<Result<Vec<_>>>()?;
            Ok(())
        }
    }

    impl Containing {
        pub fn hold(&mut self, item: EntityPtr) -> CarryingResult {
            self.holding.push(item.clone().into());

            Ok(DomainOutcome::Ok(vec![Box::new(CarryingEvent::ItemHeld(
                item,
            ))]))
        }

        pub fn stop_carrying(&mut self, item: EntityPtr) -> CarryingResult {
            let before = self.holding.len();
            self.holding.retain(|i| i.key != item.borrow().key);
            let after = self.holding.len();
            if before == after {
                return Ok(DomainOutcome::Nope);
            }

            Ok(DomainOutcome::Ok(vec![Box::new(
                CarryingEvent::ItemDropped(item),
            )]))
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Carryable {}

    impl Scope for Carryable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "carryable"
        }
    }

    impl Default for Carryable {
        fn default() -> Self {
            Self {}
        }
    }

    impl Needs<std::rc::Rc<dyn Infrastructure>> for Carryable {
        fn supply(&mut self, _infra: &std::rc::Rc<dyn Infrastructure>) -> Result<()> {
            Ok(())
        }
    }
    pub fn discover(source: &Entity, entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        if let Ok(containing) = source.scope::<Containing>() {
            // TODO Pretty sure this clone should be unnecessary, can we clone into from an iterator?
            entity_keys.extend(containing.holding.iter().map(|er| er.key.clone()))
        }
        Ok(())
    }
}

pub mod actions {
    use std::rc::Rc;

    use crate::plugins::carrying::model::Containing;

    use super::*;
    use tracing::info;

    #[derive(Debug)]
    struct HoldAction {
        item: Item,
    }

    impl Action for HoldAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("hold {:?}!", self.item);

            let (_, user, _, infra) = args.clone();
            let holding = infra.find_item(args, &self.item)?;

            match holding {
                Some(holding) => {
                    info!("holding {:?}!", holding);
                    let mut user = user.borrow_mut();
                    let mut containing = user.scope_mut::<Containing>()?;
                    let _ = containing.hold(holding);
                    containing.save()?;

                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    struct DropAction {
        maybe_item: Option<Item>,
    }

    impl Action for DropAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("drop {:?}!", self.maybe_item);

            let (_, user, area, infra) = args.clone();

            match &self.maybe_item {
                Some(item) => {
                    match infra.find_item(args, &item)? {
                        Some(dropping) => {
                            let mut user = user.borrow_mut();
                            let mut pockets = user.scope_mut::<Containing>()?;

                            // TODO Maybe the EntityPtr type becomes a wrapping
                            // struct and also knows the EntityKey that it
                            // points at.
                            info!("dropping {:?}!", dropping.borrow().key);

                            // I actually wanted to return the things that were
                            // actually dropped, felt cleaner. We don't need
                            // that for functionality, right now, though.
                            match pockets.stop_carrying(Rc::clone(&dropping))? {
                                DomainOutcome::Ok(_) => {
                                    let mut area = area.borrow_mut();
                                    let mut ground = area.scope_mut::<Containing>()?;

                                    ground.hold(dropping)?;

                                    pockets.save()?;
                                    ground.save()?;

                                    Ok(Box::new(SimpleReply::Done))
                                }
                                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
                            }
                        }
                        None => Ok(Box::new(SimpleReply::NotFound)),
                    }
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
        // TODO Look into this clone, perhaps other ways of cleaning this up.
        match s {
            Sentence::Hold(e) => Box::new(HoldAction { item: e.clone() }),
            Sentence::Drop(e) => Box::new(DropAction {
                maybe_item: e.clone(),
            }),
        }
    }

    pub struct Build {
        entity: EntityPtr,
    }

    impl Build {
        pub fn new(infra: &Rc<dyn Infrastructure>) -> Result<Self> {
            let entity = Entity::new();

            // TODO Would love to do this from `supply` except we only have
            // &self there instead of Rc<dyn Infrastructure>
            {
                let mut entity = entity.borrow_mut();
                entity.supply(infra)?;
            }

            infra.add_entity(&entity)?;

            Ok(Self { entity: entity })
        }

        pub fn named(&self, name: &str) -> Result<&Self> {
            let mut entity = self.entity.borrow_mut();

            entity.set_name(name)?;

            Ok(self)
        }

        pub fn holding(&self, item: &EntityPtr) -> Result<&Self> {
            let mut entity = self.entity.borrow_mut();
            let mut container = entity.scope_mut::<Containing>()?;

            container.hold(Rc::clone(item))?;
            container.save()?;

            Ok(self)
        }

        pub fn into_entity(&self) -> EntityPtr {
            Rc::clone(&self.entity)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::new_infra;
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

        fn get_infra() -> Result<Rc<dyn Infrastructure>> {
            Ok(new_infra()?)
        }

        fn log_test() {
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::new(
                    std::env::var("RUST_LOG")
                        .unwrap_or_else(|_| "rudder=info,tower_http=debug".into()),
                ))
                .with(tracing_subscriber::fmt::layer())
                .init();
        }

        #[test]
        fn it_drops_held_items() -> Result<()> {
            log_test();

            let infra = get_infra()?;
            let world = Build::new(&infra)?.into_entity();
            let rake = Build::new(&infra)?.named("Cool Rake")?.into_entity();
            let person = Build::new(&infra)?.holding(&rake)?.into_entity();
            let area = Build::new(&infra)?.into_entity();

            assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 1);

            let action = DropAction {
                maybe_item: Some(Item::Named("rake".to_string())),
            };
            let reply = action.perform((world, Rc::clone(&person), area, Rc::clone(&infra)))?;

            info!("reply: {:?}", reply);

            assert_eq!(person.borrow().scope::<Containing>()?.holding.len(), 0);

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_hold_noun_correctly() {
        let (remaining, actual) = parse("hold rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Hold(Item::Named("rake".to_owned())))
    }

    #[test]
    fn it_parses_solo_drop_correctly() {
        let (remaining, actual) = parse("drop").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Drop(None))
    }

    #[test]
    fn it_parses_drop_noun_correctly() {
        let (remaining, actual) = parse("drop rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Drop(Some(Item::Named("rake".to_owned()))))
    }

    #[test]
    fn it_errors_on_unknown_text() {
        let output = parse("hello");
        assert!(output.is_err()); // TODO Weak assertion.
    }
}
