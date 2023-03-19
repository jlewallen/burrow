use crate::plugins::library::plugin::*;

#[derive(Default)]
pub struct BuildingPlugin {}

impl Plugin for BuildingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "building"
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) {}
}

impl ParsesActions for BuildingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::DuplicateActionParser {}, i))
            .or_else(|_| try_parsing(parser::BidirectionalDigActionParser {}, i))
            .or_else(|_| try_parsing(parser::ObliterateActionParser {}, i))
            .or_else(|_| try_parsing(parser::MakeItemParser {}, i))
    }
}

pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EditorReply {}

    impl Reply for EditorReply {}

    impl ToJson for EditorReply {
        fn to_json(&self) -> Result<Value, serde_json::Error> {
            serde_json::to_value(self)
        }
    }
}

pub mod actions {
    use crate::plugins::{library::actions::*, looking::actions::LookAction};

    #[derive(Debug)]
    pub struct EditAction {
        pub item: Item,
    }

    impl Action for EditAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("editing {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(editing) => {
                    info!("editing {:?}", editing);
                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct DuplicateAction {
        pub item: Item,
    }

    impl Action for DuplicateAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("duplicating {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(duplicating) => {
                    info!("duplicating {:?}", duplicating);
                    _ = tools::duplicate(&duplicating)?;
                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct ObliterateAction {
        pub item: Item,
    }

    impl Action for ObliterateAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("obliterate {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(obliterating) => {
                    info!("obliterate {:?}", obliterating);
                    tools::obliterate(&obliterating)?;
                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct BidirectionalDigAction {
        pub outgoing: String,
        pub returning: String,
        pub new_area: String,
    }

    impl Action for BidirectionalDigAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!(
                "bidirectional-dig {:?} <-> {:?} '{:?}'",
                self.outgoing, self.returning, self.new_area
            );

            let (_, living, area) = surroundings.unpack();

            let new_area =
                session.add_entity(&EntityPtr::new_named(&self.new_area, &self.new_area)?)?;
            let returning =
                session.add_entity(&EntityPtr::new_named(&self.returning, &self.returning)?)?;
            let outgoing =
                session.add_entity(&EntityPtr::new_named(&self.outgoing, &self.outgoing)?)?;

            tools::leads_to(&returning, &area)?;
            tools::set_container(&new_area, &vec![returning])?;

            tools::leads_to(&outgoing, &new_area)?;
            tools::set_container(&area, &vec![outgoing])?;

            // TODO Chain to GoAction?
            match tools::navigate_between(&area, &new_area, &living)? {
                DomainOutcome::Ok => session.chain(&living, Box::new(LookAction {})),
                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct MakeItemAction {
        pub name: String,
    }

    impl Action for MakeItemAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("make-item {:?}", self.name);

            let (_, user, _area) = surroundings.unpack();

            let new_item = EntityPtr::new_named(&self.name, &self.name)?;

            session.add_entities(&[&new_item])?;

            tools::set_container(&user, &vec![new_item.try_into()?])?;

            Ok(Box::new(SimpleReply::Done))
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    use super::actions::{
        BidirectionalDigAction, DuplicateAction, EditAction, MakeItemAction, ObliterateAction,
    };

    pub struct MakeItemParser {}

    impl ParsesActions for MakeItemParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                tuple((preceded(
                    pair(separated_pair(tag("make"), spaces, tag("item")), spaces),
                    string_literal,
                ),)),
                |name| MakeItemAction {
                    name: name.0.into(),
                },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct EditActionParser {}

    impl ParsesActions for EditActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("edit"), spaces), noun_or_specific),
                |item| EditAction { item },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct DuplicateActionParser {}

    impl ParsesActions for DuplicateActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("duplicate"), spaces), noun_or_specific),
                |item| DuplicateAction { item },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct ObliterateActionParser {}

    impl ParsesActions for ObliterateActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("obliterate"), spaces), noun_or_specific),
                |item| ObliterateAction { item },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct BidirectionalDigActionParser {}

    impl ParsesActions for BidirectionalDigActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                tuple((
                    preceded(pair(tag("dig"), spaces), string_literal),
                    preceded(pair(spaces, pair(tag("to"), spaces)), string_literal),
                    preceded(pair(spaces, pair(tag("for"), spaces)), string_literal),
                )),
                |(outgoing, returning, new_area)| BidirectionalDigAction {
                    outgoing: outgoing.into(),
                    returning: returning.into(),
                    new_area: new_area.into(),
                },
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
        domain::{BuildSurroundings, QuickThing},
        plugins::{carrying::model::Containing, looking::model::new_area_observation, tools},
    };

    #[test]
    fn it_fails_to_edit_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit rake")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        Ok(())
    }

    #[test]
    fn it_fails_to_duplicate_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DuplicateActionParser {}, "duplicate rake")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        Ok(())
    }

    #[test]
    fn it_fails_to_obliterate_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(ObliterateActionParser {}, "obliterate rake")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        Ok(())
    }

    #[test]
    fn it_edits_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit broom")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        Ok(())
    }

    #[test]
    fn it_duplicates_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DuplicateActionParser {}, "duplicate broom")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, _area) = surroundings.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);
        assert_eq!(person.scope::<Containing>()?.holding.len(), 1);
        assert_eq!(
            tools::quantity(
                &person.scope::<Containing>()?.holding[0]
                    .clone()
                    .try_into()?
            )?,
            2.0
        );

        Ok(())
    }

    #[test]
    fn it_obliterates_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(ObliterateActionParser {}, "obliterate broom")?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, area) = surroundings.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);
        // It's not enough just to check this, but why not given how easy.
        // Should actually verify it's deleted.
        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.flush()?;

        Ok(())
    }

    #[test]
    fn it_fails_to_edit_items_by_missing_gid() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit #1201")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        Ok(())
    }

    #[test]
    fn it_edits_items_by_gid() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit #1")?;
        let reply = action.perform(session, &surroundings)?;

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        Ok(())
    }

    #[test]
    fn it_digs_bidirectionally() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(
            BidirectionalDigActionParser {},
            r#"dig "North Exit" to "South Exit" for "New Area""#,
        )?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, living, _area) = surroundings.unpack();

        // Not the best way of finding the constructed area.
        let destination = session
            .entry(&LookupBy::Gid(&EntityGid::new(4)))?
            .ok_or(DomainError::EntityNotFound)?;

        assert_eq!(
            reply.to_json()?,
            new_area_observation(&living, &destination)?.to_json()?
        );

        Ok(())
    }

    #[test]
    fn it_makes_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(MakeItemParser {}, r#"make item "Blue Rake""#)?;
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, living, _area) = surroundings.unpack();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(living.scope::<Containing>()?.holding.len(), 1);

        Ok(())
    }
}
