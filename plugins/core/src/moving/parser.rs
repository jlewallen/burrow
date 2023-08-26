use crate::library::parser::*;

use super::actions::AddRouteAction;
use super::actions::GoAction;
use super::actions::RemoveRouteAction;
use super::actions::ShowRoutesAction;

pub struct GoActionParser {}

impl ParsesActions for GoActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            separated_pair(tag("go"), spaces, named_place),
            |(_, item)| GoAction { item },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct RouteActionParser {}

impl ParsesActions for RouteActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let add = map(
            separated_pair(
                preceded(pair(tag("@route"), spaces), gid_reference),
                spaces,
                text_to_end_of_line,
            ),
            |(destination, name)| {
                Box::new(AddRouteAction {
                    area: Item::Area,
                    name: name.to_string(),
                    destination,
                }) as Box<dyn Action>
            },
        );

        let remove = map(
            separated_pair(
                preceded(pair(tag("@route"), spaces), tag("rm")),
                spaces,
                text_to_end_of_line,
            ),
            |(_, name)| {
                Box::new(RemoveRouteAction {
                    area: Item::Area,
                    name: name.to_string(),
                }) as Box<dyn Action>
            },
        );

        let show = map(tag("@route"), |_| {
            Box::new(ShowRoutesAction {}) as Box<dyn Action>
        });

        let (_, action) = alt((add, remove, show))(i)?;

        Ok(Some(action))
    }
}
