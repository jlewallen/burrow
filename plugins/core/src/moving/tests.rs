use super::parser::*;
use super::*;
use crate::library::tests::*;
use crate::looking::model::new_area_observation;
use crate::moving::actions::{AddRouteAction, RemoveRouteAction, ShowRoutesAction};

#[test]
fn it_goes_ignores_bad_matches() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let east = build.make(QuickThing::Place("East Place"))?;
    let west = build.make(QuickThing::Place("West Place"))?;
    let (session, surroundings) = build
        .route("East", QuickThing::Actual(east))
        .route("Wast", QuickThing::Actual(west))
        .build()?;

    let action = try_parsing(GoActionParser {}, "go north")?;
    let action = action.unwrap();
    let reply = action.perform(session, &surroundings)?;

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    build.close()?;

    Ok(())
}

#[test]
fn it_goes_through_correct_route_when_two_nearby() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let east = build.make(QuickThing::Place("East Place"))?;
    let west = build.make(QuickThing::Place("West Place"))?;
    let (session, surroundings) = build
        .route("East", QuickThing::Actual(east.clone()))
        .route("Wast", QuickThing::Actual(west))
        .build()?;

    let action = try_parsing(GoActionParser {}, "go east")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, living, area) = surroundings.unpack();

    let reply: AreaObservation = reply.json_as()?;
    assert_eq!(reply, new_area_observation(&living, &east)?);

    assert_ne!(tools::area_of(&living)?.key(), area.key());
    assert_eq!(tools::area_of(&living)?.key(), east.key());

    build.close()?;

    Ok(())
}

#[test]
fn it_goes_through_routes_when_one_nearby() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let (session, surroundings) = build
        .route("East", QuickThing::Actual(destination.clone()))
        .build()?;

    let action = try_parsing(GoActionParser {}, "go east")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, living, area) = surroundings.unpack();

    let reply: AreaObservation = reply.json_as()?;
    assert_eq!(reply, new_area_observation(&living, &destination)?);

    assert_ne!(tools::area_of(&living)?.key(), area.key());
    assert_eq!(tools::area_of(&living)?.key(), destination.key());

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_go_unknown_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.plain().build()?;

    let action = try_parsing(GoActionParser {}, "go rake")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_go_non_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Rake")])
        .build()?;

    let action = try_parsing(GoActionParser {}, "go rake")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    build.close()?;

    Ok(())
}

#[test]
fn it_parses_show_routes() -> Result<()> {
    let action = try_parsing(RouteActionParser {}, "@route")?;
    assert_eq!(
        action.unwrap().to_tagged_json()?,
        ShowRoutesAction {}.to_tagged_json()?
    );

    Ok(())
}

#[test]
fn it_parses_add_or_set_route() -> Result<()> {
    let action = try_parsing(RouteActionParser {}, "@route #5 north")?;
    assert_eq!(
        action.unwrap().to_tagged_json()?,
        AddRouteAction {
            name: "north".to_owned(),
            destination: Item::Gid(EntityGid::new(5)),
        }
        .to_tagged_json()?
    );

    Ok(())
}

#[test]
fn it_parses_remove_route() -> Result<()> {
    let action = try_parsing(RouteActionParser {}, "@route rm north")?;
    assert_eq!(
        action.unwrap().to_tagged_json()?,
        RemoveRouteAction {
            name: "north".to_owned()
        }
        .to_tagged_json()?
    );

    Ok(())
}
