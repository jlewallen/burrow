use crate::{
    get_my_session, model::Needs, CoreProps, HasScopes, Properties, ScopeValue, Scopes, ScopesMut,
    SessionRef,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::{collections::HashMap, fmt::Display};

use super::base::*;

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
    #[serde(rename = "klass")] // TODO Rename, legacy from Python.
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
    // NOTE This should be easier with the Scopes stuff below, accept that as a
    // ctor parameter and just clone the map or create an alternative we can
    // take ownership of.
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

    pub fn to_json_value(&self) -> Result<serde_json::Value, DomainError> {
        Ok(serde_json::to_value(self)?)
    }
}

impl HasScopes for Entity {
    fn into_scopes(&self) -> Scopes {
        Scopes {
            key: &self.key,
            map: &self.scopes,
        }
    }

    fn into_scopes_mut(&mut self) -> ScopesMut {
        ScopesMut {
            key: &self.key,
            map: &mut self.scopes,
        }
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
