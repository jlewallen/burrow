use anyhow::Result;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tracing::*;

use kernel::prelude::*;

use burrow_bon::prelude::{AnyChanges, Original};

pub use rpc_proto::{EntityUpdate, IncomingMessage, LookupBy, Payload, Query};

pub use kernel::prelude::{Effect, Perform};

struct WorkingEntity {
    original: JsonValue,
    entity: EntityPtr,
}

#[derive(Default)]
pub struct WorkingEntities {
    entities: HashMap<kernel::prelude::EntityKey, WorkingEntity>,
}

impl WorkingEntities {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn insert(&mut self, key: &kernel::prelude::EntityKey, value: (JsonValue, EntityPtr)) {
        self.entities.insert(
            key.clone(),
            WorkingEntity {
                original: value.0,
                entity: value.1,
            },
        );
    }

    pub fn get(&self, key: &kernel::prelude::EntityKey) -> Result<Option<EntityPtr>, DomainError> {
        Ok(self.entities.get(key).map(|r| r.entity.clone()))
    }

    pub fn flush(&self) -> Result<Vec<Query>> {
        Ok(self
            .entities
            .iter()
            .map(|(key, modified)| {
                if let Some(modified) = any_entity_changes(AnyChanges {
                    before: Some(Original::Json(&modified.original)),
                    after: modified.entity.clone(),
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
    _actor: Option<EntityPtr>,
    audience: Audience,
    raising: Raising,
}

#[derive(Default)]
pub struct AgentSession {
    entities: Rc<RefCell<WorkingEntities>>,
    raised: Rc<RefCell<Vec<RaisedEvent>>>,
    futures: Rc<RefCell<Vec<FutureAction>>>,
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
    fn perform(&self, _perform: Perform) -> Result<Effect, DomainError> {
        unimplemented!("AgentSession:perform")
    }
}

impl EntityPtrResolver for AgentSession {
    fn recursive_entity(
        &self,
        lookup: &kernel::prelude::LookupBy,
        _depth: usize,
    ) -> Result<Option<EntityPtr>, DomainError> {
        let entities = self.entities.borrow();
        match lookup {
            kernel::prelude::LookupBy::Key(key) => Ok(entities.get(key)?),
            kernel::prelude::LookupBy::Gid(_) => unimplemented!("EntityPtr by Gid"),
        }
    }
}

impl ActiveSession for AgentSession {
    fn try_deserialize_action(
        &self,
        _tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        unimplemented!("AgentSession:try-deserialize-action")
    }

    fn find_item(
        &self,
        _surroundings: &kernel::prelude::Surroundings,
        _item: &kernel::prelude::Item,
    ) -> Result<Option<Found>, DomainError> {
        unimplemented!("AgentSession:find-item")
    }

    fn add_entity(&self, entity: kernel::prelude::Entity) -> Result<EntityPtr, DomainError> {
        let key = entity.key().clone();
        let json_value = entity.to_json_value()?;
        let entity = EntityPtr::new_from_entity(entity);
        let mut entities = self.entities.borrow_mut();
        entities.insert(&key, (json_value, entity.clone()));
        Ok(entity)
    }

    fn obliterate(&self, _entity: &EntityPtr) -> Result<(), DomainError> {
        unimplemented!("AgentSession:obliterate")
    }

    fn new_key(&self) -> kernel::prelude::EntityKey {
        unimplemented!("AgentSession:new-key")
    }

    fn new_identity(&self) -> kernel::prelude::Identity {
        unimplemented!("AgentSession:new-identity")
    }

    fn raise(
        &self,
        actor: Option<EntityPtr>,
        audience: Audience,
        raising: Raising,
    ) -> Result<(), DomainError> {
        self.raised.borrow_mut().push(RaisedEvent {
            _actor: actor,
            audience,
            raising,
        });

        Ok(())
    }

    fn schedule(&self, destined: FutureAction) -> Result<(), DomainError> {
        let mut futures = self.futures.borrow_mut();
        futures.push(destined);

        Ok(())
    }
}

pub trait Agent {
    fn initialize(&mut self) -> Result<()>;
    fn have_surroundings(&mut self, surroundings: kernel::prelude::Surroundings) -> Result<()>;
    fn deliver(&mut self, incoming: kernel::prelude::Incoming) -> Result<()>;
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
        let _set = SetSession::new(session.clone());

        self.agent.initialize()?;

        self.flush_session(&session)
    }

    pub fn tick<TRecvFn>(&mut self, mut recv: TRecvFn) -> Result<Vec<Query>>
    where
        TRecvFn: FnMut() -> Option<Payload>,
    {
        let session = Rc::new(AgentSession::default());
        let _set = SetSession::new(session.clone());

        let mut queries = Vec::new();

        while let Some(message) = recv() {
            debug!("(tick) {:?}", &message);

            match message {
                Payload::Resolved(resolved) => {
                    for resolved in resolved {
                        match resolved {
                            (LookupBy::Key(_key), Some(entity)) => {
                                let json: JsonValue = entity.into();
                                let value: Entity = json.try_into()?;
                                session.add_entity(value)?;
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
                Payload::Evaluate(_text) => {
                    todo!()
                }
                _ => {}
            }
        }

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
                match &raised.raising {
                    Raising::TaggedJson(tagged) => tagged.clone().into(),
                },
            ));
        }

        let futures = session.futures.borrow();
        for future in futures.iter() {
            queries.push(Query::Schedule(
                future.key.clone(),
                future.entity.clone().into(),
                future.schedule.clone().into(),
                future.action.clone().into(),
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

impl<T, S> WithEntities<T, S>
where
    S: ActiveSession,
{
    pub fn new(session: Rc<S>, value: T) -> Self {
        Self { session, value }
    }

    fn get(
        &self,
        key: impl Into<kernel::prelude::EntityKey>,
    ) -> std::result::Result<kernel::prelude::EntityPtr, DomainError> {
        self.session
            .entity(&kernel::prelude::LookupBy::Key(&key.into()))?
            .ok_or(DomainError::EntityNotFound(here!().into()))
    }
}

impl<S> TryInto<kernel::prelude::Surroundings> for WithEntities<rpc_proto::Surroundings, S>
where
    S: ActiveSession,
{
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<kernel::prelude::Surroundings, Self::Error> {
        match &self.value {
            rpc_proto::Surroundings::Actor { world, actor, area } => {
                Ok(kernel::prelude::Surroundings::Actor {
                    world: self.get(world)?,
                    actor: self.get(actor)?,
                    area: self.get(area)?,
                })
            }
        }
    }
}

struct AgentPerformer {}

impl Performer for AgentPerformer {
    fn perform(&self, _perform: kernel::prelude::Perform) -> Result<Effect, DomainError> {
        todo!()
    }
}
