use std::str::FromStr;

use kernel::{EvaluationResult, ParsesActions, Plugin};

use crate::library::plugin::*;

#[derive(Default)]
pub struct BuildingPluginFactory {}

impl PluginFactory for BuildingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(BuildingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct BuildingPlugin {}

impl Plugin for BuildingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "building"
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

impl ParsesActions for BuildingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::DescribeActionParser {}, i))
            .or_else(|_| try_parsing(parser::DuplicateActionParser {}, i))
            .or_else(|_| try_parsing(parser::BidirectionalDigActionParser {}, i))
            .or_else(|_| try_parsing(parser::ObliterateActionParser {}, i))
            .or_else(|_| try_parsing(parser::MakeItemParser {}, i))
    }
}

pub mod model {
    use crate::library::model::*;

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    struct EditorReply {}

    impl Reply for EditorReply {}
}

#[derive(Default, Clone, Debug)]
struct QuickEdit {
    name: Option<String>,
    desc: Option<String>,
}

impl TryFrom<&Entry> for QuickEdit {
    type Error = DomainError;

    fn try_from(value: &Entry) -> std::result::Result<Self, Self::Error> {
        let name = value.name()?;
        let desc = value.desc()?;
        Ok(Self { name, desc })
    }
}

const SEPARATOR: &str = "## Separator";

impl FromStr for QuickEdit {
    type Err = DomainError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts: Vec<_> = s.split(SEPARATOR).collect();
        if parts.len() != 2 {
            return Err(DomainError::Anyhow(anyhow::anyhow!("malformed quick edit")));
        }

        let (name, desc) = match parts[..] {
            [name, desc] => (name, desc),
            _ => todo!(),
        };

        let name = Some(name.trim().to_owned());
        let desc = Some(desc.trim().to_owned());

        Ok(Self { name, desc })
    }
}

impl ToString for QuickEdit {
    fn to_string(&self) -> String {
        format!(
            "{}\n\n{}\n\n{}",
            self.name.as_ref().map(|s| s.as_str()).unwrap_or(""),
            SEPARATOR,
            self.desc.as_ref().map(|s| s.as_str()).unwrap_or("")
        )
    }
}

pub mod actions {
    use std::str::FromStr;

    use crate::{building::QuickEdit, library::actions::*, looking::actions::LookAction};

    #[action]
    pub struct EditAction {
        pub item: Item,
    }

    impl Action for EditAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            debug!("editing {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(editing) => {
                    info!("editing {:?}", editing);
                    let quick_edit: QuickEdit = (&editing).try_into()?;
                    Ok(EditorReply::new(
                        editing.key().to_string(),
                        WorkingCopy::Markdown(quick_edit.to_string()),
                    )
                    .into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
    pub struct EditRawAction {
        pub item: Item,
    }

    impl Action for EditRawAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            debug!("editing {:?}!", self.item);

            match session.find_item(surroundings, &self.item)? {
                Some(editing) => {
                    info!("editing {:?}", editing);
                    let editing = editing.entity();
                    Ok(EditorReply::new(
                        editing.key().to_string(),
                        WorkingCopy::Json(editing.to_json_value()?),
                    )
                    .into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
    pub struct DescribeAction {
        pub item: Item,
    }

    impl Action for DescribeAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_, living, _) = surroundings.unpack();
            let action = PerformAction::Instance(Rc::new(EditAction {
                item: self.item.clone(),
            }));
            session.perform(Perform::Living { living, action })
        }
    }

    #[action]
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
                    Ok(SimpleReply::Done.into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
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
                    Ok(SimpleReply::Done.into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
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

            let new_area: Entity = build_entity()
                .area()
                .name(&self.new_area)
                .desc(&self.new_area)
                .try_into()?;
            let new_area = session.add_entity(&EntityPtr::new(new_area))?;

            let returning: Entity = build_entity()
                .exit()
                .name(&self.returning)
                .desc(&self.returning)
                .try_into()?;
            let returning = session.add_entity(&EntityPtr::new(returning))?;

            let outgoing: Entity = build_entity()
                .exit()
                .name(&self.outgoing)
                .desc(&self.outgoing)
                .try_into()?;
            let outgoing = session.add_entity(&EntityPtr::new(outgoing))?;

            tools::leads_to(&returning, &area)?;
            tools::set_container(&new_area, &vec![returning])?;

            tools::leads_to(&outgoing, &new_area)?;
            tools::set_container(&area, &vec![outgoing])?;

            // TODO Chain to GoAction?
            match tools::navigate_between(&area, &new_area, &living)? {
                DomainOutcome::Ok => session.perform(Perform::Living {
                    living,
                    action: PerformAction::Instance(Rc::new(LookAction {})),
                }),
                DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
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

            let new_item = EntityPtr::new_named(EntityClass::item(), &self.name, &self.name)?;

            session.add_entities(&[&new_item])?;

            tools::set_container(&user, &vec![new_item.try_into()?])?;

            Ok(SimpleReply::Done.into())
        }
    }

    #[derive(Serialize, Deserialize, ToJson)]
    pub struct SaveWorkingCopyAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
    }

    impl std::fmt::Debug for SaveWorkingCopyAction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("SaveWorkingCopyAction")
                .field("key", &self.key)
                .finish()
        }
    }

