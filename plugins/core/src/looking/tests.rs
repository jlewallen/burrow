use super::model::*;
use super::parser::LookActionParser;
use super::*;
use crate::library::plugin::try_parsing;
use crate::library::tests::*;

#[test]
fn it_looks_in_empty_area() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.plain().build()?;

    let action = try_parsing(LookActionParser {}, "look")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_looks_in_area_with_items_on_ground() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Rake")])
        .ground(vec![QuickThing::Object("Boring Shovel")])
        .build()?;

    let action = try_parsing(LookActionParser {}, "look")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_looks_in_area_with_items_on_ground_and_a_route() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Rake")])
        .ground(vec![QuickThing::Object("Boring Shovel")])
        .route("East Exit", QuickThing::Actual(destination))
        .build()?;

    let action = try_parsing(LookActionParser {}, "look")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_looks_in_area_with_items_on_ground_and_holding_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let destination = build.make(QuickThing::Place("Place"))?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Boring Shovel")])
        .hands(vec![QuickThing::Object("Cool Rake")])
        .route("East Exit", QuickThing::Actual(destination))
        .build()?;

    let action = try_parsing(LookActionParser {}, "look")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_look_inside_non_containers() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.hands(vec![QuickThing::Object("Not A Box")]).build()?;

    let action = try_parsing(LookActionParser {}, "look inside box")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_looks_inside_containers() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let vessel = build
        .entity()?
        .named("Vessel")?
        .carryable()?
        .holding(&vec![build.make(QuickThing::Object("Key"))?])?
        .into_entity()?;
    let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

    let action = try_parsing(LookActionParser {}, "look inside vessel")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_look_at_not_found_entities() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let vessel = build
        .entity()?
        .named("Hammer")?
        .carryable()?
        .into_entity()?;
    let (session, surroundings) = build.hands(vec![QuickThing::Actual(vessel)]).build()?;

    let action = try_parsing(LookActionParser {}, "look at shovel")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_looks_at_entities() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let hammer = build
        .entity()?
        .named("Hammer")?
        .carryable()?
        .into_entity()?;
    let (session, surroundings) = build.hands(vec![QuickThing::Actual(hammer)]).build()?;

    let action = try_parsing(LookActionParser {}, "look at hammer")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_sees_worm_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let jacket = build
        .entity()?
        .named("Jacket")?
        .wearable()?
        .carryable()?
        .into_entity()?;
    let (session, surroundings) = build.wearing(vec![QuickThing::Actual(jacket)]).build()?;

    let action = try_parsing(LookActionParser {}, "look at myself")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, _person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn qualify_name_basics() {
    // Not going to test all of indefinite's behavior here, just build edge
    // cases in our integrating logic.
    assert_eq!(Unqualified::Quantity(1.0, "box").qualify(), "a box");
    assert_eq!(Unqualified::Quantity(2.0, "box").qualify(), "2 boxes");
    assert_eq!(Unqualified::Quantity(1.0, "person").qualify(), "a person");
    assert_eq!(Unqualified::Quantity(2.0, "person").qualify(), "2 people");
    assert_eq!(Unqualified::Quantity(1.0, "orange").qualify(), "an orange");
    assert_eq!(Unqualified::Quantity(2.0, "orange").qualify(), "2 oranges");
    assert_eq!(
        Unqualified::Quantity(1.0, "East Exit").qualify(),
        "an East Exit"
    );
    assert_eq!(Unqualified::Living("Jacob").qualify(), "Jacob");
}
