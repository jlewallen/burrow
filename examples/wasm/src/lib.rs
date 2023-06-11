use wasm_sys::prelude::*;

struct WasmExample {
    bridge: AgentBridge,
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    let mut example = Box::new(WasmExample {
        bridge: AgentBridge::new(),
    });

    match example.bridge.tick() {
        Err(e) => {
            error!("{:?}", e);
            return;
        }
        Ok(_) => {}
    }

    agent_state(example);
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_tick(state: *mut std::ffi::c_void) {
    let state = state as *mut WasmExample;
    match (*state).bridge.tick() {
        Err(e) => error!("{:?}", e),
        Ok(_) => {}
    }
}