    impl Action for SaveWorkingCopyAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            info!("mutate:key {:?}", self.key);

            match session.entry(&LookupBy::Key(&self.key))? {
                Some(entry) => {
                    let entity = entry.entity();
                    info!("mutate:entity {:?}", entity);

                    match &self.copy {
                        WorkingCopy::Markdown(text) => {
                            let quick = QuickEdit::from_str(text)?;
                            let mut entity = entity.borrow_mut();
                            if let Some(name) = quick.name {
                                entity.set_name(&name)?;
                            }
                            if let Some(desc) = quick.desc {
                                entity.set_desc(&desc)?;
                            }
                        }
                        WorkingCopy::Json(value) => {
                            info!("mutate:json");
                            let replacing = Entity::from_value(value.clone())?;
                            entity.replace(replacing);
                        }
                        WorkingCopy::Script(_) => unimplemented!("TODO (See SaveLeadAction)"),
                    }

                    Ok(SimpleReply::Done.into())
                }
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }
}

pub mod parser {
    use crate::library::parser::*;

    use super::actions::{
        BidirectionalDigAction, DescribeAction, DuplicateAction, EditAction, EditRawAction,
        MakeItemAction, ObliterateAction,
    };

    pub struct EditActionParser {}

    impl ParsesActions for EditActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = alt((
                map(
                    preceded(pair(tag("edit raw"), spaces), noun_or_specific),
                    |item| -> Box<dyn Action> { Box::new(EditRawAction { item }) },
                ),
                map(
                    preceded(pair(tag("edit"), spaces), noun_or_specific),
                    |item| -> Box<dyn Action> { Box::new(EditAction { item }) },
                ),
            ))(i)?;

