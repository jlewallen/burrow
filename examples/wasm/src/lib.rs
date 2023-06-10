use wasm_sys::prelude::*;

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    let mut bridge = Box::new(AgentBridge::new());

    match bridge.tick() {
        Err(e) => {
            error!("{:?}", e);
            return;
        }
        Ok(_) => {}
    }

    agent_state(bridge);
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_tick(state: *mut std::ffi::c_void) {
    let state = state as *mut AgentBridge;
    match (*state).tick() {
        Err(e) => error!("{:?}", e),
        Ok(_) => {}
    }
}
