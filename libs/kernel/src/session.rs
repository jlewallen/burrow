use anyhow::Result;
use std::ops::Deref;
use std::{cell::RefCell, rc::Rc};

use replies::ToJson;

use super::actions::Performer;
use super::model::{
    Audience, DomainError, DomainEvent, EntityKey, EntityPtr, EntityRef, Entry, EntryResolver,
    Identity, Item, When,
};
use super::{ManagedHooks, Surroundings};

pub type SessionRef = Rc<dyn ActiveSession>;

pub trait ActiveSession: Performer + EntryResolver {
    /// I think this will eventually need to return or take a construct that's
    /// builder-like so callers can take more control. Things to consider are:
    /// 1) Conditional needle visibility.
    /// 2) Items containing others.
    /// 3) Verb capabilities of the needle.
    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> Result<Option<Entry>>;

    fn find_optional_item(
        &self,
        surroundings: &Surroundings,
        item: Option<Item>,
    ) -> Result<Option<Entry>> {
        if let Some(item) = item {
            self.find_item(surroundings, &item)
        } else {
            Ok(None)
        }
    }

    fn ensure_entity(&self, entity_ref: &EntityRef) -> Result<EntityRef, DomainError>;

    fn ensure_optional_entity(&self, entity_ref: &Option<EntityRef>) -> Result<Option<EntityRef>> {
        match entity_ref {
            Some(e) => Ok(Some(self.ensure_entity(e)?)),
            None => Ok(None),
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry>;

    fn add_entities(&self, entities: &[&EntityPtr]) -> Result<Vec<Entry>> {
        entities.iter().map(|e| self.add_entity(e)).collect()
    }

    fn obliterate(&self, entity: &Entry) -> Result<()>;

    fn new_key(&self) -> EntityKey;

    fn new_identity(&self) -> Identity;

    fn raise(&self, audience: Audience, event: Box<dyn DomainEvent>) -> Result<()>;

    fn hooks(&self) -> &ManagedHooks;

    // We may want to just make `when` be something that can be Into'd a DateTime<Utc>?
    fn schedule(&self, key: &str, when: When, message: &dyn ToJson) -> Result<()>;
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
