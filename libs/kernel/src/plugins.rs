use anyhow::Result;
use std::time::Instant;
use tracing::*;

use super::{model::*, Action, ManagedHooks};
use crate::{Effect, Perform, Performer, Surroundings};

pub type EvaluationResult = Result<Option<Box<dyn Action>>, EvaluationError>;

pub trait PluginFactory: Send + Sync {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>>;

    fn stop(&self) -> Result<()>;
}

#[derive(Default)]
pub struct RegisteredPlugins {
    factories: Vec<Box<dyn PluginFactory>>,
}

impl RegisteredPlugins {
    pub fn register<P>(&mut self, factory: P)
    where
        P: PluginFactory + 'static,
    {
        self.factories.push(Box::new(factory))
    }

    pub fn create_plugins(&self) -> Result<SessionPlugins> {
        Ok(SessionPlugins::new(
            self.factories
                .iter()
                .map(|f| f.create_plugin())
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    pub fn stop(&self) -> Result<()> {
        for factory in self.factories.iter() {
            factory.stop()?;
        }

        Ok(())
    }
}

pub trait ParsesActions {
    fn try_parse_action(&self, i: &str) -> EvaluationResult;

    fn evaluate_parsed_action(
        &self,
        perform: &dyn Performer,
        consider: Evaluable,
    ) -> Result<Vec<Effect>> {
        match consider {
            Evaluable::Phrase(text) => self
                .try_parse_action(text)
                .ok()
                .flatten()
                .map(|a| perform.perform(Perform::Chain(a)))
                .map_or(Ok(None), |v| v.map(Some))?
                .map_or(Ok(Vec::new()), |v| Ok(vec![v])),
            _ => todo!(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Evaluable<'a> {
    Phrase(&'a str),
    Surroundings(Surroundings),
    Effect(Effect),
}

pub trait Evaluator {
    fn evaluate(&self, perform: &dyn Performer, consider: Evaluable) -> Result<Vec<Effect>>;
}

#[derive(Debug)]
pub struct Incoming {
    pub key: String,
    pub serialized: Vec<u8>,
}

impl Incoming {
    pub fn new(key: String, serialized: Vec<u8>) -> Self {
        Self { key, serialized }
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.key.starts_with(prefix)
    }
}

pub trait Plugin: Evaluator {
    fn plugin_key() -> &'static str
    where
        Self: Sized;

    fn key(&self) -> &'static str;

    fn initialize(&mut self) -> Result<()>;

    fn register_hooks(&self, hooks: &ManagedHooks) -> Result<()>;

    fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()>;

    fn deliver(&self, incoming: &Incoming) -> Result<()>;

    fn stop(&self) -> Result<()>;
}

#[derive(Default)]
pub struct SessionPlugins {
    plugins: Vec<Box<dyn Plugin>>,
}

impl SessionPlugins {
    fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
        Self { plugins }
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        for plugin in self.plugins.iter_mut() {
            let started = Instant::now();
            plugin.initialize()?;
            let elapsed = Instant::now() - started;
            if elapsed.as_millis() > 200 {
                warn!("plugin:{} ready {:?}", plugin.key(), elapsed);
            } else {
                debug!("plugin:{} ready {:?}", plugin.key(), elapsed);
            }
        }
        Ok(())
    }

    pub fn hooks(&self) -> Result<ManagedHooks> {
        let hooks = ManagedHooks::default();
        for plugin in self.plugins.iter() {
            plugin.register_hooks(&hooks)?;
        }
        Ok(hooks)
    }

    pub fn have_surroundings(&self, surroundings: &Surroundings) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.have_surroundings(surroundings)?;
        }
        Ok(())
    }

    pub fn deliver(&self, incoming: Incoming) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.deliver(&incoming)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        for plugin in self.plugins.iter() {
            plugin.stop()?;
        }
        Ok(())
    }
}

impl Evaluator for SessionPlugins {
    fn evaluate(&self, perform: &dyn Performer, consider: Evaluable) -> Result<Vec<Effect>> {
        Ok(self
            .plugins
            .iter()
            .map(|plugin| {
                let _span = span!(Level::INFO, "E", plugin = plugin.key()).entered();
                info!("evaluating");
                plugin.evaluate(perform, consider.clone())
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}

pub type Request = Perform;

pub type Response = Effect;

// This was copied from https://docs.rs/ureq/latest/src/ureq/middleware.rs.html#135-146
pub trait Middleware: 'static {
    fn handle(&self, value: Request, next: MiddlewareNext) -> Result<Response, anyhow::Error>;
}

// This was copied from https://docs.rs/ureq/latest/src/ureq/middleware.rs.html#135-146
pub struct MiddlewareNext<'a> {
    pub(crate) chain: &'a mut (dyn Iterator<Item = &'a dyn Middleware>),
    // Since request_fn consumes the Payload<'a>, we must have an FnOnce.
    //
    // It's possible to get rid of this Box if we make MiddlewareNext generic
    // over some type variable, i.e. MiddlewareNext<'a, R> where R: FnOnce...
    // however that would "leak" to Middleware::handle introducing a complicated
    // type signature that is totally irrelevant for someone implementing a middleware.
    //
    // So in the name of having a sane external API, we accept this Box.
    pub(crate) request_fn: Box<dyn FnOnce(Request) -> Result<Response, anyhow::Error> + 'a>,
}

impl<'a> MiddlewareNext<'a> {
    /// Continue the middleware chain by providing (a possibly amended) [`Request`].
    pub fn handle(self, request: Request) -> Result<Response, anyhow::Error> {
        if let Some(step) = self.chain.next() {
            step.handle(request, self)
        } else {
            (self.request_fn)(request)
        }
    }
}

pub fn apply_middleware<F>(
    all: &[Box<dyn Middleware>],
    value: Request,
    request_fn: F,
) -> Result<Response>
where
    F: Fn(Request) -> Result<Response>,
{
    let chain = &mut all.iter().map(|mw| mw.as_ref());
    let next = MiddlewareNext {
        chain,
        request_fn: Box::new(request_fn),
    };

    next.handle(value)
}

impl<F> Middleware for F
where
    F: Fn(Request, MiddlewareNext) -> Result<Response, anyhow::Error> + Send + Sync + 'static,
{
    fn handle(&self, request: Request, next: MiddlewareNext) -> Result<Response, anyhow::Error> {
        (self)(request, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    pub struct Middle {
        token: String,
    }

    impl Default for Middle {
        fn default() -> Self {
            Self {
                token: "Middle".to_owned(),
            }
        }
    }

    impl Middle {
        fn from(s: &str) -> Self {
            Self {
                token: s.to_owned(),
            }
        }
    }

    impl Middleware for Middle {
        fn handle(&self, value: Request, next: MiddlewareNext) -> Result<Response, anyhow::Error> {
            match value {
                Perform::Ping(value) => {
                    match next.handle(Perform::Ping(format!("{}{}", value, self.token)))? {
                        Effect::Reply(_) => todo!(),
                        Effect::Action(_) => todo!(),
                        Effect::Pong(value) => {
                            Ok(Response::Pong(format!("{}{}", value, self.token)))
                        }
                    }
                }
                Perform::Living {
                    living: _,
                    action: _,
                } => todo!(),
                Perform::Chain(_) => todo!(),
                Perform::Effect(_) => todo!(),
            }
        }
    }

    #[test]
    fn should_call_handle_with_no_middleware() -> Result<()> {
        let all: Vec<Box<dyn Middleware>> = Vec::new();
        let request_fn = Box::new(|value: Request| -> Result<Response, anyhow::Error> {
            match value {
                Perform::Ping(value) => Ok(Response::Pong(format!("{}$", value))),
                Perform::Living {
                    living: _,
                    action: _,
                } => todo!(),
                Perform::Chain(_) => todo!(),
                Perform::Effect(_) => todo!(),
            }
        });
        let pong = match apply_middleware(&all, Perform::Ping("".to_owned()), request_fn)? {
            Effect::Pong(pong) => format!("{}", pong),
            _ => panic!(),
        };
        assert_eq!(pong, "$");
        Ok(())
    }

    #[test]
    fn should_middleware_in_expected_order() -> Result<()> {
        let all: Vec<Box<dyn Middleware>> =
            vec![Box::new(Middle::from("A")), Box::new(Middle::from("B"))];
        let request_fn = Box::new(|value: Request| -> Result<Response, anyhow::Error> {
            match value {
                Perform::Ping(value) => Ok(Response::Pong(format!("{}$", value))),
                Perform::Living {
                    living: _,
                    action: _,
                } => todo!(),
                Perform::Chain(_) => todo!(),
                Perform::Effect(_) => todo!(),
            }
        });
        let pong = match apply_middleware(&all, Perform::Ping("".to_owned()), request_fn)? {
            Effect::Pong(pong) => format!("{}", pong),
            _ => panic!(),
        };
        assert_eq!(pong, "AB$BA");
        Ok(())
    }
}
