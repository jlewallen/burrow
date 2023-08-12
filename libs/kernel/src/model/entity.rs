use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use super::base::{Acls, DomainError, EntityClass, EntityKey, Identity, JsonValue};
use super::{EntityRef, LoadAndStoreScope, ScopeMap, ScopeValue};

/// Central Entity model. Right now, the only thing that is ever modified at
/// this level is `version` and even that could easily be swept into a scope.
/// It's even possible that 'version' is removed, as we need to track the value
/// outside of the Entity itself.  The only other thing that could change is
/// possibly `acls, only that's probably infrequent.  As a rule going forward,
/// these should be considered immutable.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    key: EntityKey,
    acls: Acls,
    identity: Identity,
    #[serde(rename = "klass")] // TODO Rename, legacy from Python.
    pub(super) class: EntityClass,
    pub(super) creator: Option<EntityRef>,
    pub(super) parent: Option<EntityRef>,
    pub(super) scopes: HashMap<String, ScopeValue>,
}

impl Entity {
    pub fn from_value(value: JsonValue) -> Result<Entity, DomainError> {
        Ok(serde_json::from_value(value)?)
    }

    pub(super) fn new_heavily_customized(
        key: EntityKey,
        class: EntityClass,
        identity: Identity,
        creator: Option<EntityRef>,
        parent: Option<EntityRef>,
        scopes: ScopeMap,
    ) -> Self {
        Self {
            key,
            acls: Default::default(),
            class,
            identity,
            creator,
            parent,
            scopes: scopes.into(),
        }
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn class(&self) -> &str {
        &self.class.py_type
    }

    pub fn to_json_value(&self) -> Result<JsonValue, DomainError> {
        Ok(serde_json::to_value(self)?)
    }

    pub fn entity_ref(&self) -> EntityRef {
        EntityRef::new_from_entity(self, None)
    }
}

impl LoadAndStoreScope for Entity {
    fn load_scope(&self, scope_key: &str) -> Option<&JsonValue> {
        self.scopes.get(scope_key).map(|v| v.json_value())
    }

    fn store_scope(&mut self, scope_key: &str, value: JsonValue) {
        let previous = self.scopes.remove(scope_key);
        let value = ScopeValue::Intermediate {
            value: value.into(),
            previous: previous.map(|p| p.into()),
        };
        self.scopes.insert(scope_key.to_owned(), value);
    }

    fn remove_scope(&mut self, scope_key: &str) -> Option<ScopeValue> {
        self.scopes.remove(scope_key)
    }
}

impl TryFrom<JsonValue> for Entity {
    type Error = DomainError;

    fn try_from(value: JsonValue) -> std::result::Result<Self, Self::Error> {
        Self::from_value(value)
    }
}

impl FromStr for Entity {
    type Err = DomainError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_value(serde_json::from_str(s)?)
    }
}
