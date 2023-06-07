pub mod macros;

pub mod ffi {
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

pub mod ipc {
    use bincode::{Decode, Encode};

    use crate::error;

    pub fn send<T: Encode>(message: &T) {
        let encoded: Vec<u8> = match bincode::encode_to_vec(&message, bincode::config::legacy()) {
            Ok(encoded) => encoded,
            Err(err) => {
                error!("Failed to serialize event: {}", err);
                return;
            }
        };

        unsafe {
            crate::ffi::agent_send(encoded.as_ptr(), encoded.len());
        }

        std::mem::drop(encoded);
    }

    pub fn recv<T: Decode>() -> Option<T> {
        let mut buffer = [0; 8192];
        let len = unsafe { crate::ffi::agent_recv(buffer.as_mut_ptr(), buffer.len()) };

        if len == 0 {
            return None;
        }

        if len > buffer.len() {
            error!("Serialized message is larger than buffer");
            return None;
        }

        match bincode::decode_from_slice(&buffer[..len], bincode::config::legacy()) {
            Ok((message, _)) => Some(message),
            Err(err) => {
                error!("Failed to deserialize message from host: {}", err);
                None
            }
        }
    }
}
