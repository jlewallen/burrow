use super::parser::*;
use super::*;
use crate::carrying::model::Containing;
use crate::library::tests::*;

#[test]
fn it_moves_items_from_ourselves_to_here() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Wearable("Cool Jacket")])
        .build()?;

    let action = try_parsing(MoveActionParser {}, "move jacket here")?;
    let action = action.unwrap();
    let reply = action.perform(session, &surroundings)?;

    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_moves_items_from_ourselves_to_another_place_by_gid() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let east = build.make(QuickThing::Place("East Place"))?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Wearable("Cool Jacket")])
        .route("East", QuickThing::Actual(east.clone()))
        .build()?;

    let action = try_parsing(MoveActionParser {}, &format!("move jacket #{}", east.gid()))?;
    let action = action.unwrap();
    let reply = action.perform(session, &surroundings)?;

    let (_, person, _) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(east.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_moves_items_from_area_to_area() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let east = build.make(QuickThing::Place("East Place"))?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Wearable("Cool Jacket")])
        .route("East", QuickThing::Actual(east.clone()))
        .build()?;

    let action = try_parsing(MoveActionParser {}, &format!("move jacket #{}", east.gid()))?;
    let action = action.unwrap();
    let reply = action.perform(session, &surroundings)?;

    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(east.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_moves_areas_into_areas() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let east = build.make(QuickThing::Place("East Place"))?;
    let (session, surroundings) = build
        .route("East", QuickThing::Actual(east.clone()))
        .build()?;

    let action = try_parsing(MoveActionParser {}, &format!("move #{} here", east.gid()))?;
    let action = action.unwrap();
    let reply = action.perform(session, &surroundings)?;

    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}
