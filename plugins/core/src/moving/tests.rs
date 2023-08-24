use super::parser::*;
use super::*;
use crate::library::tests::*;
use crate::looking::model::new_area_observation;
use crate::moving::actions::{
    AddRouteAction, DeactivateRouteAction, RemoveRouteAction, ShowRoutesAction,
};
use crate::moving::model::{Occupyable, Route, SimpleRoute};

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

#[test]
fn it_adds_simple_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", gid))?.unwrap();

    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::Done);

    let occupyable = area.scope::<Occupyable>()?.unwrap();
    let routes: Vec<Route> = occupyable.routes.clone().unwrap();
    assert_eq!(
        routes,
        vec![Route::Simple(SimpleRoute::new(
            "north",
            destination.entity_ref()
        ))]
    );

    build.close()?;

    Ok(())
}

#[test]
fn it_replaces_simple_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let previous = build.make(QuickThing::Place("Old Place"))?;
    let destination = build.make(QuickThing::Place("New Place"))?;
    let old_gid = previous.borrow().gid().unwrap();
    let new_gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", old_gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", new_gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;

    let (_, _person, area) = surroundings.unpack();

    let occupyable = area.scope::<Occupyable>()?.unwrap();
    let routes: Vec<Route> = occupyable.routes.clone().unwrap();
    assert_eq!(
        routes,
        vec![Route::Simple(SimpleRoute::new(
            "north",
            destination.entity_ref()
        ))]
    );

    build.close()?;

    Ok(())
}

#[test]
fn it_removes_simple_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;
    let (_, _person, area) = surroundings.unpack();

    let occupyable = area.scope::<Occupyable>()?.unwrap();
    let routes: Vec<Route> = occupyable.routes.clone().unwrap();
    assert_eq!(
        routes,
        vec![Route::Simple(SimpleRoute::new(
            "north",
            destination.entity_ref()
        ))]
    );

    let action = try_parsing(RouteActionParser {}, &format!("@route rm north"))?.unwrap();
    action.perform(session.clone(), &surroundings)?;
    let (_, _person, area) = surroundings.unpack();

    let occupyable = area.scope::<Occupyable>()?.unwrap();
    let routes: Vec<Route> = occupyable.routes.clone().unwrap();
    assert!(routes.is_empty());

    build.close()?;

    Ok(())
}

#[test]
fn it_navigates_through_simple_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;

    let action = try_parsing(GoActionParser {}, "go north")?.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_picks_best_matching_route_by_name() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;
    let action = try_parsing(RouteActionParser {}, &format!("@route #{} south", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;
    let action = try_parsing(RouteActionParser {}, &format!("@route #{} northwest", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;

    let action = try_parsing(GoActionParser {}, "go east")?.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    build.close()?;

    Ok(())
}

#[test]
fn it_deactivates_routes() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let gid = destination.borrow().gid().unwrap();
    let (session, surroundings) = build.build()?;

    let action = try_parsing(RouteActionParser {}, &format!("@route #{} north", gid))?.unwrap();
    action.perform(session.clone(), &surroundings)?;

    let action = DeactivateRouteAction {
        name: "north".to_owned(),
        reason: "A rock slide is blocking your way.".to_owned(),
    };
    action.perform(session.clone(), &surroundings)?;

    let action = try_parsing(GoActionParser {}, "go north")?.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}
