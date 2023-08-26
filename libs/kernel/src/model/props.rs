use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    acls: Option<Acls>,
    value: JsonValue,
}

impl Property {
    pub fn new(value: JsonValue) -> Self {
        Self {
            acls: Default::default(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Props(HashMap<String, Property>);

impl Props {
    fn property_named(&self, name: &str) -> Option<&Property> {
        if self.0.contains_key(name) {
            return Some(self.0.index(name));
        }
        None
    }

    fn string_property(&self, name: &str) -> Option<String> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                JsonValue::String(v) => Some(v.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    // TODO Make the next few functions.
    fn u64_property(&self, name: &str) -> Option<u64> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                JsonValue::Number(v) => v.as_u64(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn set_property(&mut self, name: &str, value: JsonValue) {
        self.0.insert(name.to_string(), Property::new(value));
    }

    fn set_u64_property(&mut self, name: &str, value: u64) -> Result<(), DomainError> {
        self.0
            .insert(name.to_owned(), Property::new(serde_json::to_value(value)?));

        Ok(())
    }

    pub(super) fn remove_property(&mut self, name: &str) {
        self.0.remove(name);
    }
}

impl From<Props> for HashMap<String, Property> {
    fn from(value: Props) -> Self {
        value.0
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Properties {
    core: Option<Props>,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            core: Some(Default::default()),
        }
    }
}

impl From<Props> for Properties {
    fn from(value: Props) -> Self {
        Self {
            core: Some(value.clone()),
        }
    }
}

pub trait HasProps<T> {
    fn props(&self) -> Props;
}

pub trait CoreProps {
    fn name(&self) -> String;
    fn gid(&self) -> Option<EntityGid>;
    fn desc(&self) -> Option<String>;
}

impl HasProps<Entity> for Entity {
    fn props(&self) -> Props {
        let scope = load_props(self).expect("Failed to load properties scope");
        scope.core.unwrap()
    }
}

impl HasProps<Properties> for Properties {
    fn props(&self) -> Props {
        self.core.clone().unwrap()
    }
}

impl Scope for Properties {
    fn scope_key() -> &'static str {
        "props"
    }
}

fn load_props(entity: &Entity) -> Result<Box<Properties>, DomainError> {
    Ok(Box::new(
        entity
            .scope::<Properties>()?
            .map(|v| v.into())
            .unwrap_or_default(),
    ))
}

fn save_props(entity: &mut Entity, properties: Box<Properties>) -> Result<(), DomainError> {
    entity.replace_scope::<Properties>(&properties)
}

impl<T: HasProps<T>> CoreProps for T {
    fn name(&self) -> String {
        self.props()
            .string_property(NAME_PROPERTY)
            .expect("Entity name missing")
    }

    fn gid(&self) -> Option<EntityGid> {
        self.props().u64_property(GID_PROPERTY).map(EntityGid::new)
    }

    fn desc(&self) -> Option<String> {
        self.props().string_property(DESC_PROPERTY)
    }
}

pub trait MutCoreProps {
    fn set_name(&mut self, value: &str) -> Result<(), DomainError>;
    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError>;
    fn set_desc(&mut self, value: &str) -> Result<(), DomainError>;
    fn destroy(&mut self) -> Result<(), DomainError>;
}

impl MutCoreProps for Entity {
    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.set_name(value)?;
        save_props(self, scope)
    }

    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.set_gid(gid)?;
        save_props(self, scope)
    }

    fn set_desc(&mut self, value: &str) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.set_desc(value)?;
        save_props(self, scope)
    }

    fn destroy(&mut self) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.destroy()?;
        save_props(self, scope)
    }
}

impl MutCoreProps for Properties {
    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let value: JsonValue = value.into();
        self.core
            .as_mut()
            .unwrap()
            .set_property(NAME_PROPERTY, value);

        Ok(())
    }

    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError> {
        self.core
            .as_mut()
            .unwrap()
            .set_u64_property(GID_PROPERTY, gid.into())
    }

    fn set_desc(&mut self, value: &str) -> Result<(), DomainError> {
        let value: JsonValue = value.into();
        self.core
            .as_mut()
            .unwrap()
            .set_property(DESC_PROPERTY, value);

        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DomainError> {
        let value: JsonValue = true.into();
        self.core
            .as_mut()
            .unwrap()
            .set_property(DESTROYED_PROPERTY, value);

        Ok(())
    }
}
