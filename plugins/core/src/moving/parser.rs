use crate::library::parser::*;

use super::actions::GoAction;

pub struct GoActionParser {}

impl ParsesActions for GoActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            separated_pair(tag("go"), spaces, named_place),
            |(_, target)| GoAction { item: target },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}
