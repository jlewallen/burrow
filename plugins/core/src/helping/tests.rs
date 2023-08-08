use insta::assert_json_snapshot;

use super::parser::*;
use crate::library::tests::*;

#[test]
fn it_reads_default_help() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(ReadHelpParser {}, "help")?;

    assert_json_snapshot!(effect.to_debug_json()?);

    Ok(())
}

#[test]
fn it_reads_help_by_name() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(ReadHelpParser {}, "help Food")?;

    assert_json_snapshot!(effect.to_debug_json()?);

    Ok(())
}

#[test]
fn it_allows_editing_default_help() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(HelpWithParser {}, "edit help")?;

    assert_json_snapshot!(effect.to_debug_json()?);

    Ok(())
}

#[test]
fn it_allows_editing_help_by_name() -> Result<()> {
    let (_surroundings, effect) = parse_and_perform(HelpWithParser {}, "edit help Food")?;

    assert_json_snapshot!(effect.to_debug_json()?);

    Ok(())
}
