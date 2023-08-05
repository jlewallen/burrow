use serde::{Deserialize, Serialize};
use std::rc::Rc;
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AllKnownItems {
    SimpleReply(SimpleReply),
    AreaObservation(AreaObservation),
    InsideObservation(InsideObservation),
    EntityObservation(EntityObservation),
    SimpleObservation(SimpleObservation),
    EditorReply(EditorReply),
    JsonReply(JsonReply),
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct HistoryEntry {
    pub value: serde_json::Value,
}

impl Into<Option<AllKnownItems>> for HistoryEntry {
    fn into(self) -> Option<AllKnownItems> {
        if let Ok(item) = serde_json::from_value::<AllKnownItems>(self.value) {
            Some(item)
        } else {
            None
        }
    }
}

impl From<serde_json::Value> for HistoryEntry {
    fn from(value: serde_json::Value) -> Self {
        Self::new(value)
    }
}

impl Into<serde_json::Value> for HistoryEntry {
    fn into(self) -> serde_json::Value {
        self.value
    }
}

impl HistoryEntry {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct SessionHistory {
    pub entries: Vec<HistoryEntry>,
}

impl Reducible for SessionHistory {
    type Action = serde_json::Value;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        Rc::new(self.append(action))
    }
}

impl SessionHistory {
    pub fn append(&self, value: serde_json::Value) -> Self {
        let entries = if !value.is_null() {
            self.entries
                .clone()
                .into_iter()
                .chain([HistoryEntry::new(value)])
                .collect()
        } else {
            self.entries.clone()
        };
        Self { entries }
    }

    pub fn latest(&self) -> Option<HistoryEntry> {
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

/// Controversial, this is from `building.rs`
#[derive(Serialize, Deserialize /*, ToJson*/)]
pub struct SaveWorkingCopyAction {
    pub key: String, // EntityKey
    pub copy: WorkingCopy,
}

/// Controversial, this is from `rune/mod.rs`
#[derive(Serialize, Deserialize /*, ToJson*/)]
pub struct SaveScriptAction {
    pub key: String, // EntityKey
    pub copy: WorkingCopy,
}
