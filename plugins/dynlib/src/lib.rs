use anyhow::Result;
use bincode::{Decode, Encode};
use libloading::Library;
use plugins_rpc::{Querying, SessionServices};
use plugins_rpc_proto::{Payload, Query, Sender};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};
use tracing::{dispatcher::get_default, info, span, trace, warn, Level, Subscriber};

use kernel::{EvaluationResult, ManagedHooks, ParsesActions, Plugin, PluginFactory};
use plugins_core::library::plugin::*;

#[derive(Default)]
pub struct DynamicPluginFactory {}

impl PluginFactory for DynamicPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::<DynamicPlugin>::default())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl DynamicHost for LoadedLibrary {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync> {
        Box::new(PluginSubscriber::new())
    }

    fn send(&mut self, bytes: &[u8]) -> usize {
        self.outbox.push(bytes.into());

        bytes.len()
    }

    fn recv(&mut self, bytes: &mut [u8]) -> usize {
        let Some(sending) = self.inbox.pop_front() else { return 0 };

        if sending.len() > bytes.len() {
            return sending.len();
        }

        bytes[..sending.len()].copy_from_slice(&sending);

        sending.len()
    }
}

struct LoadedLibrary {
    library: Rc<Library>,
    inbox: VecDeque<Arc<[u8]>>,
    outbox: Vec<Box<[u8]>>,
    _state: Option<i32>,
}

#[derive(Debug, Encode, Decode)]
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

impl LoadedLibrary {
    fn initialize(&mut self) -> Result<()> {
        unsafe {
            info!("initializing");

            let sym = self
                .library
                .get::<*mut PluginDeclaration>(b"plugin_declaration\0")?;
            let decl = sym.read();

            (decl.initialize)(self);
        }

        self.tick()?;

        Ok(())
    }

    // TODO Dupe
    fn process_queries(&mut self, messages: Vec<Box<[u8]>>) -> Result<()> {
        let messages: Vec<DynMessage> = messages
            .into_iter()
            .map(|b| Ok(DynMessage::from_bytes(&b)?))
            .collect::<Result<Vec<_>>>()?;

        let services = SessionServices::new_for_my_session()?;
        for message in messages.into_iter() {
            debug!("(server) {:?}", message);

            match message {
                DynMessage::Query(q) => {
                    let mut sender: Sender<Payload> = Default::default();
                    let querying = Querying::new();
                    querying.service(&q, &mut sender, &services)?;

                    for payload in sender.into_iter() {
                        trace!("(to-agent) {:?}", &payload);
                        self.send(&DynMessage::Payload(payload).to_bytes()?);
                    }
                }
                _ => unimplemented!("Wasm server received {:?}", message),
            }
        }

        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        unsafe {
            let sym = self
                .library
                .get::<*mut PluginDeclaration>(b"plugin_declaration\0")?;
            let decl = sym.read();

            while !self.inbox.is_empty() {
                (decl.tick)(self);

                let outbox = std::mem::take(&mut self.outbox);

                self.process_queries(outbox)?;
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct DynamicPlugin {
    libraries: RefCell<Vec<LoadedLibrary>>,
}

impl DynamicPlugin {
    fn open_dynamic(&mut self) -> Result<()> {
        let filename = libloading::library_filename("plugin_example_shared");
        let path = format!("target/debug/{}", filename.to_string_lossy());
        if std::fs::metadata(&path).is_ok() {
            let mut libraries = self.libraries.borrow_mut();
            libraries.push(self.load(&path)?);
        }

        Ok(())
    }

    fn load(&self, path: &str) -> Result<LoadedLibrary> {
        unsafe {
            let _span = span!(Level::INFO, "regdyn", lib = path).entered();

            info!("loading");

            let library = Rc::new(libloading::Library::new(path)?);

            Ok(LoadedLibrary {
                library,
                inbox: Default::default(),
                outbox: Default::default(),
                _state: None,
            })
        }
    }

    fn tick(&self) -> Result<()> {
        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            library.tick()?;
        }

        Ok(())
    }
}

// pub static RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub static CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Copy, Clone)]
pub struct PluginDeclaration {
    // pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub initialize: unsafe extern "C" fn(&mut dyn DynamicHost),
    pub tick: unsafe extern "C" fn(&mut dyn DynamicHost),
}

pub trait DynamicHost {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync>;

    fn send(&mut self, bytes: &[u8]) -> usize;

    fn recv(&mut self, bytes: &mut [u8]) -> usize;
}

#[macro_export]
macro_rules! export_plugin {
    ($initialize:expr, $tick:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static plugin_declaration: $crate::PluginDeclaration = $crate::PluginDeclaration {
            core_version: $crate::CORE_VERSION,
            // rustc_version: $crate::RUSTC_VERSION,
            initialize: $initialize,
            tick: $tick,
        };
    };
}

const KEY: &'static str = "dynamic";

impl Plugin for DynamicPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        KEY
    }

    fn key(&self) -> &'static str {
        KEY
    }

    fn initialize(&mut self) -> Result<()> {
        match self.open_dynamic() {
            Ok(v) => trace!("{:?}", v),
            Err(e) => warn!("Error: {:?}", e),
        }

        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            library.initialize()?;
        }

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, _surroundings: &Surroundings) -> Result<()> {
        self.tick()?;

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for DynamicPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

struct PluginSubscriber {}

impl PluginSubscriber {
    fn new() -> Self {
        Self {}
    }
}

impl Subscriber for PluginSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        get_default(|d| d.enabled(metadata))
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        get_default(|d| d.new_span(span))
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        get_default(|d| d.record(span, values))
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        get_default(|d| d.record_follows_from(span, follows))
    }

    fn event(&self, event: &tracing::Event<'_>) {
        get_default(|d| d.event(event))
    }

    fn enter(&self, span: &span::Id) {
        get_default(|d| d.enter(span))
    }

    fn exit(&self, span: &span::Id) {
        get_default(|d| d.exit(span))
    }
}

pub mod model {
    use plugins_core::library::model::*;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DynamicReply {}

    impl Reply for DynamicReply {}

    impl ToJson for DynamicReply {
        fn to_json(&self) -> Result<Value, serde_json::Error> {
            serde_json::to_value(self)
        }
    }
}

pub mod actions {
    // use crate::{library::actions::*, looking::actions::LookAction};
}

pub mod parser {
    // use crate::library::parser::*;
}

#[cfg(test)]
mod tests {
    use plugins_core::library::tests::*;
    // use super::parser::*;
    use super::*;

    #[test]
    fn it_dynamic() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (_session, _surroundings) = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .build()?;

        Ok(())
    }
}
