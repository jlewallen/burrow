// use std::ffi::c_void;

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    println!("ok");
}
