use kernel::here;

use super::parser::*;
use super::*;
use crate::building::actions::{SaveEntityJsonAction, SaveQuickEditAction};
use crate::building::model::QuickEdit;
use crate::fashion::model::Wearable;
use crate::library::tests::*;
use crate::{
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

    let action = try_parsing(EditActionParser {}, "edit #2")?;
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

    let action = try_parsing(DuplicateActionParser {}, "@duplicate rake")?;
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

    let action = try_parsing(DuplicateActionParser {}, "@duplicate broom")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, _area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);
    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(
        tools::quantity(&person.scope::<Containing>()?.unwrap().holding[0].to_entity()?)?,
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

    let action = try_parsing(ObliterateActionParser {}, "@obliterate rake")?;
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

    let action = try_parsing(ObliterateActionParser {}, "@obliterate broom")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);
    // It's not enough just to check this, but why not given how easy.
    // Should actually verify it's deleted.
    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);

    build.flush()?;

    Ok(())
}

#[test]
fn it_digs_bidirectionally() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.plain().build()?;

    let action = try_parsing(
        BidirectionalDigActionParser {},
        r#"@dig "North Exit" to "South Exit" for "New Area""#,
    )?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, living, _area) = surroundings.unpack();

    // Not the best way of finding the constructed area.
    let destination = session
        .entry(&LookupBy::Gid(&EntityGid::new(4)))?
        .ok_or(DomainError::EntityNotFound(here!().into()))?;

    let reply: AreaObservation = reply.json_as()?;
    assert_eq!(reply, new_area_observation(&living, &destination)?);

    Ok(())
}

#[test]
fn it_makes_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.plain().build()?;

    let action = try_parsing(MakeItemParser {}, r#"@make item "Blue Rake""#)?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, living, _area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(living.scope::<Containing>()?.unwrap().holding.len(), 1);

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

    let action = Box::new(SaveQuickEditAction {
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

    let original = living.borrow().to_json_value()?;

    let action = Box::new(SaveEntityJsonAction {
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

#[test]
fn it_adds_scopes_to_solo_held_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let jacket = build.entity()?.named("Jacket")?.carryable()?.into_entry()?;

    assert!(!jacket.scope::<Wearable>()?.is_some());

    let (session, surroundings) = build
        .plain()
        .hands(vec![QuickThing::Actual(jacket.clone())])
        .build()?;

    let action = try_parsing(ScopeActionParser {}, r#"@scope wearable"#)?.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, living, _area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(living.scope::<Containing>()?.unwrap().holding.len(), 1);

    assert!(jacket.scope::<Wearable>()?.is_some());

    Ok(())
}
