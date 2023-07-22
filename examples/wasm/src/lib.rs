use kernel::{Incoming, Surroundings};
use plugins_core::tools;
use wasm_sys::prelude::*;

#[derive(Default)]
struct WasmExample {}

impl Agent for WasmExample {
    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&mut self, surroundings: Surroundings) -> Result<()> {
        let (world, living, area) = surroundings.unpack();

        // info!("surroundings {:?}", surroundings);
        // let area = area.entity()?;
        // area.set_name("My world now!")?;

        trace!("world {:?}", world);
        trace!("living {:?}", living);
        trace!("area {:?}", area);

        let area_of = tools::area_of(&living)?;

        trace!("area-of: {:?}", area_of);

        Ok(())
    }

    fn deliver(&mut self, _incoming: Incoming) -> Result<()> {
        Ok(())
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    let mut bridge = Box::new(AgentBridge::<WasmExample>::new(WasmExample::default()));
    match bridge.tick(|| match recv::<WasmMessage>() {
        Some(m) => match m {
            WasmMessage::Payload(m) => Some(m),
            WasmMessage::Query(_) => unimplemented!(),
        },
        None => None,
    }) {
        Ok(_) => agent_state(bridge),
        Err(e) => error!("{:?}", e),
    };
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_tick(state: *mut std::ffi::c_void) {
    let bridge = state as *mut AgentBridge<WasmExample>;
    match (*bridge).tick(|| match recv::<WasmMessage>() {
        Some(m) => match m {
            WasmMessage::Payload(m) => Some(m),
            WasmMessage::Query(_) => unimplemented!(),
        },
        None => None,
    }) {
        Err(e) => error!("{:?}", e),
        Ok(_) => {}
    }
}
