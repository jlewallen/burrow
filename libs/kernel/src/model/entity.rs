use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::str::FromStr;
use std::{collections::HashMap, fmt::Display};

use super::base::*;
use crate::{
    get_my_session, model::Needs, CoreProps, HasScopes, Properties, ScopeValue, Scopes, ScopesMut,
    SessionRef,
};

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

impl FromStr for Entity {
    type Err = DomainError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_value(serde_json::from_str(s)?)
    }
}

impl Entity {
    pub fn from_value(value: serde_json::Value) -> Result<Entity, DomainError> {
        Ok(serde_json::from_value(value)?)
    }

    fn new_heavily_customized(
        key: EntityKey,
        class: EntityClass,
        creator: Option<EntityRef>,
        parent: Option<EntityRef>,
        scopes: ScopeMap,
    ) -> Self {
        Self {
            key,
            parent,
            creator,
            identity: Default::default(),
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

    pub fn to_json_value(&self) -> Result<serde_json::Value, DomainError> {
        Ok(serde_json::to_value(self)?)
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

#[derive(Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub(super) key: EntityKey,
    #[serde(rename = "klass")]
    pub(super) class: String,
    pub(super) name: Option<String>,
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
            name: shared_entity.name(),
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

pub struct EntityBuilder {
    key: Option<EntityKey>,
    class: EntityClass,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    scopes: Option<ScopeMap>,
    properties: Properties,
}

impl EntityBuilder {
    pub fn new() -> Self {
        Self {
            key: None,
            parent: None,
            creator: None,
            scopes: None,
            class: EntityClass::item(),
            properties: Properties::default(),
        }
    }

    pub fn with_key(mut self, key: EntityKey) -> Self {
        self.key = Some(key);
        self
    }

    pub fn class(mut self, class: EntityClass) -> Self {
        self.class = class;
        self
    }

    pub fn name(mut self, s: &str) -> Self {
        self.properties.set_name(s).expect("Set name failed");
        self
    }

    pub fn desc(mut self, s: &str) -> Self {
        self.properties.set_desc(s).expect("Set desc failed");
        self
    }

    pub fn copying(mut self, template: &Entity) -> Result<Self> {
        let mut scopes: ScopeMap = template.scopes.clone().into();
        let properties = scopes.scopes().load_scope::<Properties>()?;
        let mut props = properties.props();
        props.remove_property(GID_PROPERTY);
        // TODO How can we avoid passing tthis generic argument?
        scopes
            .scopes_mut()
            .replace_scope::<Properties>(&properties)?;

        self.class = template.class.clone();
        self.creator = template.creator.clone();
        self.parent = template.parent.clone();
        self.scopes = Some(scopes);

        Ok(self)
    }

    pub fn area(self) -> Self {
        self.class(EntityClass::area())
    }

    pub fn exit(self) -> Self {
        self.class(EntityClass::exit())
    }

    pub fn living(self) -> Self {
        self.class(EntityClass::living())
    }
}

impl TryInto<Entity> for EntityBuilder {
    type Error = DomainError;

    fn try_into(self) -> Result<Entity, Self::Error> {
        let key = match self.key {
            Some(key) => key,
            None => get_my_session()?.new_key(),
        };
        let map = [(
            "props".to_owned(),
            ScopeValue::Original(serde_json::to_value(self.properties)?.into()),
        )]
        .into_iter()
        .collect::<HashMap<_, _>>();

        let scopes: ScopeMap = map.into();
        Ok(Entity::new_heavily_customized(
            key,
            self.class,
            self.creator,
            self.parent,
            scopes,
        ))
    }
}

pub fn build_entity() -> EntityBuilder {
    EntityBuilder::new()
}
