use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use tracing::*;

use super::DomainError;
use crate::here;
use replies::{Json, JsonValue};

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

impl Into<Json> for ScopeValue {
    fn into(self) -> Json {
        match self {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                previous: _,
            } => v,
        }
    }
}

impl ScopeValue {
    pub fn json_value(&self) -> &JsonValue {
        match self {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                previous: _,
            } => v.value(),
        }
    }
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
pub struct ModifiedScope {
    scope: String,
    value: Json,
    previous: Option<Json>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ScopeMap(HashMap<String, ScopeValue>);

impl From<HashMap<String, ScopeValue>> for ScopeMap {
    fn from(value: HashMap<String, ScopeValue>) -> Self {
        Self(value)
    }
}

impl From<ScopeMap> for HashMap<String, ScopeValue> {
    fn from(value: ScopeMap) -> Self {
        value.0
    }
}

pub trait StoreScope {
    fn store_scope(&mut self, scope_key: &str, value: JsonValue);
}

impl StoreScope for HashMap<String, ScopeValue> {
    fn store_scope(&mut self, scope_key: &str, value: JsonValue) {
        let previous = self.remove(scope_key);
        let value = ScopeValue::Intermediate {
            value: value.into(),
            previous: previous.map(|p| p.into()),
        };
        self.insert(scope_key.to_owned(), value);
    }
}

impl StoreScope for ScopeMap {
    fn store_scope(&mut self, scope_key: &str, value: JsonValue) {
        self.0.store_scope(scope_key, value);
    }
}

pub trait LoadAndStoreScope: StoreScope {
    fn load_scope(&self, scope_key: &str) -> Option<&JsonValue>;
    fn remove_scope(&mut self, scope_key: &str) -> Option<ScopeValue>;

    fn rename_scope(&mut self, old_key: &str, new_key: &str) {
        if let Some(value) = self.remove_scope(old_key) {
            self.store_scope(new_key, value.json_value().clone());
        }
    }
    fn add_scope_by_key(&mut self, scope_key: &str) {
        self.store_scope(scope_key, JsonValue::Object(Default::default()));
    }
    fn replace_scope<T: Scope>(&mut self, value: &T) -> Result<(), DomainError> {
        let json = value.serialize()?.into();
        self.store_scope(T::scope_key(), json);
        Ok(())
    }
}

impl LoadAndStoreScope for HashMap<String, ScopeValue> {
    fn load_scope(&self, scope_key: &str) -> Option<&JsonValue> {
        self.get(scope_key).map(|v| v.json_value())
    }

    fn remove_scope(&mut self, scope_key: &str) -> Option<ScopeValue> {
        self.remove(scope_key)
    }
}

impl LoadAndStoreScope for ScopeMap {
    fn load_scope(&self, scope_key: &str) -> Option<&JsonValue> {
        self.0.load_scope(scope_key)
    }

    fn remove_scope(&mut self, scope_key: &str) -> Option<ScopeValue> {
        self.0.remove_scope(scope_key)
    }
}

pub trait OpenScope<O> {
    fn scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError>;
}

impl<O> OpenScope<O> for O
where
    O: LoadAndStoreScope,
{
    fn scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
        let Some(value) = self.load_scope(T::scope_key()) else {
                return Ok(None);
            };

        let json = value.clone().into();
        let value = serde_json::from_value(json).context(here!())?;

        Ok(Some(OpenedScope::new(value)))
    }
}

impl<O> OpenScope<O> for RefCell<O>
where
    O: LoadAndStoreScope,
{
    fn scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
        let owner = self.borrow();
        owner.scope::<T>()
    }
}

pub trait OpenScopeMut<O> {
    fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError>;
}

impl<O> OpenScopeMut<O> for O
where
    O: LoadAndStoreScope,
{
    fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError> {
        let value = match self.load_scope(T::scope_key()) {
            Some(value) => serde_json::from_value(value.clone().into()).context(here!())?,
            None => T::default(),
        };

        Ok(OpenedScopeMut::new(value))
    }
}

pub trait OpenScopeRefMut<O>
where
    O: StoreScope,
{
    fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeRefMut<T, O>, DomainError>;
}

impl<O> OpenScopeRefMut<O> for RefCell<O>
where
    O: LoadAndStoreScope,
{
    fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeRefMut<T, O>, DomainError> {
        let owner = self.borrow();
        let value = match owner.load_scope(T::scope_key()) {
            Some(value) => serde_json::from_value(value.clone().into()).context(here!())?,
            None => T::default(),
        };

        Ok(OpenedScopeRefMut::new(self, value))
    }
}

pub struct OpenedScope<T> {
    target: T,
}

