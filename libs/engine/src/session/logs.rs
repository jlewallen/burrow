use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};
use tracing::dispatcher::WeakDispatch;
use tracing::*;

pub(crate) fn capture<V, E, F, H>(f: F, h: H) -> Result<V, E>
where
    F: FnOnce() -> Result<V, E>,
    H: FnOnce(Logs) -> Result<(), E>,
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
    spans: Arc<RwLock<HashMap<span::Id, BTreeMap<String, serde_json::Value>>>>,
    entries: Arc<RwLock<Option<Logs>>>,
    stack: Arc<RwLock<Vec<span::Id>>>,
}

#[derive(Default, Clone)]
pub(crate) struct Logs {
    important: bool,
    logs: Vec<serde_json::Value>,
}

impl Logs {
    pub(crate) fn is_important(&self) -> bool {
        self.important
    }

    fn push(&mut self, important: bool, entry: serde_json::Value) {
        if important {
            self.important = true;
        }
        self.logs.push(entry);
    }
}

impl Into<Vec<serde_json::Value>> for Logs {
    fn into(self) -> Vec<serde_json::Value> {
        self.logs
    }
}

impl SessionSubscriber {
    fn new(target: WeakDispatch) -> Self {
        Self {
            target,
            spans: Default::default(),
            stack: Default::default(),
            entries: Arc::new(RwLock::new(Some(Default::default()))),
        }
    }

    fn take(&mut self) -> Logs {
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
        let id = self.with(|d| d.new_span(span));

        let mut values = BTreeMap::new();
        let mut visitor = JsonVisitor {
            values: &mut values,
        };
        span.record(&mut visitor);

        if !values.is_empty() {
            self.spans.write().unwrap().insert(id.clone(), values);
        }

        id
    }

    fn try_close(&self, id: span::Id) -> bool {
        self.with(|d| d.try_close(id))
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        self.with(|d| d.record(span, values))
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        self.with(|d| d.record_follows_from(span, follows))
    }

    fn event(&self, event: &tracing::Event<'_>) {
        let mut fields = BTreeMap::new();
        let mut visitor = JsonVisitor {
            values: &mut fields,
        };
        event.record(&mut visitor);

        let stack = self.stack.read().unwrap();
        let spans = self.spans.read().unwrap();

        let span_stack = stack
            .iter()
            .flat_map(|id| spans.get(id))
            .collect::<Vec<_>>();

        let level = event.metadata().level();
        let important = *level == Level::WARN || *level == Level::ERROR;
        let entry = serde_json::json!({
            "target": event.metadata().target(),
            "name": event.metadata().name(),
            "level": event.metadata().level().to_string(),
            "spans": span_stack,
            "fields": fields,
        });

        self.entries
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .push(important, entry);

        self.with(|d| d.event(event))
    }

    fn enter(&self, span: &span::Id) {
        self.stack.write().unwrap().push(span.clone());
        self.with(|d| d.enter(span))
    }

    fn exit(&self, span: &span::Id) {
        self.with(|d| d.exit(span));
        self.stack.write().unwrap().pop();
    }
}

struct JsonVisitor<'a> {
    values: &'a mut BTreeMap<String, serde_json::Value>,
}

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.values
            .insert(field.name().to_string(), serde_json::json!(value));
    }

    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.values
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.values
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.values
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.values
            .insert(field.name().to_string(), serde_json::Value::from(value));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            name if name.starts_with("r#") => {
                self.values.insert(
                    name[2..].to_string(),
                    serde_json::Value::from(format!("{:?}", value)),
                );
            }
            name => {
                self.values.insert(
                    name.to_string(),
                    serde_json::Value::from(format!("{:?}", value)),
                );
            }
        };
    }
}
