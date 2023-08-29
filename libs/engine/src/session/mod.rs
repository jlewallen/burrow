use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
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
mod logs;
mod state;

use crate::identifiers;
use crate::notifications::Notifier;
use crate::prelude::DevNullNotifier;
use crate::sequences::Sequence;
use crate::storage::{Storage, StorageFactory};
use crate::users::model::HasUsernames;
use kernel::{here, prelude::*};
use state::State;

use self::logs::Logs;

#[derive(Clone)]
pub struct Dependencies {
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    finder: Arc<dyn Finder>,
    storage_factory: Arc<dyn StorageFactory>,
    registered_plugins: Arc<RegisteredPlugins>,
}

impl Dependencies {
    pub(crate) fn new(
        keys: &Arc<dyn Sequence<EntityKey>>,
        identities: &Arc<dyn Sequence<Identity>>,
        finder: &Arc<dyn Finder>,
        plugins: &Arc<RegisteredPlugins>,
        storage_factory: &Arc<dyn StorageFactory>,
    ) -> Dependencies {
        Dependencies {
            keys: Arc::clone(keys),
            identities: Arc::clone(identities),
            finder: Arc::clone(finder),
            storage_factory: Arc::clone(storage_factory),
            registered_plugins: Arc::clone(plugins),
        }
    }
}

pub struct Session {
    opened: Instant,
    open: AtomicBool,
    storage: Rc<dyn Storage>,
    storage_factory: Arc<dyn StorageFactory>,
    weak: Weak<Session>,
    finder: Arc<dyn Finder>,
    registered_plugins: Arc<RegisteredPlugins>,
    plugins: Arc<RefCell<SessionPlugins>>,
    middleware: Arc<RefCell<Vec<Rc<dyn Middleware>>>>,
    hooks: ManagedHooks,
    keys: Arc<dyn Sequence<EntityKey>>,
    identities: Arc<dyn Sequence<Identity>>,
    state: Rc<State>,
    captures: RefCell<Vec<Captured>>,
}

struct Captured {
    actor_key: EntityKey,
    time: DateTime<Utc>,
    desc: String,
    logs: Logs,
}

