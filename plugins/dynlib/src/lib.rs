use anyhow::Result;
use libloading::Library;
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::Arc};
use tracing::{dispatcher::get_default, info, span, trace, warn, Level, Subscriber};

use dynlib_sys::prelude::*;
use kernel::{ManagedHooks, Plugin, PluginFactory};
use plugins_core::library::plugin::*;
use plugins_rpc::{have_surroundings, Querying, SessionServices};

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

#[derive(Default)]
pub struct Outbox {
    messages: Vec<Box<[u8]>>,
}

impl Outbox {
    pub fn send(&mut self, bytes: &[u8]) -> usize {
        self.messages.push(bytes.into());

        bytes.len()
    }
}

#[derive(Default)]
pub struct Inbox {
    messages: VecDeque<Arc<[u8]>>,
}

impl Inbox {
    pub fn recv(&mut self, bytes: &mut [u8]) -> usize {
        let Some(sending) = self.messages.pop_front() else { return 0 };

        if sending.len() > bytes.len() {
            return sending.len();
        }

        bytes[..sending.len()].copy_from_slice(&sending);

        sending.len()
    }
}

impl DynamicHost for LoadedLibrary {
    fn tracing_subscriber(&self) -> Box<dyn Subscriber + Send + Sync> {
        Box::new(PluginSubscriber::new())
    }

    fn send(&mut self, bytes: &[u8]) -> usize {
        self.outbox.send(bytes)
    }

    fn recv(&mut self, bytes: &mut [u8]) -> usize {
        self.inbox.recv(bytes)
    }

    fn state(&mut self, state: *const std::ffi::c_void) {
        self.state = Some(state);
    }
}

struct LoadedLibrary {
    prefix: String,
    library: Rc<Library>,
    inbox: Inbox,
    outbox: Outbox,
    state: Option<*const std::ffi::c_void>,
}

impl LoadedLibrary {
    fn initialize(&mut self) -> Result<()> {
        unsafe {
            trace!("initializing");

            let sym = self
                .library
                .get::<*mut PluginDeclaration>(b"plugin_declaration\0")?;
            let decl = sym.read();

            (decl.initialize)(self);
        }

        self.tick()?;

        let outbox = std::mem::take(&mut self.outbox.messages);

        self.process_queries(outbox)?;

        Ok(())
    }

    fn process_queries(&mut self, messages: Vec<Box<[u8]>>) -> Result<()> {
        let services = SessionServices::new_for_my_session(Some(&self.prefix))?;
        let messages: Vec<Query> = messages
            .into_iter()
            .map(|b| Ok(DynMessage::from_bytes(&b)?))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(|m| match m {
                DynMessage::Query(query) => query,
                DynMessage::Payload(_) => unimplemented!(),
            })
            .collect();

        let querying = Querying::new();
        for payload in querying.process(messages, &services)? {
            self.send(&DynMessage::Payload(payload).to_bytes()?);
        }

        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        unsafe {
            let sym = self
                .library
                .get::<*mut PluginDeclaration>(b"plugin_declaration\0")?;
            let decl = sym.read();

            while !self.inbox.messages.is_empty() {
                let state = self.state.unwrap_or(std::ptr::null());

                (decl.tick)(self, state);

                let outbox = std::mem::take(&mut self.outbox.messages);

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
        match self.load("plugin_example_shared") {
            Ok(library) => {
                let mut libraries = self.libraries.borrow_mut();
                libraries.push(library);
            }
            Err(e) => warn!("failed to load dynamic library: {:?}", e),
        }

        Ok(())
    }

    fn load(&self, name: &str) -> Result<LoadedLibrary> {
        unsafe {
            let _span = span!(Level::INFO, "regdyn", lib = name).entered();

            let filename = libloading::library_filename(name);
            let path = format!("target/debug/{}", filename.to_string_lossy());

            info!(%path, "loading");

            let library = Rc::new(libloading::Library::new(path)?);

            Ok(LoadedLibrary {
                prefix: name.to_owned(),
                library,
                inbox: Default::default(),
                outbox: Default::default(),
                state: None,
            })
        }
    }

    fn push_messages_with<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(&LoadedLibrary) -> Option<Vec<DynMessage>>,
    {
        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            if let Some(messages) = f(library) {
                for message in messages.iter() {
                    library.inbox.messages.push_back(message.to_bytes()?.into());
                }
            }
        }

        Ok(())
    }

    fn push_messages_to_all(&self, pushing: &Vec<DynMessage>) -> Result<()> {
        self.push_messages_with(move |_ll| Some(pushing.to_vec()))
    }

    fn push_messages_to_prefix<F>(&self, prefix: &str, mut f: F) -> Result<()>
    where
        F: FnMut(&LoadedLibrary) -> Vec<DynMessage>,
    {
        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            if prefix.starts_with(&library.prefix) {
                debug!(prefix = library.prefix, "deliver-library");
                for message in f(library).iter() {
                    library.inbox.messages.push_back(message.to_bytes()?.into());
                }
            }
        }

        Ok(())
    }

    fn tick(&self) -> Result<()> {
        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            library.tick()?;
        }

        Ok(())
    }
}

impl Plugin for DynamicPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "dynlib"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        match self.open_dynamic() {
            Ok(v) => trace!("{:?}", v),
            Err(e) => warn!("Error: {:?}", e),
        }

        // let services = SessionServices::new_for_my_session(None)?;
        let mut libraries = self.libraries.borrow_mut();
        for library in libraries.iter_mut() {
            library.initialize()?;
        }

        Ok(())
    }

    fn register_hooks(&self, _hooks: &ManagedHooks) -> Result<()> {
        Ok(())
    }

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        let services = SessionServices::new_for_my_session(None)?;
        let messages = have_surroundings(surroundings, &services)?
            .into_iter()
            .map(|m| DynMessage::Payload(m))
            .collect::<Vec<_>>();

        self.push_messages_to_all(&messages)?;

        self.tick()?;

        Ok(())
    }

    fn deliver(&self, incoming: &Incoming) -> Result<()> {
        self.push_messages_to_prefix(&incoming.key, |_ll| {
            vec![DynMessage::Payload(Payload::Deliver(
                IncomingMessage::from(incoming),
            ))]
        })?;

        self.tick()?;

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl Evaluator for DynamicPlugin {
    fn evaluate(&self, _perform: &dyn Performer, consider: Evaluation) -> Result<Vec<Effect>> {
        match consider {
            Evaluation::Text(text) => {
                let services = SessionServices::new_for_my_session(None)?;

                let messages = vec![DynMessage::Payload(Payload::TryParse(text.to_owned()))];

                self.push_messages_to_all(&messages)?;

                self.tick()?;

                if let Some(produced) = services.take_produced()? {
                    Ok(produced)
                } else {
                    Ok(Vec::new())
                }
            }
        }
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