            Ok(Some(action))
        }
    }

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

            Ok(Some(Box::new(action)))
        }
    }

    pub struct DuplicateActionParser {}

    impl ParsesActions for DuplicateActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("duplicate"), spaces), noun_or_specific),
                |item| DuplicateAction { item },
            )(i)?;

            Ok(Some(Box::new(action)))
        }
    }

    pub struct ObliterateActionParser {}

    impl ParsesActions for ObliterateActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("obliterate"), spaces), noun_or_specific),
                |item| ObliterateAction { item },
            )(i)?;

            Ok(Some(Box::new(action)))
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

            Ok(Some(Box::new(action)))
        }
    }

    pub struct DescribeActionParser {}

    impl ParsesActions for DescribeActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("describe"), spaces), noun_or_specific),
                |item| DescribeAction { item },
            )(i)?;

            Ok(Some(Box::new(action)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use super::*;
    use crate::library::tests::*;
    use crate::{
        building::actions::SaveWorkingCopyAction,
        {carrying::model::Containing, looking::model::new_area_observation, tools},
        {BuildSurroundings, QuickThing},
    };

    #[test]
    fn it_fails_to_edit_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit rake")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        Ok(())
    }

    #[test]
    fn it_fails_to_edit_raw_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit raw rake")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        Ok(())
    }

    #[test]
    fn it_edits_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit broom")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        Ok(())
    }

    #[test]
    fn it_edits_raw_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit raw broom")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        Ok(())
    }

    #[test]
    fn it_fails_to_edit_items_by_missing_gid() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit #1201")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        Ok(())
    }

    #[test]
    fn it_edits_items_by_gid() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(EditActionParser {}, "edit #1")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        Ok(())
    }

    #[test]
    fn it_fails_to_duplicate_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DuplicateActionParser {}, "duplicate rake")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        Ok(())
    }

    #[test]
    fn it_duplicates_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DuplicateActionParser {}, "duplicate broom")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, _area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);
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
    fn it_fails_to_obliterate_unknown_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(ObliterateActionParser {}, "obliterate rake")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::NotFound);

        Ok(())
    }

    #[test]
    fn it_obliterates_items_named() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .hands(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(ObliterateActionParser {}, "obliterate broom")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_world, person, area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);
        // It's not enough just to check this, but why not given how easy.
        // Should actually verify it's deleted.
        assert_eq!(person.scope::<Containing>()?.holding.len(), 0);
        assert_eq!(area.scope::<Containing>()?.holding.len(), 0);

        build.flush()?;

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
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, living, _area) = surroundings.unpack();

        // Not the best way of finding the constructed area.
        let destination = session
            .entry(&LookupBy::Gid(&EntityGid::new(3)))?
            .ok_or(DomainError::EntityNotFound)?;

        let reply: AreaObservation = reply.json_as()?;
        assert_eq!(reply, new_area_observation(&living, &destination)?);

        Ok(())
    }

    #[test]
    fn it_describes_area() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        let action = try_parsing(DescribeActionParser {}, "describe area")?;
        let action = action.unwrap();
        let reply = action.perform(session, &surroundings)?;

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        Ok(())
    }

    #[test]
    fn it_makes_items() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(MakeItemParser {}, r#"make item "Blue Rake""#)?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, living, _area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        assert_eq!(living.scope::<Containing>()?.holding.len(), 1);

        Ok(())
    }

    #[test]
    fn it_saves_changes_to_description() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;
        let (_world, _living, _area) = surroundings.unpack();

        let description = "Would be really weird if this was the original description".to_owned();
        let mut quick_edit = QuickEdit::default();
        quick_edit.name = Some("NAME!".to_owned());
        quick_edit.desc = Some(description.to_owned());

        let action = Box::new(SaveWorkingCopyAction {
            key: EntityKey::new("world"),
            copy: WorkingCopy::Markdown(quick_edit.to_string()),
        });
        let reply = action.perform(session.clone(), &surroundings)?;
        let (world, _living, _area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        assert_eq!(world.desc()?.unwrap(), description.as_str());

        Ok(())
    }

    #[test]
    fn it_saves_changes_to_whole_entities() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;
        let (_, living, area) = surroundings.unpack();

        let original = living.entity();
        let original = original.to_json_value()?;

        let action = Box::new(SaveWorkingCopyAction {
            key: area.key().clone(),
            copy: WorkingCopy::Json(original),
        });
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _living, _area) = surroundings.unpack();

        let reply: SimpleReply = reply.json_as()?;
        assert_eq!(reply, SimpleReply::Done);

        // TODO Would be really nice to have some assurances here, even though
        // I'm wondering how often this will actually get used.

        Ok(())
    }
}
