use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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

impl HasScopes for ScopeMap {
    fn scopes(&self) -> Scopes {
        Scopes { map: &self.0 }
    }

    fn scopes_mut(&mut self) -> ScopesMut {
        ScopesMut { map: &mut self.0 }
    }
}

#[allow(dead_code)]
mod exp {

    use std::cell::RefCell;

    use super::*;

    pub trait LoadAndStoreScope {
        fn load_scope(&self, scope_key: &str) -> Result<Option<&JsonValue>, DomainError>;
        fn store_scope(&mut self, scope_key: &str, value: JsonValue) -> Result<(), DomainError>;
    }

    pub trait OpenScope<O>
    where
        O: LoadAndStoreScope,
    {
        fn scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError>;
    }

    pub trait OpenScopeRefMut<O>
    where
        O: LoadAndStoreScope,
    {
        fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeRefMut<T, O>, DomainError>;
    }

    pub trait OpenScopeMut<O> {
        fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError>;
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

    impl<O> OpenScopeRefMut<O> for RefCell<O>
    where
        O: LoadAndStoreScope,
    {
        fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeRefMut<T, O>, DomainError> {
            let owner = self.borrow();
            let value = match owner.load_scope(T::scope_key())? {
                Some(value) => serde_json::from_value(value.clone().into()).context(here!())?,
                None => T::default(),
            };

            Ok(OpenedScopeRefMut::new(self, Box::new(value)))
        }
    }

    impl<O> OpenScope<O> for O
    where
        O: LoadAndStoreScope,
    {
        fn scope<T: Scope>(&self) -> Result<Option<OpenedScope<T>>, DomainError> {
            let Some(value) = self.load_scope(T::scope_key())? else {
                return Ok(None);
            };

            let json = value.clone().into();
            let value = serde_json::from_value(json).context(here!())?;

            Ok(Some(OpenedScope::new(Box::new(value))))
        }
    }

    impl<O> OpenScopeMut<O> for O
    where
        O: LoadAndStoreScope,
    {
        fn scope_mut<T: Scope>(&self) -> Result<OpenedScopeMut<T>, DomainError> {
            let value = match self.load_scope(T::scope_key())? {
                Some(value) => serde_json::from_value(value.clone().into()).context(here!())?,
                None => T::default(),
            };

            Ok(OpenedScopeMut::new(Box::new(value)))
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

        impl LoadAndStoreScope for Whatever {
            fn load_scope(
                &self,
                scope_key: &str,
            ) -> anyhow::Result<Option<&JsonValue>, crate::model::DomainError> {
                Ok(self.scopes.get(scope_key).map(|v| v.json_value()))
            }

            fn store_scope(
                &mut self,
                scope_key: &str,
                value: JsonValue,
            ) -> anyhow::Result<(), crate::model::DomainError> {
                let previous = self.scopes.remove(scope_key);
                let value = ScopeValue::Intermediate {
                    value: value.into(),
                    previous: previous.map(|p| p.into()),
                };
                self.scopes.insert(scope_key.to_owned(), value);

                Ok(())
            }
        }
    }

    pub struct OpenedScope<T: Scope> {
        target: Box<T>,
    }

    impl<T: Scope> OpenedScope<T> {
        pub fn new(target: Box<T>) -> Self {
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

    pub struct OpenedScopeMut<T: Scope> {
        target: Box<T>,
    }

    impl<T: Scope> OpenedScopeMut<T> {
        pub fn new(target: Box<T>) -> Self {
            trace!("scope-open {:?}", target);

            Self { target }
        }

        pub fn save<O>(&mut self, entity: &mut O) -> Result<(), DomainError>
        where
            O: LoadAndStoreScope,
        {
            entity.store_scope(T::scope_key(), self.target.serialize()?)
        }
    }

    impl<T: Scope> Drop for OpenedScopeMut<T> {
        fn drop(&mut self) {
            // TODO Check for unsaved changes to this scope and possibly warn the
            // user, this would require them to intentionally discard any unsaved
            // changes. Not being able to bubble an error up makes doing anything
            // elaborate in here a bad idea.
            // trace!("scope-dropped {:?}", self.target);
        }
    }

    impl<T: Scope> std::ops::Deref for OpenedScopeMut<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.target
        }
    }

    impl<T: Scope> std::ops::DerefMut for OpenedScopeMut<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.target
        }
    }

    pub struct OpenedScopeRefMut<'a, T: Scope, O>
    where
        O: LoadAndStoreScope,
    {
        owner: &'a RefCell<O>,
        target: Box<T>,
    }

    impl<'a, T: Scope, O> OpenedScopeRefMut<'a, T, O>
    where
        O: LoadAndStoreScope,
    {
        pub fn new(owner: &'a RefCell<O>, target: Box<T>) -> Self {
            Self { owner, target }
        }

        pub fn save(&mut self) -> Result<(), DomainError>
        where
            O: LoadAndStoreScope,
        {
            let value = self.target.serialize()?;
            let mut owner = self.owner.borrow_mut();
            owner.store_scope(T::scope_key(), value)
        }
    }

    impl<'a, T: Scope, O> Drop for OpenedScopeRefMut<'a, T, O>
    where
        O: LoadAndStoreScope,
    {
        fn drop(&mut self) {
            // TODO Check for unsaved changes to this scope and possibly warn the
            // user, this would require them to intentionally discard any unsaved
            // changes. Not being able to bubble an error up makes doing anything
            // elaborate in here a bad idea.
        }
    }

    impl<'a, T: Scope, O> std::ops::Deref for OpenedScopeRefMut<'a, T, O>
    where
        O: LoadAndStoreScope,
    {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.target
        }
    }

    impl<'a, T: Scope, O> std::ops::DerefMut for OpenedScopeRefMut<'a, T, O>
    where
        O: LoadAndStoreScope,
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.target
        }
    }
}
