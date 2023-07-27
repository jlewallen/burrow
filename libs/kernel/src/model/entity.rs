use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::{collections::HashMap, fmt::Display};
use tracing::*;

use crate::{get_my_session, CoreProps, Needs, Properties, Scope, SessionRef};

use super::base::*;

#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ScopeValue {
    Original(serde_json::Value),
    Intermediate {
        value: serde_json::Value,
        original: Option<Box<ScopeValue>>,
    },
}

impl Serialize for ScopeValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ScopeValue::Original(value) => value.serialize(serializer),
            ScopeValue::Intermediate { value, original: _ } => value.serialize(serializer),
        }
    }
}

/// Central Entity model. Right now, the only thing that is ever modified at
/// this level is `version` and even that could easily be swept into a scope.
/// It's even possible that 'version' is removed, as we need to track the value
/// outside of the Entity itself.  The only other thing that could change is
/// possibly `acls, only that's probably infrequent.  As a rule going forward,
/// these should be considered immutable.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    key: EntityKey,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    identity: Identity,
    #[serde(rename = "klass")]
    class: EntityClass,
    acls: Acls,
    scopes: HashMap<String, ScopeValue>,
}

impl Display for Entity {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct(&self.class.py_type)
            .field("key", &self.key)
            .field("name", &self.name())
            .field("gid", &self.gid())
            .finish()
    }
}

impl Needs<SessionRef> for Entity {
    fn supply(&mut self, session: &SessionRef) -> Result<()> {
        self.parent = session.ensure_optional_entity(&self.parent)?;
        self.creator = session.ensure_optional_entity(&self.creator)?;
        Ok(())
    }
}

impl Entity {
    pub fn new_blank() -> Result<Self> {
        Ok(Self::new_with_key(get_my_session()?.new_key()))
    }

    pub fn new_with_key(key: EntityKey) -> Self {
        Self {
            key,
            parent: Default::default(),
            creator: Default::default(),
            identity: Default::default(),
            class: Default::default(),
            acls: Default::default(),
            scopes: Default::default(),
        }
    }

    // TODO Allow scopes to hook into this process. For example
    // elsewhere in this commit I've wondered about how to copy 'kind'
    // into the new item in the situation for separate, so I'd start
    // there. Ultimately I think it'd be nice if we could just pass a
    // map of scopes in with their intended values.
    pub fn new_with_props(properties: Properties) -> Result<Self> {
        let mut entity = Self::new_blank()?;
        entity.set_props(properties.props())?;
        Ok(entity)
    }

    pub fn new_from(template: &Self) -> Result<Self> {
        // TODO Customize clone to always remove GID_PROPERTY
        let mut props = template.props();
        props.remove_property(GID_PROPERTY)?;
        let mut entity = Self::new_with_props(props.into())?;

        entity.class = template.class.clone();
        entity.acls = template.acls.clone();
        entity.parent = template.parent.clone();
        entity.creator = template.creator.clone();

        Ok(entity)
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn class(&self) -> &str {
        &self.class.py_type
    }

    pub fn has_scope<T: Scope>(&self) -> bool {
        self.scopes.contains_key(<T as Scope>::scope_key())
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

        if !self.scopes.contains_key(scope_key) {
            return Ok(Box::default());
        }

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let data = &self.scopes[scope_key];
        let owned_value = data.clone();
        let mut scope: Box<T> = match owned_value {
            ScopeValue::Original(v)
            | ScopeValue::Intermediate {
                value: v,
                original: _,
            } => serde_json::from_value(v)?,
        };

        let _prepare_span = span!(Level::TRACE, "prepare").entered();
        let session = get_my_session()?;
        scope.supply(&session)?;

        Ok(scope)
    }

    pub fn replace_scope<T: Scope>(&mut self, scope: &T) -> Result<(), DomainError> {
        let scope_key = <T as Scope>::scope_key();

        let _span = span!(
            Level::TRACE,
            "scope",
            key = self.key.key_to_string(),
            scope = scope_key
        )
        .entered();

        let value = scope.serialize()?;

        debug!("scope-replace");

        // TODO Would love to just take the value.
        let original = match self.scopes.get(scope_key) {
            Some(value) => Some(value.clone().into()),
            None => None.into(),
        };

        let value = ScopeValue::Intermediate { value, original };

        self.scopes.insert(scope_key.to_owned(), value);

        Ok(())
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value, DomainError> {
        Ok(serde_json::to_value(self)?)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub(super) key: EntityKey,
    #[serde(rename = "klass")]
    pub(super) class: String,
    pub(super) name: String,
    pub(super) gid: Option<EntityGid>,
    #[serde(skip)]
    pub(super) entity: Option<Weak<RefCell<Entity>>>,
}

impl Default for EntityRef {
    fn default() -> Self {
        Self {
            key: EntityKey::blank(),
            class: Default::default(),
            name: Default::default(),
            gid: Default::default(),
            entity: Default::default(),
        }
    }
}

impl EntityRef {
    pub(crate) fn new_from_raw(entity: &Rc<RefCell<Entity>>) -> Self {
        let shared_entity = entity.borrow();
        Self {
            key: shared_entity.key().clone(),
            class: shared_entity.class().to_owned(),
            name: shared_entity.name().unwrap_or_default(),
            gid: shared_entity.gid(),
            entity: Some(Rc::downgrade(entity)),
        }
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn has_entity(&self) -> bool {
        self.entity.is_some()
    }
}

impl std::fmt::Debug for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityRef")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("gid", &self.gid)
            .finish()
    }
}
