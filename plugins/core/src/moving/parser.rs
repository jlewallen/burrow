use crate::library::parser::*;

use super::actions::GoAction;
use super::actions::RouteAction;

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

pub struct RouteActionParser {}

impl ParsesActions for RouteActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(tag("@route"), |_| Box::new(RouteAction {}))(i)?;

        Ok(Some(action))
    }
}
