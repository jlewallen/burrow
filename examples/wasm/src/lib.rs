use wasm_sys::prelude::*;

struct WasmExample {}

impl WasmExample {
    fn tick(&mut self) -> Result<()> {
        while let Some(message) = recv::<WasmMessage>() {
            info!("(tick) {:?}", &message);
            match message {
                WasmMessage::Payload(Payload::Surroundings(surroundings)) => {
                    let keys = match &surroundings {
                        Surroundings::Living {
                            world,
                            living,
                            area,
                        } => vec![world, living, area],
                    };

                    let lookups = keys.into_iter().map(|k| LookupBy::Key(k.clone())).collect();
                    let lookup = Query::Lookup(DEFAULT_DEPTH, lookups);
                    send(&WasmMessage::Query(lookup))
                }
                WasmMessage::Payload(Payload::Resolved(resolved)) => {
                    info!("resolved {:?}", resolved);
                }
                WasmMessage::Query(_) => return Err(anyhow::anyhow!("Expecting payload only")),
                _ => {}
            }
        }

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
    let mut bridge = Box::new(WasmExample {});
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
