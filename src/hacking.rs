use anyhow::Result;
use std::fmt::Debug;
use std::rc::{Rc, Weak};
use tracing::{info, trace};

use crate::domain::DevNullNotifier;
use crate::domain::{self, Session};
use crate::kernel::{DomainError, EntityKey, LazyLoadedEntity, LoadEntities, Scope, WORLD_KEY};
use crate::plugins::carrying::model::{Carryable, Containing};
use crate::plugins::moving::model::Occupying;
use crate::plugins::users::model::Usernames;
use crate::storage;
use crate::text::Renderer;

#[derive(Clone)]
pub struct Entry {
    key: EntityKey,
    session: Weak<BetterSession>,
}

impl Entry {
    pub fn scope<T: Scope>(&self) -> Result<OpenScope<T>> {
        Ok(OpenScope::new(
            self.session
                .upgrade()
                .expect("No session in Entry::scope")
                .scope::<T>(self)?,
        ))
    }

    pub fn scope_mut<T: Scope>(&self) -> Result<OpenScopeMut<T>> {
        Ok(OpenScopeMut::new(
            Weak::clone(&self.session),
            self,
            self.session
                .upgrade()
                .expect("No session in Entry::scope")
                .scope::<T>(self)?,
        ))
    }
}

impl TryFrom<LazyLoadedEntity> for Option<Entry> {
    type Error = DomainError;

    fn try_from(value: LazyLoadedEntity) -> Result<Self, Self::Error> {
        let session = get_my_better_session().expect("No active better session");
        Ok(session.entry(&value.key)?)
    }
}

impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry").field("key", &self.key).finish()
    }
}

pub struct BetterSession {
    session: Rc<Session>,
    weak: Weak<BetterSession>,
}

impl BetterSession {
    pub fn entry(&self, key: &EntityKey) -> Result<Option<Entry>> {
        match self.session.load_entity_by_key(key)? {
            Some(_) => Ok(Some(Entry {
                key: key.clone(),
                session: Weak::clone(&self.weak),
            })),
            None => Ok(None),
        }
    }

    pub fn scope<T: Scope>(&self, entry: &Entry) -> Result<Box<T>> {
        let entity = match self.session.load_entity_by_key(&entry.key)? {
            None => panic!("How did you get an Entry for an unknown Entity?"),
            Some(entity) => entity,
        };

        info!("{:?} scope", entity);

        let entity = entity.borrow();

        entity.scope_hack::<T>()
    }

    pub fn close(&self) -> Result<()> {
        self.session.close(&DevNullNotifier {})
    }

    pub fn save<T: Scope>(&self, entry: &Entry, scope: &Box<T>) -> Result<()> {
        let entity = self.session.load_entity_by_key(&entry.key)?.unwrap();
        let mut entity = entity.borrow_mut();

        entity.replace_scope::<T>(scope)
    }
}

pub fn execute_command() -> Result<()> {
    let _renderer = Renderer::new()?;
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = domain::Domain::new(storage_factory, false);
    let session = domain.open_session()?;
    let session = Rc::new_cyclic(move |weak| BetterSession {
        session: session,
        weak: Weak::clone(weak),
    });
    set_my_better_session(Some(&session))?;

    let world = session.entry(&WORLD_KEY)?.expect("No 'WORLD' entity.");
    let usernames = world.scope::<Usernames>()?;
    let user_key = &usernames.users["jlewallen"];
    let user = session.entry(user_key)?.expect("No 'USER' entity.");

    let occupying = user.scope::<Occupying>()?;
    let area: Option<Entry> = occupying.area.clone().try_into()?; // TODO Annoying clone

    let holding = user
        .scope::<Containing>()?
        .holding
        .iter()
        .map(|i| -> Result<Option<Entry>> { Ok(i.clone().try_into()?) }) // TODO Annoying clone
        .collect::<Result<Vec<_>>>()?;

    if let Some(item) = &holding[0] {
        let mut carryable = item.scope_mut::<Carryable>()?;
        info!("quantity = {}", carryable.quantity());
        carryable.set_quantity(1.0)?;
        carryable.save()?;
    }

    info!("{:?}", holding[0]);
    info!("{:?}", user);
    info!("{:?}", area);

    session.close()?;
    set_my_better_session(None)?;

    Ok(())
}

thread_local! {
    #[allow(unused)]
    static BETTER_SESSION: std::cell::RefCell<Option<std::rc::Weak<BetterSession>>>  = std::cell::RefCell::new(None)
}

fn set_my_better_session(session: Option<&std::rc::Rc<BetterSession>>) -> Result<()> {
    BETTER_SESSION.with(|s| {
        *s.borrow_mut() = match session {
            Some(session) => Some(std::rc::Rc::downgrade(session)),
            None => None,
        };

        Ok(())
    })
}

fn get_my_better_session() -> Result<std::rc::Rc<BetterSession>> {
    BETTER_SESSION.with(|s| match &*s.borrow() {
        Some(s) => match s.upgrade() {
            Some(s) => Ok(s),
            None => Err(DomainError::ExpiredInfrastructure.into()),
        },
        None => Err(DomainError::NoInfrastructure.into()),
    })
}

pub struct OpenScope<T: Scope> {
    target: Box<T>,
}

impl<T: Scope> OpenScope<T> {
    pub fn new(target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self { target }
    }
}

impl<T: Scope> std::ops::Deref for OpenScope<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

pub struct OpenScopeMut<T: Scope> {
    session: Weak<BetterSession>,
    owner: Entry,
    target: Box<T>,
}

impl<T: Scope> OpenScopeMut<T> {
    pub fn new(session: Weak<BetterSession>, owner: &Entry, target: Box<T>) -> Self {
        trace!("scope-open {:?}", target);

        Self {
            session,
            owner: owner.clone(),
            target,
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.session
            .upgrade()
            .expect("No session")
            .save::<T>(&self.owner, &self.target)
    }
}

impl<T: Scope> Drop for OpenScopeMut<T> {
    fn drop(&mut self) {
        // TODO Check for unsaved changes to this scope and possibly warn the
        // user, this would require them to intentionally discard  any unsaved
        // changes. Not being able to bubble an error up makes doing anything
        // elaborate in here a bad idea.
        trace!("scope-dropped {:?}", self.target);
    }
}

impl<T: Scope> std::ops::Deref for OpenScopeMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.target.as_ref()
    }
}

impl<T: Scope> std::ops::DerefMut for OpenScopeMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.target.as_mut()
    }
}
