use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{fmt::Debug, rc::Rc};

use super::infra::*;
use super::model::*;
use super::ReplyResult;

pub type EvaluationResult = Result<Box<dyn Action>, EvaluationError>;

pub type ActionArgs = (EntityPtr, EntityPtr, EntityPtr, Rc<dyn Infrastructure>);

pub trait Action: Debug {
    fn perform(&self, args: ActionArgs) -> ReplyResult;

    fn is_read_only() -> bool
    where
        Self: Sized;
}

pub trait Scope: Debug + Default + Needs<Rc<dyn Infrastructure>> + DeserializeOwned {
    fn scope_key() -> &'static str
    where
        Self: Sized;

    fn serialize(&self) -> Result<Value>;
}

pub trait PrepareEntities {
    fn prepare_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr>;

    fn prepare_entity_by_gid(&self, gid: &EntityGID) -> Result<EntityPtr>;
}

pub trait LoadEntities {
    fn load_entity_by_key(&self, key: &EntityKey) -> Result<EntityPtr>;

    fn load_entity_by_gid(&self, gid: &EntityGID) -> Result<EntityPtr>;

    fn load_entity_by_ref(&self, entity_ref: &EntityRef) -> Result<EntityPtr> {
        self.load_entity_by_key(&entity_ref.key)
    }
}
