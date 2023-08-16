use anyhow::Result;
use std::ops::Deref;
use std::{cell::RefCell, rc::Rc};

use replies::{JsonValue, TaggedJson, ToTaggedJson};

use crate::actions::{Action, Performer};
use crate::hooks::ManagedHooks;
use crate::model::{
    Audience, DomainError, EntityKey, EntityPtr, EntityPtrResolver, Identity, Item, When,
};
use crate::model::{Entity, EvaluationError};
use crate::surround::Surroundings;

pub type SessionRef = Rc<dyn ActiveSession>;

pub enum Raising {
    TaggedJson(TaggedJson),
}

impl From<Raising> for TaggedJson {
    fn from(value: Raising) -> Self {
        match value {
            Raising::TaggedJson(tagged) => tagged,
        }
    }
}

pub trait ActiveSession: Performer + EntityPtrResolver {
    fn try_deserialize_action(&self, value: &JsonValue)
        -> Result<Box<dyn Action>, EvaluationError>;

    fn find_item(
        &self,
        surroundings: &Surroundings,
        item: &Item,
    ) -> Result<Option<EntityPtr>, DomainError>;

    fn add_entity(&self, entity: Entity) -> Result<EntityPtr, DomainError>;

    fn obliterate(&self, entity: &EntityPtr) -> Result<(), DomainError>;

    fn new_key(&self) -> EntityKey;

    fn new_identity(&self) -> Identity;

    fn raise(&self, audience: Audience, raising: Raising) -> Result<(), DomainError>;

    fn hooks(&self) -> &ManagedHooks;

    fn schedule(
        &self,
        key: &str,
        when: When,
        message: &dyn ToTaggedJson,
    ) -> Result<(), DomainError>;
}

thread_local! {
    static SESSION: RefCell<Option<std::rc::Weak<dyn ActiveSession>>> = RefCell::new(None)
}

pub fn get_my_session() -> Result<SessionRef> {
    SESSION.with(|s| match &*s.borrow() {
        Some(s) => match s.upgrade() {
            Some(s) => Ok(s),
            None => Err(DomainError::ExpiredSession.into()),
        },
        None => Err(DomainError::NoSession.into()),
    })
}

pub struct SetSession<T> {
    session: std::rc::Rc<T>,
    previous: Option<std::rc::Weak<dyn ActiveSession>>,
}

impl<T> SetSession<T>
where
    T: ActiveSession + 'static,
{
    pub fn new(session: Rc<T>) -> Self {
        SESSION.with(|setting| {
            let mut setting = setting.borrow_mut();
            let previous = setting.take();

            let weak = Rc::downgrade(&session);
            *setting = Some(weak.clone());

            Self { previous, session }
        })
    }
}

impl<T> Deref for SetSession<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl<T> Drop for SetSession<T> {
    fn drop(&mut self) {
        SESSION.with(|setting| {
            let mut setting = setting.borrow_mut();
            *setting = self.previous.take();
        });
    }
}
