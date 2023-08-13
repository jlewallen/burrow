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

impl From<EntityRef> for EntityKey {
    fn from(value: EntityRef) -> Self {
        value.key
    }
}

impl From<&Entity> for EntityRef {
    fn from(value: &Entity) -> Self {
        Self {
            key: value.key().clone(),
            class: value.class().to_owned(),
            name: value.name(),
            gid: value.gid(),
            entity: None,
        }
    }
}

impl EntityRef {
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
        JsonValue::Array(array) => {
            Some(array.iter().flat_map(find_entity_refs).flatten().collect())
        }
        JsonValue::Object(o) => {
            let potential = serde_json::from_value::<PotentialRef>(value.clone());

            // If this object is an EntityRef, we can stop looking, otherwise we
            // need to keep going deeper.

            if let Ok(potential) = potential {
                if let Some(entity_ref) = potential.good_enough() {
                    return Some(vec![entity_ref]);
                }
            }

            Some(
                o.iter()
                    .flat_map(|(_k, v)| find_entity_refs(v))
                    .flatten()
                    .collect(),
            )
        }
    }
}

mod exp {
    use std::{cell::RefCell, collections::HashMap};

    use serde::{Deserialize, Serialize};

    use crate::model::{EntityKey, JsonValue, Scope, ScopeMap};

    type LocalType = Option<HashMap<String, JsonValue>>;

    thread_local! {
        static MAP: RefCell<LocalType> = RefCell::new(None)
    }

    struct SetMap {
        previous: LocalType,
    }

    #[allow(dead_code)]
    fn get_from_local(key: &str) -> Option<JsonValue> {
        MAP.with(|setting| {
            let reading = setting.borrow();
            if let Some(map) = &*reading {
                map.get(key).cloned()
            } else {
                None
            }
        })
    }

    impl SetMap {
        #[allow(dead_code)]
        fn new(map: HashMap<String, JsonValue>) -> Self {
            MAP.with(|setting| {
                let mut setting = setting.borrow_mut();
                let previous = setting.take();

                *setting = Some(map);

                Self { previous }
            })
        }
    }

    impl Drop for SetMap {
        fn drop(&mut self) {
            MAP.with(|setting| {
                let mut setting = setting.borrow_mut();
                *setting = self.previous.take();
            });
        }
    }

    #[derive(Debug, Serialize)]
    struct PreloadedEntityRef {
        key: EntityKey,
    }

    #[derive(Debug, Deserialize)]
    struct KeyOnly {
        key: EntityKey,
    }

    impl<'de> Deserialize<'de> for PreloadedEntityRef {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = JsonValue::deserialize(deserializer)?;
            let key: KeyOnly = serde_json::from_value(value).unwrap();

            Ok(Self { key: key.key })
        }
    }

    #[derive(Default, Serialize, Deserialize)]
    struct ExampleEntity {
        creator: Option<PreloadedEntityRef>,
        scopes: ScopeMap,
    }

    #[derive(Default, Serialize, Deserialize, Debug)]
    struct ExampleScope {
        things: Vec<PreloadedEntityRef>,
    }

    impl Scope for ExampleScope {
        fn scope_key() -> &'static str
        where
            Self: Sized,
        {
            "example"
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::HashMap;

        use serde_json::json;

        use crate::model::{JsonValue, ScopeValue};

        use super::*;

        #[test]
        pub fn test_serializes() {
            let example = ExampleScope {
                things: vec![
                    PreloadedEntityRef {
                        key: "key-1".into(),
                    },
                    PreloadedEntityRef {
                        key: "key-2".into(),
                    },
                ],
            };
            let scopes = [(
                "example".to_owned(),
                ScopeValue::Original(serde_json::to_value(example).unwrap().into()),
            )]
            .into_iter()
            .collect::<HashMap<_, _>>();
            let entity = ExampleEntity {
                creator: Some(PreloadedEntityRef {
                    key: "jacob".into(),
                }),
                scopes: scopes.into(),
            };
            let value = serde_json::to_value(entity).unwrap();
            assert_eq!(
                value,
                json!({
                    "creator": { "key": "jacob" },
                    "scopes": {
                        "example": {
                            "things": [{
                                "key": "key-1",
                            },{
                                "key": "key-2",
                            }]
                        }
                    }
                })
            );
        }

        #[test]
        pub fn test_deserializes_simple() {
            let json: JsonValue = json!({
                "creator": { "key": "jacob" },
                "scopes": {
                    "example": {
                        "things": [{
                            "key": "key-1",
                        },{
                            "key": "key-2",
                        }]
                    }
                }
            });

            let _example: ExampleEntity = serde_json::from_value(json).unwrap();
        }
    }
}
