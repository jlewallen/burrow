use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use super::{base::*, Needs};
use super::{EntityRef, ScopeMap};
use super::{HasScopes, ScopeValue, Scopes, ScopesMut, SessionRef};

/// Central Entity model. Right now, the only thing that is ever modified at
/// this level is `version` and even that could easily be swept into a scope.
/// It's even possible that 'version' is removed, as we need to track the value
/// outside of the Entity itself.  The only other thing that could change is
/// possibly `acls, only that's probably infrequent.  As a rule going forward,
/// these should be considered immutable.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    key: EntityKey,
    pub(super) parent: Option<EntityRef>,
    pub(super) creator: Option<EntityRef>,
    identity: Identity,
    #[serde(rename = "klass")] // TODO Rename, legacy from Python.
    pub(super) class: EntityClass,
    acls: Acls,
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
            parent,
            creator,
            identity,
            class,
            acls: Default::default(),
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

impl HasScopes for Entity {
    fn scopes(&self) -> Scopes {
        Scopes { map: &self.scopes }
    }

    fn scopes_mut(&mut self) -> ScopesMut {
        ScopesMut {
            map: &mut self.scopes,
        }
    }
}

impl Needs<SessionRef> for Entity {
    fn supply(&mut self, session: &SessionRef) -> Result<()> {
        self.parent = session.ensure_optional_entity(&self.parent)?;
        self.creator = session.ensure_optional_entity(&self.creator)?;
        Ok(())
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
