use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    acls: Acls,
    value: serde_json::Value,
}

impl Property {
    pub fn new(value: serde_json::Value) -> Self {
        Self {
            acls: Default::default(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Props {
    map: HashMap<String, Property>,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl Props {
    fn property_named(&self, name: &str) -> Option<&Property> {
        if self.map.contains_key(name) {
            return Some(self.map.index(name));
        }
        None
    }

    fn string_property(&self, name: &str) -> Option<String> {
        if let Some(property) = self.property_named(name) {
            match &property.value {
                serde_json::Value::String(v) => Some(v.to_string()),
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
                serde_json::Value::Number(v) => v.as_u64(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn set_property(&mut self, name: &str, value: serde_json::Value) {
        self.map.insert(name.to_string(), Property::new(value));
    }

    fn set_u64_property(&mut self, name: &str, value: u64) -> Result<(), DomainError> {
        self.map
            .insert(name.to_owned(), Property::new(serde_json::to_value(value)?));

        Ok(())
    }

    pub(super) fn remove_property(&mut self, name: &str) -> Result<()> {
        self.map.remove(name);

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Properties {
    props: Option<Props>,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            props: Some(Default::default()),
        }
    }
}

impl Into<Properties> for Props {
    fn into(self) -> Properties {
        Properties { props: Some(self) }
    }
}

pub trait CoreProps {
    fn props(&self) -> Props;
    fn set_props(&mut self, props: Props) -> Result<(), DomainError>;
    fn name(&self) -> Option<String>;
    fn set_name(&mut self, value: &str) -> Result<(), DomainError>;
    fn gid(&self) -> Option<EntityGid>;
    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError>;
    fn desc(&self) -> Option<String>;
    fn set_desc(&mut self, value: &str) -> Result<(), DomainError>;
    fn destroy(&mut self) -> Result<(), DomainError>;
}

fn load_props(entity: &Entity) -> Result<Box<Properties>, DomainError> {
    let mut scope = entity.load_scope::<Properties>()?;

    if scope.props.is_none() {
        scope.props = Some(Props::default());
    }

    Ok(scope)
}

fn save_props(entity: &mut Entity, properties: Box<Properties>) -> Result<(), DomainError> {
    entity.replace_scope::<Properties>(&properties)
}

impl CoreProps for Entity {
    fn props(&self) -> Props {
        let scope = load_props(self).expect("Failed to load properties scope");

        scope.props.unwrap()
    }

    fn set_props(&mut self, props: Props) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.props = Some(props);
        save_props(self, scope)
    }

    fn name(&self) -> Option<String> {
        let scope = load_props(self).expect("Failed to load properties scope");

        scope.name()
    }

    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.set_name(value)?;
        save_props(self, scope)
    }

    fn gid(&self) -> Option<EntityGid> {
        let scope = load_props(self).expect("Failed to load properties scope");

        scope.gid()
    }

    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError> {
        let mut scope = load_props(self).expect("Failed to load properties scope");
        scope.set_gid(gid)?;
        save_props(self, scope)
    }

    fn desc(&self) -> Option<String> {
        let scope = load_props(self).expect("Failed to load properties scope");

        scope.desc()
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

impl CoreProps for Properties {
    fn props(&self) -> Props {
        self.props.clone().unwrap()
    }

    fn set_props(&mut self, _props: Props) -> Result<(), DomainError> {
        unimplemented!()
    }

    fn name(&self) -> Option<String> {
        self.props.as_ref().unwrap().string_property(NAME_PROPERTY)
    }

    fn set_name(&mut self, value: &str) -> Result<(), DomainError> {
        let value: serde_json::Value = value.into();
        self.props
            .as_mut()
            .unwrap()
            .set_property(NAME_PROPERTY, value);

        Ok(())
    }

    fn gid(&self) -> Option<EntityGid> {
        self.props
            .as_ref()
            .unwrap()
            .u64_property(GID_PROPERTY)
            .map(EntityGid)
    }

    fn set_gid(&mut self, gid: EntityGid) -> Result<(), DomainError> {
        self.props
            .as_mut()
            .unwrap()
            .set_u64_property(GID_PROPERTY, gid.into())
    }

    fn desc(&self) -> Option<String> {
        self.props.as_ref().unwrap().string_property(DESC_PROPERTY)
    }

    fn set_desc(&mut self, value: &str) -> Result<(), DomainError> {
        let value: serde_json::Value = value.into();
        self.props
            .as_mut()
            .unwrap()
            .set_property(DESC_PROPERTY, value);

        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DomainError> {
        let value: serde_json::Value = true.into();
        self.props
            .as_mut()
            .unwrap()
            .set_property(DESTROYED_PROPERTY, value);

        Ok(())
    }
}

impl Needs<SessionRef> for Properties {
    fn supply(&mut self, _session: &SessionRef) -> Result<()> {
        Ok(())
    }
}

impl Scope for Properties {
    fn serialize(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self)?)
    }

    fn scope_key() -> &'static str {
        "props"
    }
}
