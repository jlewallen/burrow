use anyhow::Result;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Instant;
use std::{
    cell::RefCell,
    env,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::{debug, info, span, trace, warn, Level};

mod internal;
mod state;

use super::sequences::Sequence;
use super::Notifier;
use crate::storage::Storage;
use crate::{identifiers, HasUsernames};
use kernel::*;
use state::State;

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn Storage>,
    weak: Weak<Session>,
    finder: Arc<dyn Finder>,
    plugins: Arc<RefCell<SessionPlugins>>,
    middleware: Arc<RefCell<Vec<Rc<dyn Middleware>>>>,
    hooks: ManagedHooks,

    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    state: Rc<State>,
}

impl Session {
    pub fn new(
        storage: Rc<dyn Storage>,
        keys: &Arc<dyn Sequence<EntityKey>>,
        identities: &Arc<dyn Sequence<Identity>>,
        finder: &Arc<dyn Finder>,
        registered_plugins: &Arc<RegisteredPlugins>,
    ) -> Result<Rc<Self>> {
        trace!("session-new");

        let opened = Instant::now();

        storage.begin()?;

        let plugins = registered_plugins.create_plugins()?;
        let plugins = Arc::new(RefCell::new(plugins));

        let hooks = {
            let plugins = plugins.borrow();
            plugins.hooks()?
        };

        let session = Rc::new_cyclic(move |weak: &Weak<Session>| Self {
            opened,
            storage,
            weak: Weak::clone(weak),
            open: AtomicBool::new(true),
            finder: Arc::clone(finder),
            plugins,
            middleware: Default::default(),
            hooks,
            keys: Arc::clone(keys),
            identities: Arc::clone(identities),
            state: Default::default(),
        });

        session.initialize()?;

        Ok(session)
    }

    pub fn set_session(&self) -> Result<SetSession> {
        Ok(SetSession::new(
            &self.weak.upgrade().ok_or(DomainError::NoSession)?,
        ))
    }

    pub fn evaluate_and_perform(&self, user_name: &str, text: &str) -> Result<Option<Effect>> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
        }

        let _activated = self.set_session()?;

        let action = {
            let plugins = self.plugins.borrow();
            plugins.try_parse_action(text)?
        };

        match action {
            Some(action) => {
                let living = user_name_to_entry(self, &user_name)?;
                let perform = Perform::Living {
                    living,
                    action: action.into(),
                };
                match self.perform(perform) {
                    Ok(Effect::Nothing) => Ok(None),
                    Ok(i) => Ok(Some(i)),
                    Err(original_err) => {
                        self.open.store(false, Ordering::Relaxed);
                        if let Err(_rollback_err) = self.storage.rollback(false) {
                            // TODO Include that this failed as part of the error.
                            panic!("TODO error rolling back");
                        }
                        Err(original_err)
                    }
                }
            }
            None => Ok(None),
        }
    }

    pub fn deliver(&self, incoming: Incoming) -> Result<()> {
        let _activated = self.set_session()?;

        let plugins = self.plugins.borrow();

        plugins.deliver(incoming)?;

        Ok(())
    }

    pub fn initialize(&self) -> Result<()> {
        let _activated = self.set_session()?;

        {
            let mut middleware = self.middleware.borrow_mut();
            middleware.extend({
                let mut plugins = self.plugins.borrow_mut();
                plugins.middleware()?
            });
        }

        let mut plugins = self.plugins.borrow_mut();
        plugins.initialize()?;

        Ok(())
    }

    pub fn flush<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let _activated = self.set_session()?;

        self.save_changes(notifier)?;

        self.storage.begin()
    }

    pub fn close<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let _activated = self.set_session()?;

        self.save_changes(notifier)?;

        let nentities = self.state.size();
        let elapsed = self.opened.elapsed();
        let elapsed = format!("{:?}", elapsed);

        let plugins = self.plugins.borrow();

        plugins.stop()?;

        info!(%elapsed, %nentities, "closed");

        self.open.store(false, Ordering::Relaxed);

        Ok(())
    }

    fn save_changes<T: Notifier>(&self, notifier: &T) -> Result<()> {
        match self.state.close(&self.storage, notifier, &self.finder) {
            Ok(changes) => {
                if changes {
                    if should_force_rollback() {
                        let _span = span!(Level::DEBUG, "FORCED").entered();
                        self.storage.rollback(true)
                    } else {
                        self.storage.commit()
                    }
                } else {
                    self.storage.rollback(true)
                }
            }
            Err(e) => {
                warn!("Save error, rolling back: {:?}", e);
                self.open.store(false, Ordering::Relaxed);
                if let Err(_rollback_err) = self.storage.rollback(false) {
                    // TODO Include that this failed as part of the error.
                    panic!("TODO error rolling back");
                }
                Err(e)
            }
        }
    }
}