impl<T> OpenedScope<T> {
    pub fn into(self) -> T {
        self.target
    }
}

impl<T: Scope + Default> Default for OpenedScope<T> {
    fn default() -> Self {
        Self {
            target: Default::default(),
        }
    }
}

impl<T: Scope> OpenedScope<T> {
    pub fn new(target: T) -> Self {
        trace!("scope-open {:?}", target);

        Self { target }
    }
}

impl<T: Scope> AsRef<T> for OpenedScope<T> {
    fn as_ref(&self) -> &T {
        &self.target
    }
}

impl<T: Scope> std::ops::Deref for OpenedScope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

pub struct OpenedScopeMut<T> {
    target: T,
}

impl<T: Scope> OpenedScopeMut<T> {
    pub fn new(target: T) -> Self {
        trace!("scope-open {:?}", target);

        Self { target }
    }

    pub fn save<O>(&mut self, entity: &mut O) -> Result<(), DomainError>
    where
        O: StoreScope,
    {
        Ok(entity.store_scope(T::scope_key(), self.target.serialize()?))
    }
}

impl<T> Drop for OpenedScopeMut<T> {
    fn drop(&mut self) {
        // TODO Panic or log on unsaved changes?
    }
}

impl<T> std::ops::Deref for OpenedScopeMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl<T> std::ops::DerefMut for OpenedScopeMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

pub struct OpenedScopeRefMut<'a, T, O> {
    owner: &'a RefCell<O>,
    target: T,
}

impl<'a, T: Scope, O> OpenedScopeRefMut<'a, T, O>
where
    O: StoreScope,
{
    pub fn new(owner: &'a RefCell<O>, target: T) -> Self {
        trace!("scope-open {:?}", target);

        Self { owner, target }
    }

    pub fn save(&mut self) -> Result<(), DomainError> {
        let value = self.target.serialize()?;
        let mut owner = self.owner.borrow_mut();
        Ok(owner.store_scope(T::scope_key(), value))
    }
}

impl<'a, T, O> Drop for OpenedScopeRefMut<'a, T, O> {
    fn drop(&mut self) {
        // TODO Panic or log on unsaved changes?
    }
}

impl<'a, T, O> std::ops::Deref for OpenedScopeRefMut<'a, T, O> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl<'a, T, O> std::ops::DerefMut for OpenedScopeRefMut<'a, T, O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    pub fn test_plain_owner_passing_mutable_to_save() -> Result<()> {
        let mut w = Whatever::default();
        let mut scope = w.scope_mut::<ExampleScope>()?;
        scope.values.push("A".to_owned());
        scope.save(&mut w)?;

        let mut scope = w.scope_mut::<ExampleScope>()?;
        scope.values.push("B".to_owned());
        scope.save(&mut w)?;

        let read = w.scope::<ExampleScope>()?.unwrap();
        assert_eq!(read.values, vec!["A", "B"]);

        Ok(())
    }

    #[test]
    pub fn test_refcell_owner() -> Result<()> {
        let w = RefCell::new(Whatever::default());

        assert!(w.scope::<ExampleScope>()?.is_none());

        let mut scope = w.scope_mut::<ExampleScope>()?;
        scope.values.push("A".to_owned());
        scope.save()?;

        let mut scope = w.scope_mut::<ExampleScope>()?;
        scope.values.push("B".to_owned());
        scope.save()?;

        let read = w.scope::<ExampleScope>()?.unwrap();
        assert_eq!(read.values, vec!["A", "B"]);

        Ok(())
    }

    #[derive(Debug, Deserialize, Serialize, Default)]
    pub struct ExampleScope {
        values: Vec<String>,
    }

    impl Scope for ExampleScope {
        fn scope_key() -> &'static str
        where
            Self: Sized,
        {
            "example"
        }

        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }
    }

    #[derive(Default)]
    pub struct Whatever {
        scopes: HashMap<String, ScopeValue>,
    }

    impl StoreScope for Whatever {
        fn store_scope(&mut self, scope_key: &str, value: JsonValue) {
            let previous = self.scopes.remove(scope_key);
            let value = ScopeValue::Intermediate {
                value: value.into(),
                previous: previous.map(|p| p.into()),
            };
            self.scopes.insert(scope_key.to_owned(), value);
        }
    }

    impl LoadAndStoreScope for Whatever {
        fn load_scope(&self, scope_key: &str) -> Option<&JsonValue> {
            self.scopes.get(scope_key).map(|v| v.json_value())
        }

        fn remove_scope(&mut self, scope_key: &str) -> Option<ScopeValue> {
            self.scopes.remove(scope_key)
        }
    }
}
