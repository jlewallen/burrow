use kernel::prelude::*;
use plugins_core::library::parser::*;

use crate::actions::RegisterAction;

use super::actions::EditAction;
use super::actions::ShowLogAction;

pub struct EditActionParser {}

impl ParsesActions for EditActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("rune"), spaces), noun_or_specific),
            |item| -> EvaluationResult { Ok(Some(Box::new(EditAction { item }))) },
        )(i)?;

        action
    }
}

pub struct ShowLogsActionParser {}

impl ParsesActions for ShowLogsActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("@log"), spaces), noun_or_specific),
            |item| -> EvaluationResult { Ok(Some(Box::new(ShowLogAction { item }))) },
        )(i)?;

        action
    }
}

pub struct RegisterActionParser {}

impl ParsesActions for RegisterActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("@register"), spaces), noun_or_specific),
            |target| -> EvaluationResult { Ok(Some(Box::new(RegisterAction { target }))) },
        )(i)?;

        action
    }
}
