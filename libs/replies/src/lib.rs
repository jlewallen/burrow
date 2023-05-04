use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;

pub trait ToJson: Debug {
    fn to_json(&self) -> Result<Value, serde_json::Error>;
}

pub trait Reply: ToJson {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SimpleReply {
    Done,
    NotFound,
    What,
    Impossible,
    Prevented,
}

impl ToJson for SimpleReply {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "simpleReply": serde_json::to_value(self)? }))
    }
}

impl Reply for SimpleReply {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObservedEntity {
    pub key: String,
    pub name: Option<String>,
    pub qualified: Option<String>,
    pub desc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaObservation {
    pub area: ObservedEntity,
    pub person: ObservedEntity,
    pub living: Vec<ObservedEntity>,
    pub items: Vec<ObservedEntity>,
    pub carrying: Vec<ObservedEntity>,
    pub routes: Vec<ObservedEntity>,
}

impl Reply for AreaObservation {}

impl ToJson for AreaObservation {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "areaObservation": serde_json::to_value(self)? }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsideObservation {
    pub vessel: ObservedEntity,
    pub items: Vec<ObservedEntity>,
}

impl Reply for InsideObservation {}

impl ToJson for InsideObservation {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "insideObservation": serde_json::to_value(self)? }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KnownReply {
    AreaObservation(AreaObservation),
    InsideObservation(InsideObservation),
    SimpleReply(SimpleReply),
    SimpleObservation(SimpleObservation),
}

pub trait Observed: ToJson {}

impl Observed for InsideObservation {}

impl Observed for AreaObservation {}

impl Observed for SimpleReply {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleObservation(serde_json::Value);

impl SimpleObservation {
    pub fn new(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl From<&SimpleObservation> for serde_json::Value {
    fn from(o: &SimpleObservation) -> Self {
        o.0.clone()
    }
}

impl ToJson for SimpleObservation {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "simpleObservation": serde_json::to_value(self)? }))
    }
}

impl Observed for SimpleObservation {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkingCopy {
    Description(String),
    Json(serde_json::Value),
    // Script(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorReply {
    pub key: String,
    pub editing: WorkingCopy,
}

impl EditorReply {
    pub fn new(key: String, editing: WorkingCopy) -> Self {
        Self { key, editing }
    }
}

impl Reply for EditorReply {}

impl ToJson for EditorReply {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        Ok(json!({ "editor": serde_json::to_value(self)? }))
    }
}
