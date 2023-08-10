use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::str::FromStr;
use std::{collections::HashMap, fmt::Display};
use JsonValue;

use super::base::*;
use crate::Scope;
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
    pub fn from_value(value: JsonValue) -> Result<Entity, DomainError> {
        Ok(serde_json::from_value(value)?)
    }

    fn new_heavily_customized(
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

#[derive(Default, Deserialize)]
struct PotentialRef {
    key: Option<String>,
    #[serde(rename = "klass")] // TODO Python name collision.
    class: Option<String>,
    name: Option<String>,
    gid: Option<EntityGid>,
}

impl PotentialRef {
    fn good_enough(self) -> Option<EntityRef> {
        let Some(key) = self.key else {
            return None;
        };
        let Some(class) = self.class else {
            return None;
        };
        let Some(name) = self.name else {
            return None;
        };
        let Some(gid) = self.gid else {
            return None;
        };
        Some(EntityRef {
            key: EntityKey::new(&key),
            class,
            name: Some(name),
            gid: Some(gid),
            entity: None,
        })
    }
}

pub fn find_entity_refs(value: &JsonValue) -> Option<Vec<EntityRef>> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(_) => None,
        JsonValue::Number(_) => None,
        JsonValue::String(_) => None,
        JsonValue::Array(array) => Some(
            array
                .iter()
                .map(|e| find_entity_refs(e))
                .flatten()
                .flatten()
                .collect(),
        ),
        JsonValue::Object(o) => {
            let potential = serde_json::from_value::<PotentialRef>(value.clone());

            // If this object is an EntityRef, we can stop looking, otherwise we
            // need to keep going deeper.
            match potential {
                Ok(potential) => potential.good_enough().map(|i| vec![i]),
                Err(_) => Some(
                    o.iter()
                        .map(|(_k, v)| find_entity_refs(v))
                        .flatten()
                        .flatten()
                        .collect(),
                ),
            }
        }
    }
}

// TODO Make this generic across 'entity's type?
#[derive(Clone, Serialize, Deserialize)]
pub struct EntityRef {
    key: EntityKey,
    #[serde(rename = "klass")] // TODO Python name collision.
    class: String,
    name: Option<String>,
    gid: Option<EntityGid>,
    #[serde(skip)]
    entity: Option<Weak<RefCell<Entity>>>,
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

impl Into<EntityRef> for &Entity {
    fn into(self) -> EntityRef {
        EntityRef {
            key: self.key().clone(),
            class: self.class().to_owned(),
            name: self.name(),
            gid: self.gid(),
            entity: None,
        }
    }
}

impl EntityRef {
    pub(crate) fn new_from_raw(entity: &Rc<RefCell<Entity>>) -> Self {
        let shared_entity = entity.borrow();
        Self::new_from_entity(&shared_entity, Some(Rc::downgrade(entity)))
    }

    pub(crate) fn new_from_entity(entity: &Entity, shared: Option<Weak<RefCell<Entity>>>) -> Self {
        Self {
            key: entity.key().clone(),
            class: entity.class().to_owned(),
            name: entity.name(),
            gid: entity.gid(),
            entity: shared,
        }
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn has_entity(&self) -> bool {
        self.entity.is_some()
    }

    pub fn entity(&self) -> Result<Rc<RefCell<Entity>>, DomainError> {
        match &self.entity {
            Some(e) => match e.upgrade() {
                Some(e) => Ok(e),
                None => Err(DomainError::DanglingEntity),
            },
            None => Err(DomainError::DanglingEntity),
        }
    }
}

impl std::fmt::Debug for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match &self.name {
            Some(name) => name.to_owned(),
            None => "<none>".to_owned(),
        };
        if let Some(gid) = &self.gid {
            write!(f, "Entity(#{}, `{}`, {})", &gid, &name, &self.key)
        } else {
            write!(f, "Entity(?, `{}`, {})", &name, &self.key)
        }
    }
}

pub struct EntityBuilder {
    key: Option<EntityKey>,
    class: EntityClass,
    parent: Option<EntityRef>,
    identity: Option<Identity>,
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
            identity: None,
            scopes: None,
            class: EntityClass::item(),
            properties: Properties::default(),
        }
    }

    pub fn with_key(mut self, key: EntityKey) -> Self {
        self.key = Some(key);
        self
    }

    pub fn creator(mut self, value: EntityRef) -> Self {
        self.creator = Some(value);
        self
    }

    pub fn parent(mut self, value: EntityRef) -> Self {
        self.parent = Some(value);
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

    pub fn identity(mut self, identity: impl Into<Identity>) -> Self {
        self.identity = Some(identity.into());
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

    pub fn default_scope<T>(mut self) -> Result<Self>
    where
        T: Scope + Default,
    {
        if self.scopes.is_none() {
            self.scopes = Some(ScopeMap::default());
        }
        let scopes = self.scopes.as_mut().unwrap();
        let mut scopes = scopes.scopes_mut();
        scopes.replace_scope(&T::default())?;

        Ok(self)
    }
}

impl TryInto<Entity> for EntityBuilder {
    type Error = DomainError;

    fn try_into(self) -> Result<Entity, Self::Error> {
        let identity = match self.identity {
            Some(identity) => identity,
            None => get_my_session()?.new_identity(),
        };
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
            identity,
            self.creator,
            self.parent,
            scopes,
        ))
    }
}

pub fn build_entity() -> EntityBuilder {
    EntityBuilder::new()
}
