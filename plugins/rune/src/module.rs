use kernel::{
    common::Json,
    prelude::{DomainError, EntityKey, EntityPtr, IntoEntityPtr, LookupBy},
    session::get_my_session,
};
use rune::runtime::{Object, Protocol};
use serde::Deserialize;

use crate::sources::{Owner, Relation};

use super::*;

#[derive(rune::Any, Debug)]
pub(super) struct BeforePerform(pub(super) Perform);

impl BeforePerform {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self.0)
    }
}

#[derive(rune::Any, Debug)]
pub(super) struct AfterEffect(pub(super) Effect);

impl AfterEffect {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self.0)
    }
}

#[derive(Debug, rune::Any)]
pub(super) struct Bag(pub(super) Json);

impl Bag {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self)
    }

    fn get(&self, key: &str) -> Option<LocalEntity> {
        self.0
            .tagged(key)
            .map(|r| r.value().clone().into_inner())
            .and_then(|r| KeyOnly::from_json(r).ok())
            .map(|r| r.to_entity())
            // Right now we can be reasonable sure that there are no dangling EntityRef's
            // nearby. This is still a bummer, though.
            .map(|r| r.unwrap())
            .map(|r| LocalEntity(r))
    }

    fn item(&self) -> Option<LocalEntity> {
        self.get("item")
    }

    fn area(&self) -> Option<LocalEntity> {
        self.get("area")
    }

    fn actor(&self) -> Option<LocalEntity> {
        self.get("actor")
    }
}

#[derive(Debug, rune::Any)]
pub(super) struct LocalEntity(EntityPtr);

impl LocalEntity {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self.0)
    }

    fn key(&self) -> String {
        self.0.key().key_to_string().to_owned()
    }

    fn name(&self) -> String {
        self.0.name().expect("Error getting name").unwrap()
    }
}

fn action_factory(
    _plugin_name: &str,
    action_name: &str,
    value: rune::Value,
) -> Result<Object, anyhow::Error> {
    let args: Object = match value.clone() {
        rune::Value::Object(args) => args.borrow_ref()?.clone(),
        rune::Value::Any(_args) => {
            let args: ActionArgs = rune::from_value(value)?;
            args.to_rune_object()?
        }
        _ => panic!("Unexpected action arguments: {:?}", value),
    };

    let mut action = Object::new();
    action.insert(action_name.to_owned(), args.into());
    Ok(action)
}

pub(super) fn create(schema: &SchemaCollection, owner: Option<Owner>) -> Result<rune::Module> {
    let mut module = rune::Module::default();
    for (plugin, actions) in schema.actions() {
        for action in actions {
            let function_name = action.0.trim_end_matches("Action");
            trace!("declaring 'actions.{}.{}'", plugin, function_name);
            module.function(["actions", &plugin, &function_name], {
                let plugin = plugin.to_owned();
                let action = action.to_owned();
                move |v: rune::Value| action_factory(&plugin, &action.0, v)
            })?;
        }
    }
    module.function(["owner"], move || owner.clone())?;
    module.function(["info"], |s: &str| {
        info!(target: "RUNE", "{}", s);
    })?;
    module.function(["debug"], |s: &str| {
        debug!(target: "RUNE", "{}", s);
    })?;
    module.ty::<ActionArgs>()?;
    module.associated_function(Protocol::STRING_DEBUG, ActionArgs::string_debug)?;
    module.ty::<BeforePerform>()?;
    module.associated_function(Protocol::STRING_DEBUG, BeforePerform::string_debug)?;
    module.ty::<AfterEffect>()?;
    module.associated_function(Protocol::STRING_DEBUG, AfterEffect::string_debug)?;
    module.ty::<Bag>()?;
    module.associated_function(Protocol::STRING_DEBUG, Bag::string_debug)?;
    module.associated_function("area", Bag::area)?;
    module.associated_function("item", Bag::item)?;
    module.associated_function("actor", Bag::actor)?;
    module.ty::<LocalEntity>()?;
    module.associated_function(Protocol::STRING_DEBUG, LocalEntity::string_debug)?;
    module.associated_function("key", LocalEntity::key)?;
    module.associated_function("name", LocalEntity::name)?;
    module.ty::<Owner>()?;
    module.associated_function(Protocol::STRING_DEBUG, Owner::string_debug)?;
    module.associated_function("key", Owner::key)?;
    module.associated_function("relation", Owner::relation)?;
    module.ty::<Relation>()?;
    module.ty::<RuneState>()?;
    module.associated_function(Protocol::STRING_DEBUG, RuneState::string_debug)?;
    module.function(["RuneState", "new"], || RuneState::default())?;
    Ok(module)
}

#[derive(Debug, Deserialize)]
struct KeyOnly {
    key: EntityKey,
}

impl KeyOnly {
    fn from_json(value: JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

impl IntoEntityPtr for KeyOnly {
    fn to_entity(&self) -> Result<EntityPtr, DomainError> {
        if !self.key.valid() {
            return Err(DomainError::InvalidKey);
        }
        get_my_session()?
            .entity(&LookupBy::Key(&self.key))?
            .ok_or(DomainError::DanglingEntity)
    }
}

#[derive(Default, rune::Any, Debug, Serialize)]
#[rune(constructor)]
pub struct ActionArgs {
    #[rune(get, set)]
    here: Option<String>,
}

impl ActionArgs {
    #[inline]
    fn string_debug(&self, s: &mut String) -> std::fmt::Result {
        use std::fmt::Write;
        write!(s, "{:?}", self)
    }
}

pub trait ToRuneObject {
    fn to_rune_object(&self) -> Result<rune::runtime::Object>;
}

impl ToRuneObject for ActionArgs {
    fn to_rune_object(&self) -> Result<rune::runtime::Object> {
        match &self.here {
            Some(here) => {
                let mut obj = Object::new();
                obj.insert("here".to_owned(), here.to_owned().into());
                Ok(obj)
            }
            None => Ok(Object::new().into()),
        }
    }
}
