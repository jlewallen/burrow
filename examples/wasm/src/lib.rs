use std::collections::HashMap;

use anyhow::Context;
use kernel::Entry;
use wasm_sys::prelude::*;

#[derive(Default)]
struct WasmExample {
    entities: HashMap<EntityKey, Entry>,
}

struct WithEntities<'a, T> {
    entities: &'a HashMap<EntityKey, Entry>,
    value: T,
}

impl<'a, T> WithEntities<'a, T> {
    pub fn new(entities: &'a HashMap<EntityKey, Entry>, value: T) -> Self {
        Self { entities, value }
    }
}

impl<'a> TryInto<kernel::Surroundings> for WithEntities<'a, Surroundings> {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<kernel::Surroundings, Self::Error> {
        match self.value {
            Surroundings::Living {
                world,
                living,
                area,
            } => Ok(kernel::Surroundings::Living {
                world: self.entities.get(&world.into()).expect("No world").clone(),
                living: self
                    .entities
                    .get(&living.into())
                    .expect("No living")
                    .clone(),
                area: self.entities.get(&area.into()).expect("No area").clone(),
            }),
        }
    }
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
                                self.entities.insert(
                                    key.clone(),
                                    Entry::new_from_json((&key).into(), value)
                                        .with_context(|| "Entry from JSON")?,
                                );
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
