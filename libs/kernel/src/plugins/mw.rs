// This started by copying from https://docs.rs/ureq/latest/src/ureq/middleware.rs.html#135-146

use std::rc::Rc;

use crate::{Effect, Perform};
use anyhow::Result;

pub trait Middleware: 'static {
    fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error>;
}

pub struct MiddlewareNext<'a> {
    pub chain: &'a mut (dyn Iterator<Item = &'a dyn Middleware>),
    // Since request_fn consumes the Perform, we must have an FnOnce.
    //
    // It's possible to get rid of this Box if we make MiddlewareNext generic
    // over some type variable, i.e. MiddlewareNext<'a, R> where R: FnOnce...
    // however that would "leak" to Middleware::handle introducing a complicated
    // type signature that is totally irrelevant for someone implementing a middleware.
    //
    // So in the name of having a sane external API, we accept this Box.
    pub request_fn: Box<dyn FnOnce(Perform) -> Result<Effect, anyhow::Error> + 'a>,
}

impl<'a> MiddlewareNext<'a> {
    /// Continue the middleware chain by providing (a possibly amended) [`Perform`].
    pub fn handle(self, request: Perform) -> Result<Effect, anyhow::Error> {
        if let Some(step) = self.chain.next() {
            step.handle(request, self)
        } else {
            (self.request_fn)(request)
        }
    }
}

pub fn apply_middleware<F>(
    all: &[Rc<dyn Middleware>],
    value: Perform,
    request_fn: F,
) -> Result<Effect>
where
    F: Fn(Perform) -> Result<Effect>,
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
    F: Fn(Perform, MiddlewareNext) -> Result<Effect, anyhow::Error> + Send + Sync + 'static,
{
    fn handle(&self, request: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
        (self)(request, next)
    }
}

#[cfg(test)]
mod tests {
    use crate::TracePath;

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
        fn handle(&self, value: Perform, next: MiddlewareNext) -> Result<Effect, anyhow::Error> {
            match value {
                Perform::Ping(value) => {
                    match next.handle(Perform::Ping(value.push(self.token.clone())))? {
                        Effect::Pong(value) => Ok(Effect::Pong(value.push(self.token.clone()))),
                        _ => todo!(),
                    }
                }
                _ => todo!(),
            }
        }
    }

    #[test]
    fn should_call_handle_with_no_middleware() -> Result<()> {
        let all: Vec<Rc<dyn Middleware>> = Vec::new();
        let request_fn = Box::new(|value: Perform| -> Result<Effect, anyhow::Error> {
            match value {
                Perform::Ping(value) => Ok(Effect::Pong(value.push("$".to_owned()))),
                _ => todo!(),
            }
        });
        let pong = match apply_middleware(&all, Perform::Ping(TracePath::default()), request_fn)? {
            Effect::Pong(pong) => format!("{:?}", pong),
            _ => panic!(),
        };
        assert_eq!(pong, "$");
        Ok(())
    }

    #[test]
    fn should_middleware_in_expected_order() -> Result<()> {
        let all: Vec<Rc<dyn Middleware>> =
            vec![Rc::new(Middle::from("A")), Rc::new(Middle::from("B"))];
        let request_fn = Box::new(|value: Perform| -> Result<Effect, anyhow::Error> {
            match value {
                Perform::Ping(value) => Ok(Effect::Pong(value.push("$".to_owned()))),
                _ => todo!(),
            }
        });
        let pong = match apply_middleware(&all, Perform::Ping(Default::default()), request_fn)? {
            Effect::Pong(pong) => format!("{:?}", pong),
            _ => panic!(),
        };
        assert_eq!(pong, "AB$BA");
        Ok(())
    }
}
