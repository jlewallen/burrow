use crate::library::parser::*;

use super::actions::{
    AddScopeAction, BidirectionalDigAction, BuildAreaAction, DuplicateAction, EditAction,
    EditRawAction, LimboAction, MakeItemAction, ObliterateAction,
};

pub struct EditActionParser {}

impl ParsesActions for EditActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = alt((
            map(
                preceded(pair(tag("edit raw"), spaces), noun_or_specific),
                |item| -> Box<dyn Action> { Box::new(EditRawAction { item }) },
            ),
            map(
                preceded(pair(tag("edit"), spaces), noun_or_specific),
                |item| -> Box<dyn Action> { Box::new(EditAction { item }) },
            ),
        ))(i)?;

        Ok(Some(action))
    }
}

pub struct MakeItemParser {}

impl ParsesActions for MakeItemParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(
                tuple((tag("@make"), spaces, tag("item"), spaces)),
                string_literal,
            ),
            |name| MakeItemAction { name: name.into() },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct DuplicateActionParser {}

impl ParsesActions for DuplicateActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("@duplicate"), spaces), noun_or_specific),
            |item| DuplicateAction { item },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct LimboActionParser {}

impl ParsesActions for LimboActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(tag("@limbo"), |_| LimboAction {})(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct ObliterateActionParser {}

impl ParsesActions for ObliterateActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("@obliterate"), spaces), noun_or_specific),
            |item| ObliterateAction { item },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct BidirectionalDigActionParser {}

impl ParsesActions for BidirectionalDigActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            tuple((
                preceded(pair(tag("@dig"), spaces), string_literal),
                preceded(pair(spaces, pair(tag("to"), spaces)), string_literal),
                preceded(pair(spaces, pair(tag("for"), spaces)), string_literal),
            )),
            |(outgoing, returning, new_area)| BidirectionalDigAction {
                outgoing: outgoing.into(),
                returning: returning.into(),
                new_area: new_area.into(),
            },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct ScopeActionParser {}

impl ParsesActions for ScopeActionParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(pair(tag("@scope"), spaces), camel_case_word),
            |scope_key| AddScopeAction {
                scope_key: scope_key.to_owned(),
            },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}

pub struct BuildAreaParser {}

impl ParsesActions for BuildAreaParser {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        let (_, action) = map(
            preceded(tuple((tag("@build"), spaces)), string_literal),
            |name| BuildAreaAction { name: name.into() },
        )(i)?;

        Ok(Some(Box::new(action)))
    }
}
