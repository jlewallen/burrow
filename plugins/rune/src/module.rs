use kernel::{
    common::Json,
    prelude::{DomainError, EntityKey, EntityPtr, IntoEntityPtr, LookupBy},
    session::get_my_session,
};
use rune::runtime::Protocol;
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

    fn living(&self) -> Option<LocalEntity> {
        self.get("living")
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

#[derive(rune::Any, Debug)]
struct RuneActions {}

pub(super) fn create(schema: &SchemaCollection, owner: Option<Owner>) -> Result<rune::Module> {
    let mut module = rune::Module::default();
    module.ty::<RuneActions>()?;
    for (plugin, action) in schema.actions() {
        let action = action.trim_end_matches("Action");
        trace!("declaring 'actions.{}.{}'", plugin, action);
        module.function(
            ["actions", plugin, action],
            move || -> std::result::Result<rune::Value, anyhow::Error> { Ok(rune::Value::Unit) },
        )?;
    }
    module.function(["owner"], move || owner.clone())?;
    module.function(["info"], |s: &str| {
        info!(target: "RUNE", "{}", s);
    })?;
    module.function(["debug"], |s: &str| {
        debug!(target: "RUNE", "{}", s);
    })?;
    module.ty::<BeforePerform>()?;
    module.inst_fn(Protocol::STRING_DEBUG, BeforePerform::string_debug)?;
    module.ty::<AfterEffect>()?;
    module.inst_fn(Protocol::STRING_DEBUG, AfterEffect::string_debug)?;
    module.ty::<Bag>()?;
    module.inst_fn(Protocol::STRING_DEBUG, Bag::string_debug)?;
    module.inst_fn("area", Bag::area)?;
    module.inst_fn("item", Bag::item)?;
    module.inst_fn("living", Bag::living)?;
    module.ty::<LocalEntity>()?;
    module.inst_fn(Protocol::STRING_DEBUG, LocalEntity::string_debug)?;
    module.inst_fn("key", LocalEntity::key)?;
    module.inst_fn("name", LocalEntity::name)?;
    module.ty::<Owner>()?;
    module.inst_fn(Protocol::STRING_DEBUG, Owner::string_debug)?;
    module.inst_fn("key", Owner::key)?;
    module.inst_fn("relation", Owner::relation)?;
    module.ty::<Relation>()?;
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
