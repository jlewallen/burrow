use std::{cell::RefCell, collections::HashMap, rc::Rc};

use kernel::{
    get_my_session, set_my_session, ActiveSession, DomainError, DomainEvent, EntityPtr, Entry,
    SessionRef,
};
use plugins_core::tools;
use wasm_sys::prelude::*;

#[derive(Default)]
struct WorkingEntities {
    entities: HashMap<kernel::EntityKey, Entry>,
}

impl WorkingEntities {
    pub fn insert(&mut self, key: &kernel::EntityKey, entry: Entry) {
        self.entities.insert(key.clone(), entry);
    }

    pub fn get(&self, key: &kernel::EntityKey) -> Result<Option<Entry>, DomainError> {
        Ok(self.entities.get(key).cloned())
    }
}

#[derive(Default)]
struct WasmExample {}

#[derive(Default)]
struct WasmSession {
    entities: RefCell<WorkingEntities>,
    raised: Rc<RefCell<Vec<Box<dyn DomainEvent>>>>,
}

impl ActiveSession for WasmSession {
    fn entry(&self, lookup: &kernel::LookupBy) -> Result<Option<Entry>> {
        let entities = self.entities.borrow();
        match lookup {
            kernel::LookupBy::Key(key) => Ok(entities.get(*key)?),
            kernel::LookupBy::Gid(_) => todo!(),
        }
    }

    fn find_item(
        &self,
        _surroundings: &kernel::Surroundings,
        _item: &kernel::Item,
    ) -> Result<Option<Entry>> {
        fail!("session:find-item")
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
        let entry = Entry::new(&key, entity.clone(), Rc::downgrade(&get_my_session()?));
        let mut entities = self.entities.borrow_mut();
        entities.insert(&key, entry.clone());
        Ok(entry)
    }

    fn obliterate(&self, _entity: &Entry) -> Result<()> {
        fail!("session:obliterate")
    }

    fn new_key(&self) -> kernel::EntityKey {
        fail!("session:new-key")
    }

    fn new_identity(&self) -> kernel::Identity {
        fail!("session:new-identity")
    }

    fn raise(&self, event: Box<dyn kernel::DomainEvent>) -> Result<()> {
        self.raised.borrow_mut().push(event);

        Ok(())
    }

    fn chain(&self, _perform: kernel::Perform) -> Result<Box<dyn kernel::Reply>> {
        fail!("session:chain")
    }

    fn hooks(&self) -> &kernel::ManagedHooks {
        fail!("session:hooks")
    }
}

impl WasmExample {
    fn tick(&mut self) -> Result<()> {
        let session: Rc<dyn ActiveSession> = Rc::new(WasmSession::default());

        set_my_session(Some(&session))?;

        while let Some(message) = recv::<WasmMessage>() {
            debug!("(tick) {:?}", &message);

            match message {
                WasmMessage::Payload(Payload::Resolved(resolved)) => {
                    for resolved in resolved {
                        match resolved {
                            (LookupBy::Key(_key), Some(entity)) => {
                                let value: serde_json::Value = entity.try_into()?;
                                session.add_entity(&EntityPtr::new_from_json(value)?)?;
                            }
                            (LookupBy::Key(_key), None) => todo!(),
                            _ => {}
                        }
                    }
                }
                WasmMessage::Payload(Payload::Surroundings(surroundings)) => {
                    self.have_surroundings(WithEntities::new(&session, surroundings).try_into()?)?;
                }
                WasmMessage::Query(_) => return Err(anyhow::anyhow!("Expecting payload only")),
                _ => {}
            }
        }

        Ok(())
    }

    fn have_surroundings(&mut self, surroundings: kernel::Surroundings) -> Result<()> {
        let (world, living, area) = surroundings.unpack();

        trace!("surroundings {:?}", surroundings);
        trace!("world {:?}", world);
        trace!("living {:?}", living);
        trace!("area {:?}", area);

        let area_of = tools::area_of(&living)?;

        trace!("area-of: {:?}", area_of);

        Ok(())
    }
}

impl WasmAgent for WasmExample {}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    let mut bridge = Box::new(WasmExample::default());
    match bridge.tick() {
        Ok(_) => agent_state(bridge),
        Err(e) => error!("{:?}", e),
    };
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_tick(state: *mut std::ffi::c_void) {
    let bridge = state as *mut WasmExample;
    match (*bridge).tick() {
        Err(e) => error!("{:?}", e),
        Ok(_) => {}
    }
}

struct WithEntities<'a, T> {
    session: &'a SessionRef,
    value: T,
}

impl<'a, T> WithEntities<'a, T> {
    pub fn new(session: &'a SessionRef, value: T) -> Self {
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

impl<'a> TryInto<kernel::Surroundings> for WithEntities<'a, Surroundings> {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<kernel::Surroundings, Self::Error> {
        match &self.value {
            Surroundings::Living {
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
