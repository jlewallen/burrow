use bincode::{Decode, Encode};

use wasm_sys::{info, ipc};

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn agent_initialize() {
    match ipc::recv::<Message>() {
        Some(m) => {
            info!("message: {:?}", m);
            ipc::send(&Message::Pong);
        }
        None => info!("empty"),
    }
}

#[derive(Debug, Encode, Decode)]
pub enum Message {
    Ping(String),
    Pong,
}
