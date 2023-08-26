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

pub trait CoreProps {
    fn name(&self) -> String;
    fn gid(&self) -> Option<EntityGid>;
    fn desc(&self) -> Option<String>;
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

impl Into<Props> for Properties {
    fn into(self) -> Props {
        self.core.unwrap_or_default()
    }
}

impl Scope for Properties {
    fn scope_key() -> &'static str {
        "props"
    }
}

pub trait HasProps<T> {
    fn props(&self) -> Props;
}

impl HasProps<Entity> for Entity {
    fn props(&self) -> Props {
        let properties = self
            .scope::<Properties>()
            .expect("Failed to load properties scope")
            .unwrap();
        properties.clone().into()
    }
}

impl HasProps<Properties> for Properties {
    fn props(&self) -> Props {
        self.core.clone().unwrap()
    }
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

pub trait MutCoreProps<T> {
    fn set_name(&mut self, value: &str) -> Result<(), DomainError>;
    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError>;
    fn set_desc(&mut self, value: &str) -> Result<(), DomainError>;
    fn destroy(&mut self) -> Result<(), DomainError>;
}

impl<T: OpenScopeMut<T> + StoreScope> MutCoreProps<T> for T {
    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let mut properties = self.scope_mut::<Properties>()?;
        properties.set_name(value)?;
        properties.save(self)
    }

    fn set_gid(&mut self, value: EntityGid) -> Result<(), DomainError> {
        let mut properties = self.scope_mut::<Properties>()?;
        properties.set_gid(value)?;
        properties.save(self)
    }

    fn set_desc(&mut self, value: &str) -> Result<(), DomainError> {
        let mut properties = self.scope_mut::<Properties>()?;
        properties.set_desc(value)?;
        properties.save(self)
    }

    fn destroy(&mut self) -> Result<(), DomainError> {
        let mut properties = self.scope_mut::<Properties>()?;
        properties.destroy()?;
        properties.save(self)
    }
}

impl MutCoreProps<Properties> for Properties {
    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError> {
        self.core
            .as_mut()
            .unwrap()
            .set_u64_property(GID_PROPERTY, gid.into())
    }

    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let value: JsonValue = value.into();
        self.core
            .as_mut()
            .unwrap()
            .set_property(NAME_PROPERTY, value);

        Ok(())
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
