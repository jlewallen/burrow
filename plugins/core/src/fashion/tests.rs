use super::model::*;
use super::parser::*;
use super::*;
use crate::carrying::model::Containing;
use crate::library::tests::*;

#[test]
fn it_wears_unworn_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .hands(vec![QuickThing::Wearable("Cool Jacket")])
        .build()?;

    let (_, person, _area) = surroundings.unpack();
    assert_eq!(person.scope::<Wearing>()?.wearing.len(), 0);
    assert_eq!(person.scope::<Containing>()?.holding.len(), 1);

    let action = try_parsing(WearActionParser {}, "wear jacket")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;
    assert_eq!(effect, Effect::Ok);

    assert_eq!(person.scope::<Wearing>()?.wearing.len(), 1);
    assert_eq!(person.scope::<Containing>()?.holding.len(), 0);

    build.close()?;

    Ok(())
}

#[test]
fn it_removes_worn_items() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (session, surroundings) = build
        .wearing(vec![QuickThing::Wearable("Cool Jacket")])
        .build()?;

    let action = try_parsing(RemoveActionParser {}, "remove jacket")?;
    let action = action.unwrap();
    let effect = action.perform(session.clone(), &surroundings)?;
    assert_eq!(effect, Effect::Ok);

    let (_, person, _area) = surroundings.unpack();
    assert_eq!(person.scope::<Wearing>()?.wearing.len(), 0);
    assert_eq!(person.scope::<Containing>()?.holding.len(), 1);

    build.close()?;

    Ok(())
}
