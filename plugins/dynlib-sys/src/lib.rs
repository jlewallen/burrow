use anyhow::Result;
use bincode::{Decode, Encode};
use tracing::{dispatcher, error, Dispatch, Subscriber};

pub use agent_sys::*;

#[derive(Debug, Encode, Decode, Clone)]
pub enum DynMessage {
    Query(Query),
    Payload(Payload),
}

impl DynMessage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(bincode::decode_from_slice(bytes, bincode::config::legacy()).map(|(m, _)| m)?)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(bincode::encode_to_vec(self, bincode::config::legacy())?)
    }
}

// pub static RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub static CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct DynamicNext<'a> {
    pub n: Box<dyn FnOnce(Perform) -> Result<Effect> + 'a>,
}

#[derive(Copy, Clone)]
pub struct PluginDeclaration {
    // pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub initialize: unsafe extern "C" fn(&mut dyn DynamicHost),
    pub middleware: unsafe extern "C" fn(Perform, DynamicNext) -> Result<Effect>,
    pub tick: unsafe extern "C" fn(&mut dyn DynamicHost, state: *const std::ffi::c_void),
}

pub trait DynamicHost {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync>;

    fn send(&mut self, bytes: &[u8]) -> usize;

    fn recv(&mut self, bytes: &mut [u8]) -> usize;

    fn state(&mut self, state: *const std::ffi::c_void);
}

#[macro_export]
macro_rules! export_plugin {
    ($initialize:expr, $middleware:expr, $tick:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            core_version: $crate::CORE_VERSION,
            // rustc_version: $crate::RUSTC_VERSION,
            initialize: $initialize,
            middleware: $middleware,
            tick: $tick,
        };
    };
}

pub fn default_agent_initialize<A>(dh: &mut dyn DynamicHost)
where
    A: Agent + Default,
{
    if !dispatcher::has_been_set() {
        let subscriber = dh.tracing_subscriber();
        let dispatch = Dispatch::new(subscriber);
        match dispatcher::set_global_default(dispatch) {
            Err(e) => println!("Error configuring plugin tracing: {:?}", e),
            Ok(_) => {}
        };
    }

    let mut bridge = Box::new(AgentBridge::<A>::new(A::default()));

    let sending = match bridge.initialize() {
        Err(e) => {
            error!("Error initializing agent bridge: {:?}", e);
            // TODO Return an error message here.
            vec![]
        }
        Ok(sending) => {
            dh.state(Box::into_raw(bridge) as *const std::ffi::c_void);

            sending
        }
    };

    for m in sending {
        send(dh, DynMessage::Query(m));
    }
}

pub unsafe fn default_agent_tick<A>(dh: &mut dyn DynamicHost, state: *const std::ffi::c_void)
where
    A: Agent + Default,
{
    assert!(!state.is_null());

    let bridge = state as *mut AgentBridge<A>;
    let sending = match (*bridge).tick(|| match recv::<DynMessage>(dh) {
        Some(m) => match m {
            DynMessage::Payload(m) => Some(m),
            DynMessage::Query(_) => unimplemented!(),
        },
        None => None,
    }) {
        Ok(sending) => sending,
        Err(e) => {
            error!("Agent bridge error: {:?}", e);
            vec![]
        }
    };

    for m in sending {
        send(dh, DynMessage::Query(m));
    }
}

fn recv<T: Decode>(dh: &mut dyn DynamicHost) -> Option<T> {
    // For now this seems ok, but 'now' is basically the first test. So if
    // you end up here in the future I think you'll probably be better off
    // batching the protocol than you'll be worrying about memory
    // management.
    let mut buffer = [0; 65536];
    let len = dh.recv(&mut buffer);

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

fn send<T: Encode>(dh: &mut dyn DynamicHost, message: T) {
    let encoded: Vec<u8> = match bincode::encode_to_vec(&message, bincode::config::legacy()) {
        Ok(encoded) => encoded,
        Err(err) => {
            error!("Failed to serialize event: {}", err);
            return;
        }
    };

    dh.send(&encoded);

    std::mem::drop(encoded);
}

pub mod prelude {
    pub use anyhow::Result;

    pub use agent_sys::*;

    pub use super::{
        default_agent_initialize, default_agent_tick, export_plugin, Agent, DynMessage,
        DynamicHost, PluginDeclaration,
    };
}
