use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

use macros::ToJson;

pub trait ToJson: Debug {
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
}

impl Reply for EntityObservation {}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkingCopy {
    Markdown(String),
    Json(serde_json::Value),
    Script(String),
    Placeholder,
}

impl std::fmt::Debug for WorkingCopy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(_) => f.debug_tuple("Markdown").finish(),
            Self::Json(_) => f.debug_tuple("Json").finish(),
            Self::Script(_) => f.debug_tuple("Script").finish(),
            Self::Placeholder => write!(f, "Placeholder"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct EditorReply {
    key: String,
    editing: WorkingCopy,
    target: EditTarget,
    save: Option<ActionTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EditTarget {
    EntityJson,
    QuickEdit,
    Script,
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToJson)]
#[serde(rename_all = "camelCase")]
pub struct ActionTemplate(serde_json::Value);

impl EditorReply {
    pub fn new(
        key: String,
        editing: WorkingCopy,
        target: EditTarget,
        save: Option<ActionTemplate>,
    ) -> Self {
        Self {
            key,
            editing,
            target,
            save,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn editing(&self) -> &WorkingCopy {
        &self.editing
    }

    pub fn target(&self) -> &EditTarget {
        &self.target
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

pub trait DomainEvent: ToJson + Debug {}

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

pub mod messy {
    use crate::ToJson;
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;

    use crate::WorkingCopy;

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    pub struct SaveQuickEditAction {
        pub key: String,
        pub copy: WorkingCopy,
    }

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    pub struct SaveEntityJsonAction {
        pub key: String,
        pub copy: WorkingCopy,
    }

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    pub struct SaveScriptAction {
        pub key: String,
        pub copy: WorkingCopy,
    }

    #[derive(Debug, Serialize, Deserialize, ToJson)]
    pub struct SaveHelpAction {
        pub key: String,
        pub copy: WorkingCopy,
    }
}
