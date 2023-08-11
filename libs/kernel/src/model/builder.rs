use anyhow::Result;
use std::collections::HashMap;

use super::{
    base::{DomainError, EntityClass, EntityKey, Identity, GID_PROPERTY},
    CoreProps, Entity, EntityRef, HasScopes, Properties, Scope, ScopeMap, ScopeValue,
};
use crate::session::get_my_session;

pub struct EntityBuilder {
    key: Option<EntityKey>,
    class: EntityClass,
    parent: Option<EntityRef>,
    identity: Option<Identity>,
    creator: Option<EntityRef>,
    scopes: Option<ScopeMap>,
    properties: Properties,
}

impl Default for EntityBuilder {
    fn default() -> Self {
        Self::new()
    }
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
        let scopes: ScopeMap = template.scopes.clone().into();
        let properties = scopes.scopes().load_scope::<Properties>()?;
        let mut props = properties.props();
        props.remove_property(GID_PROPERTY);
        self.properties = props.into();
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
