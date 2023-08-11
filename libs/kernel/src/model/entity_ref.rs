use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use super::base::{DomainError, EntityGid, EntityKey, JsonValue};
use super::{CoreProps, Entity};

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

impl Into<EntityKey> for EntityRef {
    fn into(self) -> EntityKey {
        self.key
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

#[derive(Default, Deserialize, Debug)]
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
                Ok(potential) => match potential.good_enough() {
                    Some(entity_ref) => {
                        return Some(vec![entity_ref]);
                    }
                    None => {}
                },
                _ => {}
            }

            Some(
                o.iter()
                    .map(|(_k, v)| find_entity_refs(v))
                    .flatten()
                    .flatten()
                    .collect(),
            )
        }
    }
}
