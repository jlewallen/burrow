use super::{model::*, Action};

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub trait ParsesActions {
    fn try_parse_action(&self, i: &str) -> EvaluationResult;
}

pub trait Plugin: ParsesActions {
    fn plugin_key() -> &'static str
    where
        Self: Sized;
}
