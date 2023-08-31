use serde::{Deserialize, Serialize};

use macros::*;

pub use burrow_bon::prelude::{
    identifier_to_key, DeserializeTagged, HasTag, Json, JsonTemplate, JsonValue, TaggedJson,
    TaggedJsonError, ToTaggedJson, JSON_TEMPLATE_VALUE_SENTINEL,
};

pub trait Reply {}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub enum SimpleReply {
    Done,
    NotFound,
    What,
    Impossible,
    Prevented(Option<String>),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ObservedEntity {
    pub key: String,
    pub gid: u64,
    pub name: String,
    pub qualified: String,
    pub desc: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum ObservedRoute {
    Simple { name: String, to: ObservedEntity },
}

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AreaObservation {
    pub area: ObservedEntity,
    pub person: ObservedEntity,
    pub living: Vec<ObservedEntity>,
    pub items: Vec<ObservedEntity>,
    pub carrying: Vec<ObservedEntity>,
    pub routes: Vec<ObservedRoute>,
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

impl JsonReply {
    pub fn new(value: TaggedJson) -> Self {
        Self { value }
    }
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
pub enum Carrying {
    Held {
        actor: ObservedEntity,
        item: ObservedEntity,
        area: ObservedEntity,
    },
    Dropped {
        actor: ObservedEntity,
        item: ObservedEntity,
        area: ObservedEntity,
    },
}

impl DomainEvent for Carrying {}

#[derive(Serialize, Deserialize, ToTaggedJson, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Moving {
    Left {
        actor: ObservedEntity,
        area: ObservedEntity,
    },
    Arrived {
        actor: ObservedEntity,
        area: ObservedEntity,
    },
}

impl DomainEvent for Moving {}

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
pub enum Talking {
    Conversation(Spoken),
    Whispering(Spoken),
}

impl DomainEvent for Talking {}

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
pub enum Emoting {
    Laugh(Emoted),
}

impl DomainEvent for Emoting {}