impl Performer for Session {
    fn perform(&self, perform: Perform) -> Result<Effect> {
        let _span = span!(Level::DEBUG, "P").entered();

        debug!("perform {:?}", perform);

        match perform {
            Perform::Living { living, action } => {
                info!("perform:living");

                let surroundings = {
                    let make = MakeSurroundings {
                        finder: self.finder.clone(),
                        living: living.clone(),
                    };
                    let surroundings = make.try_into()?;
                    info!("surroundings {:?}", &surroundings);
                    let plugins = self.plugins.borrow();
                    plugins.have_surroundings(&surroundings)?;
                    surroundings
                };

                self.perform(Perform::Surroundings {
                    surroundings,
                    action,
                })
            }
            _ => {
                let target = self.state.clone();
                let request_fn = Box::new(move |value: Perform| -> Result<Effect, anyhow::Error> {
                    target.perform(value)
                });

                let middleware = self.middleware.borrow();
                apply_middleware(&middleware, perform, request_fn)
            }
        }
    }
}

impl LoadsEntities for Session {
    fn load_entity(&self, lookup: &LookupBy) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.state.lookup_entity(lookup)? {
            return Ok(Some(e));
        }

        let _loading_span =
            span!(Level::INFO, "entity", lookup = format!("{:?}", lookup)).entered();

        trace!("loading");
        if let Some(persisted) = self.storage.load(lookup)? {
            Ok(Some(self.state.add_persisted(persisted)?))
        } else {
            Ok(None)
        }
    }
}

impl EntryResolver for Session {
    fn entry(&self, lookup: &LookupBy) -> Result<Option<Entry>, DomainError> {
        match self.load_entity(lookup)? {
            Some(entity) => Ok(Some(Entry::new(&entity.key(), entity))),
            None => Ok(None),
        }
    }
}

impl ActiveSession for Session {
    fn new_key(&self) -> EntityKey {
        self.keys.following()
    }

    fn new_identity(&self) -> Identity {
        self.identities.following()
    }

    fn find_item(&self, surroundings: &Surroundings, item: &Item) -> Result<Option<Entry>> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        match item {
            Item::Gid(gid) => Ok(self.entry(&LookupBy::Gid(gid))?),
            _ => self.finder.find_item(surroundings, item),
        }
    }

    fn ensure_entity(&self, entity_ref: &EntityRef) -> Result<EntityRef, DomainError> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = &self.load_entity(&LookupBy::Key(entity_ref.key()))? {
            Ok(entity.into())
        } else {
            Err(DomainError::EntityNotFound)
        }
    }

    fn add_entity(&self, entity: &EntityPtr) -> Result<Entry> {
        if let Some(gid) = entity.borrow().gid() {
            let key = &entity.key();
            warn!(key = ?key, gid = ?gid, "unnecessary add-entity");
            return Ok(self
                .entry(&LookupBy::Key(key))?
                .ok_or(DomainError::EntityNotFound)?);
        }

        let world = self.world()?;
        let gid = match world {
            Some(world) => {
                let gid = identifiers::model::get_gid(&world)?;
                let gid = gid.unwrap_or(EntityGid::new(0));
                let gid = gid.next();
                let gid = identifiers::model::set_gid(&world, gid)?;
                gid
            }
            None => {
                // Otherwise we keep assigning 0 until the world gets created!
                assert_eq!(&entity.key(), &EntityKey::new(WORLD_KEY));
                EntityGid::new(0)
            }
        };

        self.state.add_entity(gid, entity)?;

        Ok(self
            .entry(&LookupBy::Key(&entity.key()))?
            .expect("Bug: Newly added entity has no Entry"))
    }

    fn obliterate(&self, entry: &Entry) -> Result<()> {
        self.state.obliterate(entry)
    }

    fn raise(&self, audience: Audience, event: Box<dyn DomainEvent>) -> Result<()> {
        let perform = Perform::Raised(Raised::new(audience.clone(), "".to_owned(), event.into()));

        self.perform(perform).map(|_| ())
    }

    fn schedule(&self, key: &str, when: When, message: &dyn ToJson) -> Result<()> {
        let key = key.to_owned();
        let message = message.to_json()?;
        let scheduling = Scheduling { key, when, message };
        let perform = Perform::Schedule(scheduling);

        self.perform(perform).map(|_| ())
    }

    fn hooks(&self) -> &ManagedHooks {
        &self.hooks
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.open.load(Ordering::Relaxed) {
            warn!("session-drop: open session!");
        } else {
            trace!("session-drop");
        }
    }
}

fn should_force_rollback() -> bool {
    env::var("FORCE_ROLLBACK").is_ok()
}

struct MakeSurroundings {
    finder: Arc<dyn Finder>,
    living: Entry,
}

impl TryInto<Surroundings> for MakeSurroundings {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<Surroundings, Self::Error> {
        let world = self.finder.find_world()?;
        let living = self.living.clone();
        let area: Entry = self.finder.find_location(&living)?;

        Ok(Surroundings::Living {
            world,
            living,
            area,
        })
    }
}

fn user_name_to_entry<R: EntryResolver>(resolve: &R, name: &str) -> Result<Entry, DomainError> {
    let world = resolve.world()?.expect("No world");
    let user_key = world
        .find_name_key(name)?
        .ok_or(DomainError::EntityNotFound)?;

    resolve
        .entry(&LookupBy::Key(&user_key))?
        .ok_or(DomainError::EntityNotFound)
}
