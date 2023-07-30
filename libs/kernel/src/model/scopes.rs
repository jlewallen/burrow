use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, span, Level};

use crate::{get_my_session, DomainError, EntityKey, SessionRef};

#[derive(Clone, Serialize, Deserialize)]
pub struct JsonValue(serde_json::Value);

/// TODO Consider giving this Trait and the combination of another the ability to
/// extract itself, potentially cleaning up Entity.
pub trait Scope: Needs<SessionRef> + DeserializeOwned + Default + std::fmt::Debug {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<serde_json::Value>;
}

/// TODO I would love to deprecate this but I don't know if I'll need it.
pub trait Needs<T> {
    fn supply(&mut self, resource: &T) -> Result<()>;
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ScopeValue {
    Original(JsonValue),
    Intermediate {
        value: JsonValue,
        previous: Option<JsonValue>,
    },
}

impl Serialize for ScopeValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ScopeValue::Original(value) => value.serialize(serializer),
            ScopeValue::Intermediate { value, previous: _ } => value.serialize(serializer),
        }
    }
}

#[allow(dead_code)]
pub struct ScopesMut<'e> {
    pub(crate) key: &'e EntityKey,
    pub(crate) map: &'e mut HashMap<String, ScopeValue>,
}

#[allow(dead_code)]
pub struct Scopes<'e> {
    pub(crate) key: &'e EntityKey,
    pub(crate) map: &'e HashMap<String, ScopeValue>,
}

#[allow(dead_code)]
pub struct ModifiedScope {
    entity: EntityKey,
    scope: String,
    value: JsonValue,
    previous: Option<JsonValue>,
}

impl<'e> Scopes<'e> {
    pub fn has_scope<T: Scope>(&self) -> bool {
        self.map.contains_key(<T as Scope>::scope_key())
    }

    pub fn load_scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _load_scope_span = span!(
            Level::TRACE,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        if !self.map.contains_key(scope_key) {
            return Ok(Box::default());
        }

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let data = &self.map[scope_key];
        let owned_value = data.clone();
        let mut scope: Box<T> = match owned_value {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                previous: _,
            } => serde_json::from_value(v.0)?,
        };

        match get_my_session() {
            Ok(session) => scope.supply(&session)?,
            Err(e) => debug!("load-scope: {:?}", e),
        };

        Ok(scope)
    }

    pub fn modified(&self) -> Result<Vec<ModifiedScope>> {
        let mut changes = Vec::new();

        for (key, value) in self.map.iter() {
            match value {
                ScopeValue::Original(_) => {}
                ScopeValue::Intermediate { value, previous } => {
                    // TODO Not happy about cloning the JSON here.
                    changes.push(ModifiedScope {
                        entity: self.key.clone(),
                        scope: key.clone(),
                        value: value.clone(),
                        previous: previous.clone(),
                    });
                }
            }
        }

        Ok(changes)
    }
}

impl<'e> ScopesMut<'e> {
    pub fn replace_scope<T: Scope>(&mut self, scope: &T) -> Result<(), DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _span = span!(
            Level::TRACE,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        let value = JsonValue(scope.serialize()?);

        debug!("scope-replace");

        // TODO Would love to just take the value.
        let previous = self.map.get(scope_key).map(|value| match value {
            ScopeValue::Original(original) => original.clone(),
            ScopeValue::Intermediate { value, previous: _ } => value.clone(),
        });

        let value = ScopeValue::Intermediate { value, previous };

        self.map.insert(scope_key.to_owned(), value);

        Ok(())
    }
}

pub trait HasScopes {
    fn scopes(&self) -> Scopes;

    fn scopes_mut(&mut self) -> ScopesMut;
}
