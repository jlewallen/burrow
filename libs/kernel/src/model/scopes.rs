use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::*;

use crate::here;

use super::DomainError;
use replies::{Json, JsonValue};

/// TODO Consider giving this Trait and the combination of another the ability to
/// extract itself, potentially cleaning up Entity.
pub trait Scope: DeserializeOwned + Default + std::fmt::Debug {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<JsonValue>;
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ScopeValue {
    Original(Json),
    Intermediate { value: Json, previous: Option<Json> },
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
    pub(crate) map: &'e mut HashMap<String, ScopeValue>,
}

#[allow(dead_code)]
pub struct Scopes<'e> {
    pub(crate) map: &'e HashMap<String, ScopeValue>,
}

#[allow(dead_code)]
pub struct ModifiedScope {
    scope: String,
    value: Json,
    previous: Option<Json>,
}

use tap::prelude::*;

impl<'e> Scopes<'e> {
    pub fn has_scope<T: Scope>(&self) -> bool {
        let scope_key = <T as Scope>::scope_key();

        self.map
            .contains_key(<T as Scope>::scope_key())
            .tap(|has| trace!(scope_key = scope_key, has = has, "has-scope"))
    }

    pub fn load_scope<T: Scope>(&self) -> Result<Box<T>, DomainError> {
        let scope_key = <T as Scope>::scope_key();

        if !self.map.contains_key(scope_key) {
            trace!(scope_key = scope_key, "load-scope(default)");
            return Ok(Box::default());
        }

        trace!(scope_key = scope_key, "load-scope");

        let data = &self.map[scope_key];
        let owned_value = data.clone();
        let scope: Box<T> = match owned_value {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                previous: _,
            } => serde_json::from_value(v.into()).context(here!())?,
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

        trace!(scope = scope_key, "scope-replace");

        let value: Json = scope.serialize()?.into();

        // TODO Would love to just take the value.
        let previous = self.map.get(scope_key).map(|value| match value {
            ScopeValue::Original(original) => original.clone(),
            ScopeValue::Intermediate { value, previous: _ } => value.clone(),
        });

        let value = ScopeValue::Intermediate { value, previous };

        self.map.insert(scope_key.to_owned(), value);

        Ok(())
    }

    pub fn remove_scope_by_key(&mut self, scope_key: &str) -> Result<(), DomainError> {
        self.map.remove(scope_key);

        Ok(())
    }

    pub fn rename_scope(&mut self, old_key: &str, new_key: &str) -> Result<(), DomainError> {
        if let Some(value) = self.map.remove(old_key) {
            self.map.insert(new_key.to_owned(), value);
        }

        Ok(())
    }

    pub fn add_scope_by_key(&mut self, scope_key: &str) -> Result<(), DomainError> {
        if !self.map.contains_key(scope_key) {
            self.map.insert(
                scope_key.to_owned(),
                ScopeValue::Original(JsonValue::Object(Default::default()).into()),
            );
        }
        Ok(())
    }
}

pub trait HasScopes {
    fn scopes(&self) -> Scopes;

    fn scopes_mut(&mut self) -> ScopesMut;
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ScopeMap(HashMap<String, ScopeValue>);

impl ScopeMap {}

impl From<HashMap<String, ScopeValue>> for ScopeMap {
    fn from(value: HashMap<String, ScopeValue>) -> Self {
        Self(value)
    }
}

impl Into<HashMap<String, ScopeValue>> for ScopeMap {
    fn into(self) -> HashMap<String, ScopeValue> {
        self.0
    }
}

impl HasScopes for ScopeMap {
    fn scopes(&self) -> Scopes {
        Scopes { map: &self.0 }
    }

    fn scopes_mut(&mut self) -> ScopesMut {
        ScopesMut { map: &mut self.0 }
    }
}
