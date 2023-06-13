use std::collections::HashMap;

use anyhow::Context;
use kernel::{DomainError, Entry};
use wasm_sys::prelude::*;

#[derive(Default)]
struct WorkingEntities {
    entities: HashMap<EntityKey, Entry>,
}

impl WorkingEntities {
    pub fn insert(&mut self, key: &EntityKey, entry: Entry) {
        self.entities.insert(key.clone(), entry);
    }

    pub fn get(&self, key: impl Into<EntityKey>) -> Result<Entry, DomainError> {
        Ok(self
            .entities
            .get(&key.into())
            .ok_or(DomainError::EntityNotFound)?
            .clone())
    }
}

#[derive(Default)]
struct WasmExample {
    entities: WorkingEntities,
}

impl WasmExample {
    fn tick(&mut self) -> Result<()> {
        while let Some(message) = recv::<WasmMessage>() {
            debug!("(tick) {:?}", &message);

            match message {
                WasmMessage::Payload(Payload::Resolved(resolved)) => {
                    for resolved in resolved {
                        match resolved {
                            (LookupBy::Key(key), Some(entity)) => {
                                let value: serde_json::Value = entity.try_into()?;
                                let entry = Entry::new_from_json((&key).into(), value)
                                    .with_context(|| "Entry from JSON")?;
                                self.entities.insert(&key, entry);
                            }
                            (LookupBy::Key(_key), None) => todo!(),
                            _ => {}
                        }
                    }
                }
                WasmMessage::Payload(Payload::Surroundings(surroundings)) => {
                    self.have_surroundings(
                        WithEntities::new(&self.entities, surroundings).try_into()?,
                    )?;
                }
                WasmMessage::Query(_) => return Err(anyhow::anyhow!("Expecting payload only")),
                _ => {}
            }
        }

        Ok(())
    }

    fn have_surroundings(&mut self, surroundings: kernel::Surroundings) -> Result<()> {
        info!("surroundings {:?}", surroundings);

        Ok(())
    }
}

impl Agent for WasmExample {
    fn ready(&mut self) -> Result<()> {
        info!("ready");

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
    entities: &'a WorkingEntities,
    value: T,
}

impl<'a, T> WithEntities<'a, T> {
    pub fn new(entities: &'a WorkingEntities, value: T) -> Self {
        Self { entities, value }
    }
}

impl<'a> TryInto<kernel::Surroundings> for WithEntities<'a, Surroundings> {
    type Error = DomainError;

    fn try_into(self) -> std::result::Result<kernel::Surroundings, Self::Error> {
        match self.value {
            Surroundings::Living {
                world,
                living,
                area,
            } => Ok(kernel::Surroundings::Living {
                world: self.entities.get(world)?,
                living: self.entities.get(living)?,
                area: self.entities.get(area)?,
            }),
        }
    }
}
