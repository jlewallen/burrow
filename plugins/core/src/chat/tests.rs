use super::parser::*;
use crate::{chat::actions::SpeakAction, library::tests::*};

#[test]
fn it_raises_conversation_events() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(SpeakActionParser {}, "say hello, everyone!")?;

    assert!(matches!(effect, Effect::Ok));

    Ok(())
}

#[test]
fn it_raises_conversation_events_for_actor_area() -> Result<()> {
    let mut build = BuildSurroundings::new()?;
    let (_session, surroundings) = build.plain().encyclopedia()?.build()?;
    let (_world, _, area) = surroundings.unpack();

    let (_, effect) = perform_directly(SpeakAction {
        area: Some(Item::Key(area.key())),
        actor: Some(Item::Key(area.key())),
        here: Some("Hello!".to_owned()),
    })?;

    assert!(matches!(effect, Effect::Ok));

    Ok(())
}
