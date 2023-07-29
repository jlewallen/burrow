use anyhow::Result;
use chrono::{DateTime, Utc};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tracing::*;

use kernel::{
    any_entity_changes, get_my_session, set_my_session, ActiveSession, AnyChanges, Audience,
    DomainError, DomainEvent, EntityPtr, Entry, Evaluator, Original, Performer,
};

pub use rpc_proto::{EntityUpdate, IncomingMessage, LookupBy, Payload, Query};

pub use kernel::{Effect, Perform};

struct WorkingEntity {
    original: serde_json::Value,
    entry: Entry,
}

#[derive(Default)]
pub struct WorkingEntities {
    entities: HashMap<kernel::EntityKey, WorkingEntity>,
}

impl WorkingEntities {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn insert(&mut self, key: &kernel::EntityKey, value: (serde_json::Value, Entry)) {
        self.entities.insert(
            key.clone(),
            WorkingEntity {
                original: value.0,
                entry: value.1,
            },
        );
    }

    pub fn get(&self, key: &kernel::EntityKey) -> Result<Option<Entry>, DomainError> {
        Ok(self.entities.get(key).map(|r| r.entry.clone()))
    }

    pub fn flush(&self) -> Result<Vec<Query>> {
        Ok(self
            .entities
            .iter()
            .map(|(key, modified)| {
                if let Some(modified) = any_entity_changes(AnyChanges {
                    before: Some(Original::Json(&modified.original)),
                    after: modified.entry.entity().clone(),
                })? {
                    debug!("{:?} modified", key);
                    Ok(vec![Query::Update(EntityUpdate::new(
                        key.into(),
                        modified.after.into(),
                    ))])
                } else {
                    trace!("{:?} unmodified", key);
                    Ok(vec![])
                }
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}

struct RaisedEvent {
    audience: Audience,
    event: Box<dyn DomainEvent>,
}

pub struct ScheduledFuture {
    pub key: String,
    pub time: DateTime<Utc>,
    pub serialized: serde_json::Value,
}

#[derive(Default)]
pub struct AgentSession {
    entities: Rc<RefCell<WorkingEntities>>,
    raised: Rc<RefCell<Vec<RaisedEvent>>>,
    futures: Rc<RefCell<Vec<ScheduledFuture>>>,
}

impl AgentSession {
    pub fn new(entities: Rc<RefCell<WorkingEntities>>) -> Rc<Self> {
        Rc::new(Self {
            entities,
            raised: Default::default(),
            futures: Default::default(),
        })
    }
}

impl Performer for AgentSession {
    fn perform(&self, _perform: Perform) -> Result<Effect> {
        unimplemented!("AgentSession:perform")
    }
}

impl ActiveSession for AgentSession {
    fn entry(&self, lookup: &kernel::LookupBy) -> Result<Option<Entry>> {
        let entities = self.entities.borrow();
        match lookup {
            kernel::LookupBy::Key(key) => Ok(entities.get(*key)?),
            kernel::LookupBy::Gid(_) => unimplemented!("Entry by Gid"),
        }
    }

    fn find_item(
        &self,
        _surroundings: &kernel::Surroundings,
        _item: &kernel::Item,
    ) -> Result<Option<Entry>> {
        unimplemented!("AgentSession:find-item")
    }

    fn ensure_entity(
        &self,
        entity_ref: &kernel::EntityRef,
    ) -> Result<kernel::EntityRef, DomainError> {
        if entity_ref.has_entity() {
            Ok(entity_ref.clone())
        } else if let Some(entity) = &self.entry(&kernel::LookupBy::Key(entity_ref.key()))? {
            Ok(entity.entity().into())
        } else {
            Err(DomainError::EntityNotFound)
        }
    }

    fn add_entity(&self, entity: &kernel::EntityPtr) -> Result<Entry> {
        let key = entity.key();
        let json_value = entity.to_json_value()?;
        let entry = Entry::new(&key, entity.clone(), Rc::downgrade(&get_my_session()?));
        let mut entities = self.entities.borrow_mut();
        entities.insert(&key, (json_value, entry.clone()));
        Ok(entry)
    }

    fn obliterate(&self, _entity: &Entry) -> Result<()> {
        unimplemented!("AgentSession:obliterate")
    }

    fn new_key(&self) -> kernel::EntityKey {
        unimplemented!("AgentSession:new-key")
    }

    fn new_identity(&self) -> kernel::Identity {
        unimplemented!("AgentSession:new-identity")
    }

    fn raise(&self, audience: Audience, event: Box<dyn kernel::DomainEvent>) -> Result<()> {
        self.raised
            .borrow_mut()
            .push(RaisedEvent { audience, event });

        Ok(())
    }

    fn hooks(&self) -> &kernel::ManagedHooks {
        unimplemented!("AgentSession:hooks")
    }

    fn schedule(&self, key: &str, time: kernel::When, message: &dyn kernel::ToJson) -> Result<()> {
        let mut futures = self.futures.borrow_mut();
        futures.push(ScheduledFuture {
            key: key.to_owned(),
            time: time.to_utc_time()?,
            serialized: message.to_json()?,
        });
        Ok(())
    }
}

pub trait Agent: Evaluator {
    fn initialize(&mut self) -> Result<()>;
    fn have_surroundings(&mut self, surroundings: kernel::Surroundings) -> Result<()>;
    fn deliver(&mut self, incoming: kernel::Incoming) -> Result<()>;
}

pub struct AgentBridge<T>
where
    T: Agent,
{
    agent: T,
}

impl<T> AgentBridge<T>
where
    T: Agent,
{
    pub fn new(agent: T) -> Self {
        Self { agent }
    }

    pub fn initialize(&mut self) -> Result<Vec<Query>> {
        let session = Rc::new(AgentSession::default());
        set_my_session(Some(&(session.clone() as Rc<dyn ActiveSession>)))?;

        self.agent.initialize()?;

        self.flush_session(&session)
    }

    pub fn tick<TRecvFn>(&mut self, mut recv: TRecvFn) -> Result<Vec<Query>>
    where
        TRecvFn: FnMut() -> Option<Payload>,
    {
        let session = Rc::new(AgentSession::default());
        set_my_session(Some(&(session.clone() as Rc<dyn ActiveSession>)))?;

        let mut queries = Vec::new();

        while let Some(message) = recv() {
            debug!("(tick) {:?}", &message);

            match message {
                Payload::Resolved(resolved) => {
                    for resolved in resolved {
                        match resolved {
                            (LookupBy::Key(_key), Some(entity)) => {
                                let value = entity.try_into()?;
                                session.add_entity(&EntityPtr::from_value(value)?)?;
                            }
                            (LookupBy::Key(_key), None) => todo!(),
                            _ => {}
                        }
                    }
                }
                Payload::Surroundings(surroundings) => {
                    let with = WithEntities::new(Rc::clone(&session), surroundings);
                    self.agent.have_surroundings(with.try_into()?)?;
                }
                Payload::Deliver(incoming) => {
                    self.agent.deliver(incoming.into())?;
                }
                Payload::Evaluate(text) => {
                    let performer = AgentPerformer {};
                    for effect in self
                        .agent
                        .evaluate(&performer, kernel::Evaluable::Phrase(&text))?
                    {
                        queries.push(Query::Effect(effect.try_into()?))
                    }
                }
                _ => {}
            }
        }

        set_my_session(None)?;

        queries.extend(self.flush_session(&session)?);

        Ok(queries)
    }

    fn flush_session(&self, session: &AgentSession) -> Result<Vec<Query>> {
        let entities = session.entities.borrow();
        let mut queries = entities.flush()?;

        // TODO Can we use Into here for converting to the Query?

        let raised = session.raised.borrow();
        for raised in raised.iter() {
            queries.push(Query::Raise(
                raised.audience.clone().into(),
                raised.event.to_json()?.into(),
            ));
        }

        let futures = session.futures.borrow();
        for future in futures.iter() {
            queries.push(Query::Schedule(
                future.key.clone(),
                future.time.timestamp_millis(),
                future.serialized.clone().into(),
            ));
        }

        Ok(queries)
    }
}

pub struct WithEntities<T, S>
where
    S: ActiveSession,
{
    session: Rc<S>,
    value: T,
}

impl<'a, T, S> WithEntities<T, S>
where
    S: ActiveSession,
{
    pub fn new(session: Rc<S>, value: T) -> Self {
        Self { session, value }
    }

    fn get(
        &self,
        key: impl Into<kernel::EntityKey>,
    ) -> std::result::Result<kernel::Entry, DomainError> {
        self.session
            .entry(&kernel::LookupBy::Key(&key.into()))?
            .ok_or(DomainError::EntityNotFound)
    }
}

impl<S> TryInto<kernel::Surroundings> for WithEntities<rpc_proto::Surroundings, S>
where
    S: ActiveSession,
{
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<kernel::Surroundings, Self::Error> {
        match &self.value {
            rpc_proto::Surroundings::Living {
                world,
                living,
                area,
            } => Ok(kernel::Surroundings::Living {
                world: self.get(world)?,
                living: self.get(living)?,
                area: self.get(area)?,
            }),
        }
    }
}

struct AgentPerformer {}

impl Performer for AgentPerformer {
    fn perform(&self, perform: kernel::Perform) -> Result<Effect> {
        match perform {
            _ => todo!(),
        }
    }
}
