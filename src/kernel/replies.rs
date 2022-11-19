use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Debug;

pub type ReplyResult = Result<Box<dyn Reply>>;

pub trait ToJson: Debug {
    fn to_json(&self) -> Result<Value>;
}

pub trait Reply: ToJson {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SimpleReply {
    Done,
    NotFound,
    What,
    Impossible,
}

impl ToJson for SimpleReply {
    fn to_json(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

impl Reply for SimpleReply {}
