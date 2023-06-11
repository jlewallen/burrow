pub mod macros;

pub mod ffi {
    #![allow(dead_code)]

    use std::ffi::c_void;

    #[link(wasm_import_module = "burrow")]
    extern "C" {
        pub fn console_info(msg: *const u8, len: usize);
        pub fn console_warn(msg: *const u8, len: usize);
        pub fn console_error(msg: *const u8, len: usize);

        pub fn agent_store(app: *const c_void);
        pub fn agent_send(event: *const u8, len: usize);
        pub fn agent_recv(event: *const u8, len: usize) -> usize;
    }
}

pub mod ipc {
    use anyhow::Result;
    use bincode::{Decode, Encode};
    use tracing::trace;

    use crate::{error, info};

    use plugins_rpc_proto::{AgentProtocol, DefaultResponses, Payload, Query, Sender};

    pub struct AgentBridge {
        agent: AgentProtocol<DefaultResponses>,
    }

    impl AgentBridge {
        pub fn new() -> Self {
            Self {
                agent: AgentProtocol::new(),
            }
        }

        pub fn tick(&mut self) -> anyhow::Result<()> {
            while let Some(message) = recv::<WasmMessage>() {
                let mut replies: Sender<Query> = Default::default();
                info!("{:?}", message);

                match message {
                    WasmMessage::Payload(payload) => self.agent.apply(&payload, &mut replies)?,
                    _ => unimplemented!(),
                }

                for query in replies.into_iter() {
                    trace!("(to-server): {:?}", &query);
                    send(&WasmMessage::Query(query))
                }
            }

            Ok(())
        }
    }

    #[derive(Debug, Encode, Decode)]
    pub enum WasmMessage {
        Query(Query),
        Payload(Payload),
    }

    impl WasmMessage {
        pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
            Ok(bincode::decode_from_slice(bytes, bincode::config::legacy()).map(|(m, _)| m)?)
        }

        pub fn to_bytes(&self) -> Result<Vec<u8>> {
            Ok(bincode::encode_to_vec(self, bincode::config::legacy())?)
        }
    }

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
        // For now this seems ok, but 'now' is basically the first test. So if
        // you end up here in the future I think you'll probably be better off
        // batching the protocol than you'll be worrying about memory
        // management.
        let mut buffer = [0; 65536];
        let len = unsafe { crate::ffi::agent_recv(buffer.as_mut_ptr(), buffer.len()) };

        if len == 0 {
            return None;
        }

        if len > buffer.len() {
            error!(
                "Serialized message is larger than buffer (by {} bytes)",
                len - buffer.len()
            );
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

pub mod prelude {
    use std::ffi::c_void;

    pub use crate::ffi;
    pub use crate::ipc::{recv, send, AgentBridge};
    pub use crate::{error, info, warn};

    pub unsafe fn agent_state<T>(state: Box<T>) {
        crate::ffi::agent_store(Box::into_raw(state) as *const c_void);
    }
}
