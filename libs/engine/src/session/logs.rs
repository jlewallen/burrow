use std::sync::{Arc, RwLock};
use tracing::dispatcher::WeakDispatch;
use tracing::*;

pub fn capture<V, E, F, H>(f: F, h: H) -> Result<V, E>
where
    F: FnOnce() -> Result<V, E>,
    H: FnOnce(Vec<serde_json::Value>) -> Result<(), E>,
{
    let weak = tracing::dispatcher::get_default(move |d| d.downgrade());
    let mut capturing = SessionSubscriber::new(weak);
    let dispatch = tracing::dispatcher::Dispatch::new(capturing.clone());
    let rv = tracing::dispatcher::with_default(&dispatch, f);

    h(capturing.take())?;

    rv
}

#[derive(Clone)]
struct SessionSubscriber {
    target: WeakDispatch,
    entries: Arc<RwLock<Option<Vec<serde_json::Value>>>>,
}

impl SessionSubscriber {
    fn new(target: WeakDispatch) -> Self {
        Self {
            target,
            entries: Arc::new(RwLock::new(Some(Vec::new()))),
        }
    }

    fn take(&mut self) -> Vec<serde_json::Value> {
        self.entries.write().unwrap().take().unwrap()
    }

    fn with<T, V>(&self, f: T) -> V
    where
        T: FnOnce(&Dispatch) -> V,
    {
        f(self.target.upgrade().as_ref().unwrap())
    }
}

impl Subscriber for SessionSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        self.with(|d| d.enabled(metadata))
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        self.with(|d| d.new_span(span))
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        self.with(|d| d.record(span, values))
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        self.with(|d| d.record_follows_from(span, follows))
    }

    fn event(&self, event: &tracing::Event<'_>) {
        let entry = serde_json::json!({
            "target": event.metadata().target(),
            "name": event.metadata().name(),
            "level": format!("{:?}", event.metadata().level()),
            // "fields": event.fields(),
        });

        self.entries.write().unwrap().as_mut().unwrap().push(entry);

        self.with(|d| d.event(event))
    }

    fn enter(&self, span: &span::Id) {
        self.with(|d| d.enter(span))
    }

    fn exit(&self, span: &span::Id) {
        self.with(|d| d.exit(span))
    }
}
