use anyhow::{anyhow, Error, Result};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info};

pub static WORLD_KEY: Lazy<EntityKey> = Lazy::new(|| "world".to_string());

pub trait Action {
    fn perform(&self) -> Result<()>;
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("unknown parsing human readable")]
    ParseError,
}

pub type EntityKey = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityRef {
    #[serde(alias = "py/object")]
    py_object: String,
    #[serde(alias = "py/ref")]
    py_ref: String,
    key: EntityKey,
    klass: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Identity {
    #[serde(alias = "py/object")]
    py_object: String,
    private: String,
    public: String,
    signature: Option<String>, // TODO Why does this happen?
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Kind {
    #[serde(alias = "py/object")]
    py_object: String,
    identity: Identity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityClass {
    #[serde(alias = "py/type")]
    py_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AclRule {
    #[serde(alias = "py/object")]
    py_object: String,
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Acls {
    #[serde(alias = "py/object")]
    py_object: String,
    rules: Vec<AclRule>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    #[serde(alias = "py/object")]
    py_object: String,
    i: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Property {
    value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Props {
    map: HashMap<String, Property>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entity {
    #[serde(alias = "py/object")]
    py_object: String,
    pub key: String,
    version: Version,
    parent: Option<EntityRef>,
    creator: Option<EntityRef>,
    identity: Identity,
    #[serde(alias = "klass")]
    class: EntityClass,
    acls: Acls,
    props: Props,
    scopes: HashMap<String, serde_json::Value>,
}

impl Entity {
    pub fn scope<T: Scope + DeserializeOwned>(&self) -> Result<Box<T>> {
        let key = <T as Scope>::scope_key();

        if !self.scopes.contains_key(key) {
            return Err(anyhow!("unable to create scope immutably"));
        }

        let data = &self.scopes[key];

        debug!(%data, "parsing");

        // The call to serde_json::from_value requires owned data and we have a
        // reference to somebody else's. Presumuably so that we don't couple the
        // lifetime of the returned object to the lifetime of the data being
        // referenced? What's the right solution here?
        // Should the 'un-parsed' Scope also owned the parsed data?
        let owned_value = data.clone();
        Ok(serde_json::from_value(owned_value)?)
    }
}

pub trait DomainEvent {}

pub trait Scope {
    fn scope_key() -> &'static str
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct DomainResult<T> {
    pub events: Vec<T>,
}
