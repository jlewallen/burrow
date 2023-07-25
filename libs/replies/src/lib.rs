use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

pub trait ToJson: Debug {
    fn to_json(&self) -> Result<Value, serde_json::Error>;
}

pub trait Reply: ToJson {}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        serde_json::to_value(BasicReply::Simple(self.clone()))
    }
}

impl Reply for SimpleReply {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservedEntity {
    pub key: String,
    pub name: Option<String>,
    pub qualified: Option<String>,
    pub desc: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        serde_json::to_value(BasicReply::AreaObservation(self.clone()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsideObservation {
    pub vessel: ObservedEntity,
    pub items: Vec<ObservedEntity>,
}

impl Reply for InsideObservation {}

impl ToJson for InsideObservation {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(BasicReply::InsideObservation(self.clone()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityObservation {
    pub entity: ObservedEntity,
}

impl Reply for EntityObservation {}

impl ToJson for EntityObservation {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(BasicReply::EntityObservation(self.clone()))
    }
}

/*
pub trait Observed: ToJson {}

impl Observed for InsideObservation {}

impl Observed for AreaObservation {}

impl Observed for SimpleReply {}

impl Observed for SimpleObservation {}
*/

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        serde_json::to_value(BasicReply::SimpleObservation(self.clone()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkingCopy {
    Description(String),
    Json(serde_json::Value),
    Script(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        serde_json::to_value(BasicReply::Editor(self.clone()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonReply {
    value: serde_json::Value,
}

impl From<serde_json::Value> for JsonReply {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Reply for JsonReply {}

impl ToJson for JsonReply {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(BasicReply::Json(self.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BasicReply {
    Simple(SimpleReply),
    EntityObservation(EntityObservation),
    InsideObservation(InsideObservation),
    AreaObservation(AreaObservation),
    SimpleObservation(SimpleObservation),
    Editor(EditorReply),
    Json(JsonReply),
}

impl ToJson for BasicReply {
    fn to_json(&self) -> Result<Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

/*
impl From<SimpleReply> for BasicReply {
    fn from(value: SimpleReply) -> Self {
        Self::Simple(value)
    }
}

impl From<JsonReply> for BasicReply {
    fn from(value: JsonReply) -> Self {
        Self::Json(value)
    }
}

impl From<EditorReply> for BasicReply {
    fn from(value: EditorReply) -> Self {
        Self::Editor(value)
    }
}
*/
