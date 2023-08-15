use serde::{ser::SerializeMap, Deserialize, Serialize};

use macros::*;

pub use serde_json::Value as JsonValue;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Json(JsonValue);

impl Json {
    pub fn value(&self) -> &JsonValue {
        &self.0
    }
}

impl From<JsonValue> for Json {
    fn from(value: JsonValue) -> Self {
        Self(value)
    }
}

impl From<Json> for JsonValue {
    fn from(value: Json) -> Self {
        value.0
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TaggedJson(String, Json);

impl TaggedJson {
    pub fn new(tag: String, value: Json) -> Self {
        Self(tag, value)
    }

    pub fn new_from(value: JsonValue) -> Result<Self, TaggedJsonError> {
        match value {
            JsonValue::Object(o) => {
                let mut iter = o.into_iter();
                if let Some(solo) = iter.next() {
                    if iter.next().is_some() {
                        Err(TaggedJsonError::Malformed)
                    } else {
                        Ok(Self(solo.0, solo.1.into()))
                    }
                } else {
                    Err(TaggedJsonError::Malformed)
                }
            }
            _ => Err(TaggedJsonError::Malformed),
        }
    }

    pub fn tag(&self) -> &str {
        &self.0
    }

    pub fn value(&self) -> &Json {
        &self.1
    }

    pub fn into_untagged(self) -> JsonValue {
        self.1.into()
    }

    pub fn into_tagged(self) -> JsonValue {
        self.into()
    }
}

impl Serialize for TaggedJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &self.1)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for TaggedJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;

        TaggedJson::new_from(value).map_err(|_| serde::de::Error::custom("Malformed tagged JSON"))
    }
}

impl TryFrom<JsonValue> for TaggedJson {
    type Error = TaggedJsonError;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        TaggedJson::new_from(value)
    }
}

impl From<TaggedJson> for JsonValue {
    fn from(value: TaggedJson) -> Self {
        JsonValue::Object([(value.0, value.1.into())].into_iter().collect())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TaggedJsonError {
    #[error("Malformed tagged JSON")]
    Malformed,
    #[error("JSON Error")]
    Json(#[source] serde_json::Error),
}

impl From<serde_json::Error> for TaggedJsonError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub trait ToTaggedJson {
    fn to_tagged_json(&self) -> Result<TaggedJson, TaggedJsonError>;
}

pub trait Reply {}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub enum SimpleReply {
    Done,
    NotFound,
    What,
    Impossible,
    Prevented,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ObservedEntity {
    pub key: String,
    pub gid: u64,
    pub name: Option<String>,
    pub qualified: Option<String>,
    pub desc: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AreaObservation {
    pub area: ObservedEntity,
    pub person: ObservedEntity,
    pub living: Vec<ObservedEntity>,
    pub items: Vec<ObservedEntity>,
    pub carrying: Vec<ObservedEntity>,
    pub routes: Vec<ObservedEntity>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InsideObservation {
    pub vessel: ObservedEntity,
    pub items: Vec<ObservedEntity>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EntityObservation {
    pub entity: ObservedEntity,
    pub wearing: Option<Vec<ObservedEntity>>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WorkingCopy {
    Markdown(String),
    Json(JsonValue),
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

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JsonTemplate(JsonValue);

pub const JSON_TEMPLATE_VALUE_SENTINEL: &str = "!#$value";

impl JsonTemplate {
    pub fn instantiate(self, value: &JsonValue) -> JsonValue {
        match self.0 {
            JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => self.0,
            JsonValue::String(s) => {
                if s == JSON_TEMPLATE_VALUE_SENTINEL {
                    value.clone()
                } else {
                    JsonValue::String(s)
                }
            }
            JsonValue::Array(v) => JsonValue::Array(
                v.into_iter()
                    .map(|c| JsonTemplate(c).instantiate(value))
                    .collect(),
            ),
            JsonValue::Object(v) => JsonValue::Object(
                v.into_iter()
                    .map(|(k, v)| (k, JsonTemplate(v).instantiate(value)))
                    .collect(),
            ),
        }
    }
}

impl From<JsonValue> for JsonTemplate {
    fn from(value: JsonValue) -> Self {
        Self(value)
    }
}

impl From<TaggedJson> for JsonTemplate {
    fn from(value: TaggedJson) -> Self {
        Self(value.into_tagged())
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
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

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JsonReply {
    value: TaggedJson,
}

impl From<TaggedJson> for JsonReply {
    fn from(value: TaggedJson) -> Self {
        Self { value }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownReply {
    value: String,
}

impl From<MarkdownReply> for String {
    fn from(value: MarkdownReply) -> Self {
        value.value
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

pub trait DomainEvent {}

#[derive(Serialize, Deserialize, ToTaggedJson, Debug)]
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

#[derive(Serialize, Deserialize, ToTaggedJson, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, ToTaggedJson, Debug)]
#[serde(rename_all = "camelCase")]
pub enum TalkingEvent {
    Conversation(Spoken),
    Whispering(Spoken),
}

impl DomainEvent for TalkingEvent {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Emoted {
    pub who: ObservedEntity,
}

impl Emoted {
    pub fn new(who: ObservedEntity) -> Self {
        Self { who }
    }
}

#[derive(Serialize, Deserialize, ToTaggedJson, Debug)]
#[serde(rename_all = "camelCase")]
pub enum EmotingEvent {
    Laugh(Emoted),
}

impl DomainEvent for EmotingEvent {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[derive(Debug, Serialize, ToTaggedJson)]
    #[serde(rename_all = "camelCase")]
    pub enum HelloWorld {
        Message(String),
    }

    #[test]
    pub fn test_to_json_tags_enum() {
        assert_eq!(
            HelloWorld::Message("Hey!".to_owned())
                .to_tagged_json()
                .expect("ToTaggedJson failed"),
            TaggedJson::new("helloWorld".to_owned(), json!({ "message": "Hey!"}).into())
        );
    }

    #[test]
    pub fn test_serialize_tagged_json() {
        let tagged = TaggedJson::new(
            "thing".to_owned(),
            Json(JsonValue::String("value".to_owned())),
        );
        assert_eq!(
            serde_json::to_value(&tagged).unwrap(),
            json!({ "thing": "value" })
        );
        assert_eq!(
            serde_json::from_value::<TaggedJson>(json!({ "thing": "value" })).unwrap(),
            tagged
        );
    }
}