pub enum EvaluateAs<'a> {
    Name(&'a str),
    Key(&'a EntityKey),
}

impl Session {
    pub fn open(&self) -> Result<Rc<Self>> {
        Session::new(
            Dependencies::new(
                &self.keys,
                &self.identities,
                &self.finder,
                &self.registered_plugins,
                &self.storage_factory,
            ),
            vec![],
        )
    }

    pub fn new(deps: Dependencies, middleware: Vec<Rc<dyn Middleware>>) -> Result<Rc<Self>> {
        trace!("session-new");

        let opened = Instant::now();

        let storage = deps.storage_factory.create_storage()?;

        storage.begin()?;

        let plugins = deps.registered_plugins.create_plugins()?;
        let plugins = Arc::new(RefCell::new(plugins));

        let expand_surroundings: Rc<dyn Middleware> = Rc::new(ExpandSurroundingsMiddleware {
            finder: Arc::clone(&deps.finder),
        });
        let middleware: Vec<Rc<dyn Middleware>> = middleware
            .clone()
            .into_iter()
            .chain([expand_surroundings].into_iter())
            .collect();
        let middleware = Arc::new(RefCell::new(middleware));

        let hooks = ManagedHooks::default();

        let session = Rc::new_cyclic(move |weak: &Weak<Session>| Self {
            opened,
            storage,
            storage_factory: Arc::clone(&deps.storage_factory),
            weak: Weak::clone(weak),
            open: AtomicBool::new(true),
            finder: Arc::clone(&deps.finder),
            registered_plugins: deps.registered_plugins,
            plugins,
            middleware,
            hooks,
            keys: Arc::clone(&deps.keys),
            identities: Arc::clone(&deps.identities),
            state: Default::default(),
            captures: Default::default(),
        });

        session.initialize()?;

        Ok(session)
    }

    pub fn set_session(&self) -> Result<SetSession<Self>> {
        Ok(SetSession::new(self.weak.upgrade().unwrap()))
    }

    pub fn evaluate_and_perform(
        &self,
        user_name: &str,
        text: &str,
    ) -> Result<Option<Effect>, DomainError> {
        self.evaluate_and_perform_as(EvaluateAs::Name(user_name), text)
    }

    pub(super) fn captured(
        &self,
        actor: EntityPtr,
        action: Box<dyn Action>,
    ) -> Result<Effect, DomainError> {
        let actor_key = actor.key().clone();
        let action = PerformAction::Instance(action.into());
        let perform = Perform::Actor { actor, action };

        let desc = format!("{:?}", perform);
        logs::capture(
            || self.perform(perform.clone()),
            |logs| {
                let mut captures = self.captures.borrow_mut();
                captures.push(Captured {
                    actor_key,
                    time: Utc::now(),
                    desc,
                    logs,
                });

                Ok(())
            },
        )
    }

    fn save_logs(&self, forced: bool) -> Result<()> {
        let _span = span!(Level::INFO, "logs").entered();

        let captures = self.captures.borrow();
        for captured in captures.iter().filter(|c| forced || c.logs.is_important()) {
            let actor = get_my_session()?
                .entity(&LookupBy::Key(&captured.actor_key))?
                .unwrap();

            let mut actor = actor.borrow_mut();
            actor.replace_scope(&Diagnostics::new(
                captured.time,
                captured.desc.clone(),
                captured.logs.clone().into(),
            ))?;
        }

        Ok(())
    }

    fn parse_action(&self, text: &str) -> Result<Option<Box<dyn Action>>, EvaluationError> {
        let plugins = self.plugins.borrow();
        plugins.try_parse_action(text)
    }

    fn find_actor(&self, evaluate_as: EvaluateAs) -> Result<EntityPtr, DomainError> {
        let key = match evaluate_as {
            EvaluateAs::Name(user_name) => user_name_to_key(self, user_name)?,
            EvaluateAs::Key(key) => key.clone(),
        };

        Ok(self
            .recursive_entity(&LookupBy::Key(&key), USER_DEPTH)?
            .expect("No actor found with key"))
    }

    pub fn evaluate_and_perform_as(
        &self,
        evaluate_as: EvaluateAs,
        text: &str,
    ) -> Result<Option<Effect>, DomainError> {
        if !self.open.load(Ordering::Relaxed) {
            return Err(DomainError::SessionClosed.into());
        }

        match self.parse_action(text)? {
            Some(action) => {
                debug!("{:#?}", action.to_tagged_json()?.into_tagged());

                let session = self.set_session()?;
                let actor = session.find_actor(evaluate_as)?;

                match session.captured(actor, action) {
                    Ok(i) => Ok(Some(i)),
                    Err(original_err) => {
                        warn!("error: {:?}", original_err);
                        self.open.store(false, Ordering::Relaxed);
                        if let Err(_rollback_err) = self.storage.rollback(false) {
                            // TODO Include that this failed as part of the error.
                            panic!("TODO error rolling back");
                        }

                        let separate = self.open()?.set_session()?;
                        self.save_logs(true)?;
                        separate.close(&DevNullNotifier {})?;

                        Err(original_err)
                    }
                }
            }
            None => Ok(None),
        }
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

    pub fn schema(&self) -> SchemaCollection {
        let plugins = self.plugins.borrow();
        plugins.schema()
    }

    pub fn flush<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let _activated = self.set_session()?;

        self.save_changes(notifier)?;

        self.storage.begin()
    }

    pub fn close<T: Notifier>(&self, notifier: &T) -> Result<()> {
        let _activated = self.set_session()?;

        self.save_logs(self.state.write_expected())?;

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

    fn load_entity(&self, lookup: &LookupBy, depth: usize) -> Result<Option<EntityPtr>> {
        if let Some(e) = self.state.lookup_entity(lookup)? {
            return Ok(Some(e));
        }

        let _span = span!(Level::INFO, "entity", lookup = format!("{:?}", lookup)).entered();

        trace!("loading");
        if let Some(persisted) = self.storage.load(lookup)? {
            let added = self.state.add_persisted(persisted)?;
            if depth > 0 {
                for key in added.find_refs().into_iter() {
                    self.load_entity(&LookupBy::Key(&key), depth - 1)?;
                }
            }
            Ok(Some(added.into()))
        } else {
            Ok(None)
        }
    }
}

impl Performer for Session {
    fn perform(&self, perform: Perform) -> Result<Effect, DomainError> {
        let _span = span!(Level::DEBUG, "P").entered();

        info!("perform {:?}", perform);

        let target = self.state.clone();
        let request_fn =
            Box::new(move |value: Perform| -> Result<Effect> { Ok(target.perform(value)?) });

        let middleware = self.middleware.borrow();
        Ok(apply_middleware(&middleware, perform, request_fn)?)
    }
}

impl EntityPtrResolver for Session {
    fn recursive_entity(
        &self,
        lookup: &LookupBy,
        depth: usize,
    ) -> Result<Option<EntityPtr>, DomainError> {
        match self.load_entity(lookup, depth)? {
            Some(entity) => Ok(Some(entity)),
            None => Ok(None),
        }
    }
}

impl ActiveSession for Session {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        let plugins = self.plugins.borrow();
        plugins.try_deserialize_action(tagged)
    }

    fn new_key(&self) -> EntityKey {
        self.keys.following()
    }

    fn new_identity(&self) -> Identity {
        self.identities.following()
    }

    fn find_item(
        &self,
        surroundings: &Surroundings,
        item: &Item,
    ) -> Result<Option<EntityPtr>, DomainError> {
        let _loading_span = span!(Level::INFO, "finding", i = format!("{:?}", item)).entered();

        info!("finding");

        match item {
            Item::Gid(gid) => Ok(self.entity(&LookupBy::Gid(gid))?),
            Item::Key(key) => Ok(self.entity(&LookupBy::Key(key))?),
            _ => self.finder.find_item(surroundings, item),
        }
    }

    fn add_entity(&self, entity: Entity) -> Result<EntityPtr, DomainError> {
        if let Some(gid) = entity.gid() {
            let key = &entity.key();
            warn!(key = ?key, gid = ?gid, "unnecessary add-entity");
            return Ok(self
                .entity(&LookupBy::Key(key))?
                .ok_or(DomainError::EntityNotFound(here!().into()))?);
        }

        let world = self.world()?;
        let gid = match world {
            Some(world) => identifiers::model::fetch_add_one(&world)?,
            None => {
                // Otherwise we keep assigning 0 until the world gets created!
                assert_eq!(entity.key(), &EntityKey::new(WORLD_KEY));
                EntityGid::new(0)
            }
        };

        let key = entity.key().clone();

        self.state.add_entity(gid, entity)?;

        Ok(self
            .entity(&LookupBy::Key(&key))?
            .expect("Bug: Newly added entity has no EntityPtr"))
    }

    fn obliterate(&self, entity: &EntityPtr) -> Result<(), DomainError> {
        self.state.obliterate(entity)
    }

    fn raise(
        &self,
        actor: Option<EntityPtr>,
        audience: Audience,
        raising: Raising,
    ) -> Result<(), DomainError> {
        let perform = Perform::Raised(Raised::new(
            audience.clone(),
            "".to_owned(),
            actor.clone(),
            raising.into(),
        ));

        self.perform(perform).map(|_| ())
    }

    fn schedule(&self, destined: FutureAction) -> Result<(), DomainError> {
        let perform = Perform::Schedule(destined);

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

const USER_DEPTH: usize = 2;

fn user_name_to_key<R: EntityPtrResolver>(
    resolve: &R,
    name: &str,
) -> Result<EntityKey, DomainError> {
    let world = resolve.world()?.expect("No world");
    world
        .find_name_key(name)?
        .ok_or(DomainError::EntityNotFound(here!().into()))
}

struct MakeSurroundings {
    finder: Arc<dyn Finder>,
    actor: EntityPtr,
}

impl TryInto<Surroundings> for MakeSurroundings {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<Surroundings, Self::Error> {
        let world = self.finder.find_world()?;
        let actor = self.actor.clone();
        let area: EntityPtr = self
            .finder
            .find_area(&actor)
            .with_context(|| "find-location")?;

        Ok(Surroundings::Actor { world, actor, area })
    }
}

struct ExpandSurroundingsMiddleware {
    finder: Arc<dyn Finder>,
}

impl Middleware for ExpandSurroundingsMiddleware {
    fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
        match value {
            Perform::Actor { actor, action } => {
                let _span = span!(Level::DEBUG, "surround").entered();

                let surroundings = MakeSurroundings {
                    finder: self.finder.clone(),
                    actor: actor.clone(),
                }
                .try_into()
                .context(here!())?;

                info!("surroundings {:?}", &surroundings);

                next.handle(Perform::Surroundings {
                    surroundings,
                    action,
                })
            }
            _ => next.handle(value),
        }
    }
}
