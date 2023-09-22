use super::parser::*;
use super::*;
use crate::carrying::model::{Carryable, Containing};
use crate::library::tests::*;
use crate::location::Location;

#[test]
fn it_holds_unheld_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Rake")])
        .build()?;

    let (_, person, area) = surroundings.unpack();
    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    let action = try_parsing(HoldActionParser {}, "hold rake")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_separates_multiple_ground_items_when_held() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Multiple("Cool Rake", 2.0)])
        .build()?;

    let (_, person, area) = surroundings.unpack();
    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    let action = try_parsing(HoldActionParser {}, "hold rake")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let held = &person.scope::<Containing>()?.unwrap().holding;
    let ground = &area.scope::<Containing>()?.unwrap().holding;
    assert_eq!(held.len(), 1);
    assert_eq!(ground.len(), 1);

    let held_keys: HashSet<_> = held.iter().map(|i| i.key().clone()).collect();
    let ground_keys: HashSet<_> = ground.iter().map(|i| i.key().clone()).collect();
    let common_keys: HashSet<_> = held_keys.intersection(&ground_keys).collect();
    assert_eq!(common_keys.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_combines_multiple_items_when_together_on_ground() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let same_kind = build.make(QuickThing::Object("Cool Rake"))?;
    tools::set_quantity(&same_kind, &2.0.into())?;
    let (first, second) = tools::separate(&same_kind, &1.0.into())?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Actual(first.clone())])
        .hands(vec![QuickThing::Actual(second)])
        .build()?;

    let (_, person, area) = surroundings.unpack();
    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    let action = try_parsing(HoldActionParser {}, "hold rake")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_hold_unknown_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Broom")])
        .build()?;

    let action = try_parsing(HoldActionParser {}, "hold rake")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_drops_held_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build.hands(vec![QuickThing::Object("Cool Rake")]).build()?;

    let action = try_parsing(DropActionParser {}, "drop rake")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_drop_unknown_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Object("Cool Broom")])
        .build()?;

    let action = try_parsing(DropActionParser {}, "drop rake")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_drop_unheld_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Object("Cool Broom")])
        .build()?;

    let action = try_parsing(DropActionParser {}, "drop rake")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_, person, area) = surroundings.unpack();

    let reply: SimpleReply = reply.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_put_item_in_non_containers() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let vessel = build
        .entity()?
        .named("Not A Vessel")?
        .save()?
        .carryable()?
        .into_entity()?;
    let (session, surroundings) = build
        .hands(vec![
            QuickThing::Object("key"),
            QuickThing::Actual(vessel.clone()),
        ])
        .build()?;

    let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 2);
    assert!(vessel.scope::<Containing>()?.is_none());

    build.close()?;

    Ok(())
}

#[test]
fn it_puts_items_in_containers() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let vessel = build
        .entity()?
        .named("Vessel")?
        .save()?
        .carryable()?
        .holding(&vec![])?
        .into_entity()?;
    let (session, surroundings) = build
        .hands(vec![
            QuickThing::Object("key"),
            QuickThing::Actual(vessel.clone()),
        ])
        .build()?;

    let action = try_parsing(PutInsideActionParser {}, "put key inside vessel")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(vessel.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_takes_items_out_of_containers() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let key = build
        .entity()?
        .named("Key")?
        .save()?
        .carryable()?
        .into_entity()?;
    let vessel = build
        .entity()?
        .named("Vessel")?
        .save()?
        .carryable()?
        .holding(&vec![key.clone()])?
        .into_entity()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Actual(vessel.clone())])
        .build()?;

    let action = try_parsing(TakeOutActionParser {}, "take key out of vessel")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, _area) = surroundings.unpack();

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 2);
    assert_eq!(vessel.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(
        *key.scope::<Location>()?
            .unwrap()
            .container
            .as_ref()
            .unwrap()
            .key(),
        person.key()
    );

    build.close()?;

    Ok(())
}

#[test]
fn it_gives_items_to_others() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let carla = build.with(build_entity().living().name("Carla").try_into()?)?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Object("key")])
        .occupying(vec![QuickThing::Actual(carla.clone())])
        .build()?;

    let action = try_parsing(GiveToActionParser {}, "give key to Carla")?;
    let action = action.unwrap();
    let reply = action.perform(session.clone(), &surroundings)?;
    let (_world, person, _area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(carla.scope::<Containing>()?.unwrap().holding.len(), 1);

    insta::assert_json_snapshot!(reply.to_debug_json()?);

    build.close()?;

    Ok(())
}

#[test]
fn it_drops_quantified_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Multiple("Coin", 10.0)])
        .build()?;

    let action = try_parsing(DropActionParser {}, "drop 2 coin")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_drop_when_available_quantity_insufficient() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Multiple("Coin", 4.0)])
        .build()?;

    let action = try_parsing(DropActionParser {}, "drop 5 coin")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    let reply: SimpleReply = effect.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_drops_specified_quantity() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Multiple("Coin", 4.0)])
        .build()?;

    let (_, person, _) = surroundings.unpack();

    assert_eq!(
        person.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        4.0
    );

    let action = try_parsing(DropActionParser {}, "drop 2 coin")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    assert_eq!(
        person.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        2.0
    );
    assert_eq!(
        area.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        2.0
    );

    build.close()?;

    Ok(())
}

#[test]
fn it_fails_to_hold_when_available_quantity_insufficient() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Multiple("Coin", 4.0)])
        .build()?;

    let action = try_parsing(HoldActionParser {}, "hold 5 coin")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    let reply: SimpleReply = effect.json_as()?;
    assert_eq!(reply, SimpleReply::NotFound);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 0);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    build.close()?;

    Ok(())
}

#[test]
fn it_holds_specified_quantity() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .ground(vec![QuickThing::Multiple("Coin", 4.0)])
        .build()?;

    let (_, _, area) = surroundings.unpack();

    assert_eq!(
        area.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        4.0
    );

    let action = try_parsing(HoldActionParser {}, "hold 2 coin")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let (_, person, area) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(area.scope::<Containing>()?.unwrap().holding.len(), 1);

    assert_eq!(
        person.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        2.0
    );
    assert_eq!(
        area.scope::<Containing>()?.unwrap().holding[0]
            .to_entity()?
            .scope::<Carryable>()?
            .unwrap()
            .quantity(),
        2.0
    );

    build.close()?;

    Ok(())
}

#[test]
fn it_trades_one_for_one() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let rake = build.with(build_entity().name("Rake"))?;
    let shovel = build.with(build_entity().name("Shovel"))?;
    let carla = build
        .entity()?
        .named("Carla")?
        .save()?
        .holding(&vec![rake.clone()])?
        .into_entity()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Actual(shovel.clone())])
        .occupying(vec![QuickThing::Actual(carla.clone())])
        .build()?;

    let action = try_parsing(TradeActionParser {}, "trade Carla shovel for rake")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;

    assert_eq!(effect, Effect::Ok);

    let (_, person, _) = surroundings.unpack();

    assert_eq!(person.scope::<Containing>()?.unwrap().holding.len(), 1);
    assert_eq!(carla.scope::<Containing>()?.unwrap().holding.len(), 1);

    assert_eq!(
        *person.scope::<Containing>()?.unwrap().holding[0].key(),
        rake.key()
    );
    assert_eq!(
        *carla.scope::<Containing>()?.unwrap().holding[0].key(),
        shovel.key()
    );

    build.close()?;

    Ok(())
}
