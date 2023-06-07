mod macros;

mod ffi {
    #![allow(dead_code)]

    #[link(wasm_import_module = "burrow")]
    extern "C" {
        pub fn console_info(msg: *const u8, len: usize);
        pub fn console_warn(msg: *const u8, len: usize);
        pub fn console_error(msg: *const u8, len: usize);

        pub fn agent_send(event: *const u8, len: usize);
        pub fn agent_recv(event: *const u8, len: usize) -> usize;
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    // println!("ok");
    info!("info!");
    // warn!("warn!");
    // error!("error!");
}
