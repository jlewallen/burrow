use serde::{Deserialize, Serialize};
use serde_json::Value;

use macros::ToJson;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Json(serde_json::Value);

impl From<serde_json::Value> for Json {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl Into<serde_json::Value> for Json {
    fn into(self) -> serde_json::Value {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaggedJson(String, serde_json::Value);

impl TaggedJson {
    pub fn tag(&self) -> &str {
        &self.0
    }

    pub fn value(&self) -> &serde_json::Value {
        &self.1
    }
}

impl TryFrom<serde_json::Value> for TaggedJson {
    type Error = TaggedJsonError;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        match value {
            Value::Object(o) => {
                let mut iter = o.into_iter();
                if let Some(solo) = iter.next() {
                    if iter.next().is_some() {
                        Err(TaggedJsonError::Malformed)
                    } else {
                        Ok(Self(solo.0, solo.1))
                    }
                } else {
                    Err(TaggedJsonError::Malformed)
                }
            }
            _ => Err(TaggedJsonError::Malformed),
        }
    }
}

impl Into<serde_json::Value> for TaggedJson {
    fn into(self) -> serde_json::Value {
        serde_json::Value::Object([(self.0, self.1)].into_iter().collect())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TaggedJsonError {
    #[error("Malformed tagged JSON")]
    Malformed,
}

pub trait ToJson: std::fmt::Debug {
    fn to_tagged_json(&self) -> Result<Value, serde_json::Error>;
}

pub trait Reply: ToJson {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub enum SimpleReply {
    Done,
    NotFound,
    What,
    Impossible,
    Prevented,
}

impl Reply for SimpleReply {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObservedEntity {
    pub key: String,
    pub name: Option<String>,
    pub qualified: Option<String>,
    pub desc: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct InsideObservation {
    pub vessel: ObservedEntity,
    pub items: Vec<ObservedEntity>,
}

impl Reply for InsideObservation {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct EntityObservation {
    pub entity: ObservedEntity,
    pub wearing: Option<Vec<ObservedEntity>>,
}

impl Reply for EntityObservation {}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkingCopy {
    Markdown(String),
    Json(serde_json::Value),
    Script(String),
}

impl std::fmt::Debug for WorkingCopy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(_) => f.debug_tuple("Markdown").finish(),
            Self::Json(_) => f.debug_tuple("Json").finish(),
            Self::Script(_) => f.debug_tuple("Script").finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct JsonTemplate(serde_json::Value);

pub const JSON_TEMPLATE_VALUE_SENTINEL: &str = "!#$value";

impl JsonTemplate {
    pub fn instantiate(self, value: &serde_json::Value) -> serde_json::Value {
        match self.0 {
            Value::Null | Value::Bool(_) | Value::Number(_) => self.0,
            Value::String(s) => {
                if s == JSON_TEMPLATE_VALUE_SENTINEL {
                    value.clone()
                } else {
                    Value::String(s)
                }
            }
            Value::Array(v) => Value::Array(
                v.into_iter()
                    .map(|c| JsonTemplate(c).instantiate(value))
                    .collect(),
            ),
            Value::Object(v) => Value::Object(
                v.into_iter()
                    .map(|(k, v)| (k, JsonTemplate(v).instantiate(value)))
                    .collect(),
            ),
        }
    }
}

impl From<serde_json::Value> for JsonTemplate {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct EditorReply {
    key: String,
    editing: WorkingCopy,
    save: JsonTemplate,
}

impl EditorReply {
    pub fn new(key: String, editing: WorkingCopy, save: JsonTemplate) -> Self {
        Self { key, editing, save }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn editing(&self) -> &WorkingCopy {
        &self.editing
    }

    pub fn save(&self) -> &JsonTemplate {
        &self.save
    }
}

impl Reply for EditorReply {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct JsonReply {
    value: serde_json::Value,
}

impl From<serde_json::Value> for JsonReply {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}

impl Reply for JsonReply {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownReply {
    value: String,
}

impl Into<String> for MarkdownReply {
    fn into(self) -> String {
        self.value
    }
}

impl From<String> for MarkdownReply {
    fn from(value: String) -> Self {
        Self { value }
    }
}

impl std::str::FromStr for MarkdownReply {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            value: s.to_owned(),
        })
    }
}

impl Reply for MarkdownReply {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub enum HelloWorld {
        Message(String),
    }

    #[test]
    pub fn test_to_json_tags_enum() {
        assert_eq!(
            HelloWorld::Message("Hey!".to_owned())
                .to_tagged_json()
                .expect("ToJson failed"),
            json!({ "helloWorld": { "message": "Hey!" } })
        );
    }
}

pub trait DomainEvent: ToJson {}

#[derive(Debug, Serialize, Deserialize, ToJson)]
#[serde(rename_all = "camelCase")]
pub enum CarryingEvent {
    Held {
        living: ObservedEntity,
        item: ObservedEntity,
        area: ObservedEntity,
    },
    Dropped {
        living: ObservedEntity,
        item: ObservedEntity,
        area: ObservedEntity,
    },
}

impl DomainEvent for CarryingEvent {}

#[derive(Debug, Serialize, Deserialize, ToJson)]
#[serde(rename_all = "camelCase")]
pub enum MovingEvent {
    Left {
        living: ObservedEntity,
        area: ObservedEntity,
    },
    Arrived {
        living: ObservedEntity,
        area: ObservedEntity,
    },
}

impl DomainEvent for MovingEvent {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Spoken {
    pub who: ObservedEntity,
    pub message: String,
}

impl Spoken {
    pub fn new(who: ObservedEntity, message: &str) -> Self {
        Self {
            who,
            message: message.to_owned(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToJson)]
#[serde(rename_all = "camelCase")]
pub enum TalkingEvent {
    Conversation(Spoken),
    Whispering(Spoken),
}

impl DomainEvent for TalkingEvent {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Emoted {
    pub who: ObservedEntity,
}

impl Emoted {
    pub fn new(who: ObservedEntity) -> Self {
        Self { who }
    }
}

#[derive(Debug, Serialize, Deserialize, ToJson)]
#[serde(rename_all = "camelCase")]
pub enum EmotingEvent {
    Laugh(Emoted),
}

impl DomainEvent for EmotingEvent {}
