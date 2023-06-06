/*
wai_bindgen_wasmer::import!("agent.wai");

wai_bindgen_rust::export!("agent.wai");

struct Agent {}

impl agent::Agent for Agent {
    fn hello(_who: Option<agent::Person>) -> String {
        println!("hello");
        "Hey!".to_string()
    }
}
*/

use std::ffi::c_void;

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    println!("ok");
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_update(_app_state: *mut c_void) {
    println!("update");
    // let app_state = app_state as *mut AppState;
    // update_app_state(&mut *app_state);
}
