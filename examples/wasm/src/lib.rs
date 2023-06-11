use wasm_sys::prelude::*;

struct WasmExample {}

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
    let mut bridge = Box::new(AgentBridge::new(WasmExample {}));
    match bridge.tick() {
        Ok(_) => agent_state(bridge),
        Err(e) => error!("{:?}", e),
    };
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_tick(state: *mut std::ffi::c_void) {
    let bridge = state as *mut AgentBridge<WasmExample>;
    match (*bridge).tick() {
        Err(e) => error!("{:?}", e),
        Ok(_) => {}
    }
}
