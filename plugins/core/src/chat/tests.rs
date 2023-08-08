use super::parser::*;
use crate::library::tests::*;

#[test]
fn it_raises_conversation_events() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(SpeakActionParser {}, "say hello, everyone!")?;

    assert!(matches!(effect, Effect::Ok));

    Ok(())
}
