use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, rc::Rc};
use yew::prelude::Reducible;

use replies::*;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfo {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoginInfoWrapper {
    pub user: LoginInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegisterInfo {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RegisterInfoWrapper {
    pub user: RegisterInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub key: String,
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoWrapper {
    pub user: UserInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Run {
    Diagnostics {
        time: DateTime<Utc>,
        desc: String,
        logs: Vec<Entry>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LogFields {
    pub message: String,
    #[serde(flatten)]
    pub extra: HashMap<String, JsonValue>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub target: String,
    pub name: String,
    pub level: String,
    pub fields: LogFields,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub runs: Vec<Run>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AllKnownItems {
    SimpleReply(SimpleReply),
    AreaObservation(AreaObservation),
    InsideObservation(InsideObservation),
    EntityObservation(EntityObservation),
    EditorReply(EditorReply),
    MarkdownReply(MarkdownReply),
    JsonReply(JsonReply),
    Carrying(Carrying),
    Moving(Moving),
    Talking(Talking),
    Diagnostics(Diagnostics),
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct HistoryEntityPtr {
    pub value: JsonValue,
}

impl Into<Option<AllKnownItems>> for HistoryEntityPtr {
    fn into(self) -> Option<AllKnownItems> {
        if let Ok(item) = serde_json::from_value::<AllKnownItems>(self.value) {
            Some(item)
        } else {
            None
        }
    }
}

impl From<JsonValue> for HistoryEntityPtr {
    fn from(value: JsonValue) -> Self {
        Self::new(value)
    }
}

impl Into<JsonValue> for HistoryEntityPtr {
    fn into(self) -> JsonValue {
        self.value
    }
}

impl HistoryEntityPtr {
    pub fn new(value: JsonValue) -> Self {
        Self { value }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct SessionHistory {
    pub entries: Vec<HistoryEntityPtr>,
}

impl Reducible for SessionHistory {
    type Action = JsonValue;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        Rc::new(self.append(action))
    }
}

impl SessionHistory {
    pub fn append(&self, value: JsonValue) -> Self {
        let entries = if !value.is_null() {
            self.entries
                .clone()
                .into_iter()
                .chain([HistoryEntityPtr::new(value)])
                .collect()
        } else {
            self.entries.clone()
        };
        Self { entries }
    }

    pub fn latest(&self) -> Option<HistoryEntityPtr> {
        self.entries.iter().last().cloned()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Myself {
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Interaction {
    LoggedIn,
    EditorClosed,
}

impl Default for Interaction {
    fn default() -> Self {
        Self::LoggedIn
    }
}
