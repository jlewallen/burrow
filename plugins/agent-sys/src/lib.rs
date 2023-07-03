use anyhow::Result;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tracing::*;

use kernel::{
    compare::{any_entity_changes, AnyChanges, Original},
    get_my_session, set_my_session, ActiveSession, DomainError, DomainEvent, EntityPtr, Entry,
};
use plugins_rpc_proto::{EntityUpdate, LookupBy, Payload, Query};

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
                    entity: modified.entry.entity()?,
                    original: Some(Original::Json(&modified.original)),
                })? {
                    debug!("{:?} modified", key);
                    Ok(vec![Query::Update(EntityUpdate::new(
                        key.into(),
                        modified.entity.into(),
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

pub struct AgentSession {
    entities: Rc<RefCell<WorkingEntities>>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
}

impl AgentSession {
    pub fn new(entities: Rc<RefCell<WorkingEntities>>) -> Rc<Self> {
        Rc::new(Self {
            entities,
            raised: Default::default(),
        })
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
            Ok(entity.entity()?.into())
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

    fn raise(&self, event: Box<dyn kernel::DomainEvent>) -> Result<()> {
        self.raised.borrow_mut().push(event);

        Ok(())
    }

    fn chain(&self, _perform: kernel::Perform) -> Result<Box<dyn kernel::Reply>> {
        unimplemented!("AgentSession:chain")
    }

    fn hooks(&self) -> &kernel::ManagedHooks {
        unimplemented!("AgentSession:hooks")
    }
}

pub trait Agent {
    fn have_surroundings(&mut self, surroundings: kernel::Surroundings) -> Result<()>;
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

    pub fn tick<TRecvFn>(&mut self, mut recv: TRecvFn) -> Result<Vec<Query>>
    where
        TRecvFn: FnMut() -> Option<Payload>,
    {
        let entities = WorkingEntities::new();
        let session = AgentSession::new(entities.clone());

        set_my_session(Some(&(session.clone() as Rc<dyn ActiveSession>)))?;

        while let Some(message) = recv() {
            debug!("(tick) {:?}", &message);

            match message {
                Payload::Resolved(resolved) => {
                    for resolved in resolved {
                        match resolved {
                            (LookupBy::Key(_key), Some(entity)) => {
                                let value = entity.try_into()?;
                                session.add_entity(&EntityPtr::new_from_json(value)?)?;
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
                _ => {}
            }
        }

        set_my_session(None)?;

        let entities = entities.borrow();
        let mut queries = entities.flush()?;

        let raised = session.raised.borrow();
        for raised in raised.iter() {
            queries.push(Query::Raise(raised.to_json_value()?.into()));
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

impl<S> TryInto<kernel::Surroundings> for WithEntities<plugins_rpc_proto::Surroundings, S>
where
    S: ActiveSession,
{
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<kernel::Surroundings, Self::Error> {
        match &self.value {
            plugins_rpc_proto::Surroundings::Living {
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
