use crate::library::parser::*;

use super::actions::RelocateAction;

pub struct MoveActionParser {}

impl ParsesActions for MoveActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            pair(
                preceded(tuple((tag("move"), spaces)), noun_or_specific),
                preceded(spaces, noun_or_specific),
            ),
            |(item, destination)| RelocateAction { item, destination },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}
