use super::actions::*;
use crate::library::parser::*;

pub struct HoldActionParser {}

impl ParsesActions for HoldActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
            HoldAction { item: target }
        })(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct DropActionParser {}

impl ParsesActions for DropActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
            DropAction {
                maybe_item: Some(Item::Held(Box::new(target))),
            }
        });

        let everything = map(tag("drop"), |_| DropAction { maybe_item: None });

        let (_, action) = alt((specific, everything))(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct TakeOutActionParser {}

impl ParsesActions for TakeOutActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let item = map(separated_pair(tag("take"), spaces, noun), |(_, target)| {
            target
        });

        let (_, action) = map(
            separated_pair(separated_pair(item, spaces, tag("out of")), spaces, noun),
            |(item, target)| TakeOutAction {
                item: Item::Contained(Box::new(item.0)),
                vessel: target,
            },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct PutInsideActionParser {}

impl ParsesActions for PutInsideActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let item = map(separated_pair(tag("put"), spaces, noun), |(_, target)| {
            target
        });

        let (_, action) = map(
            separated_pair(
                separated_pair(
                    item,
                    spaces,
                    pair(tag("inside"), opt(pair(spaces, tag("of")))),
                ),
                spaces,
                noun,
            ),
            |(item, target)| PutInsideAction {
                item: item.0,
                vessel: target,
            },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}