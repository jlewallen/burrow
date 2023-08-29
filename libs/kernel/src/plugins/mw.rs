// This started by copying from https://docs.rs/ureq/latest/src/ureq/middleware.rs.html#135-146

use std::rc::Rc;

use anyhow::Result;

use crate::actions::{Effect, Perform};

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
    use anyhow::Result;
    use replies::{TaggedJson, ToTaggedJson};
    use serde::Serialize;

    use super::*;
    use crate::{
        actions::{Action, HasTag, PerformAction},
        model::{build_entity, EntityKey, EntityPtr, Identity},
    };

    #[allow(dead_code)]
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
            next.handle(value)
        }
    }

    #[derive(Default, Serialize, Debug)]
    struct ExampleAction {}

    impl HasTag for ExampleAction {
        fn tag() -> std::borrow::Cow<'static, str>
        where
            Self: Sized,
        {
            "exampleAction".into()
        }
    }

    impl ToTaggedJson for ExampleAction {
        fn to_tagged_json(
            &self,
        ) -> std::result::Result<replies::TaggedJson, replies::TaggedJsonError> {
            Ok(TaggedJson::new(
                Self::tag().to_string(),
                serde_json::to_value(self)?.into(),
            ))
        }
    }

    impl Action for ExampleAction {
        fn is_read_only(&self) -> bool {
            true
        }

        fn perform(
            &self,
            _session: crate::session::SessionRef,
            _surroundings: &crate::surround::Surroundings,
        ) -> crate::actions::ReplyResult {
            todo!()
        }
    }

    #[test]
    fn should_call_handle_with_no_middleware() -> Result<()> {
        let actor = EntityPtr::new_from_entity(
            build_entity()
                .living()
                .with_key(EntityKey::new("E-0"))
                .identity(Identity::new("".to_lowercase(), "".to_owned()))
                .try_into()?,
        );

        let all: Vec<Rc<dyn Middleware>> = Vec::new();
        let request_fn =
            Box::new(|_value: Perform| -> Result<Effect, anyhow::Error> { Ok(Effect::Ok) });
        let action = PerformAction::Instance(Rc::new(ExampleAction::default()));
        let perform = Perform::Actor { actor, action };
        let effect = apply_middleware(&all, perform, request_fn)?;
        assert_eq!(effect, Effect::Ok);
        Ok(())
    }

    #[test]
    fn should_middleware_in_expected_order() -> Result<()> {
        let actor = EntityPtr::new_from_entity(
            build_entity()
                .living()
                .with_key(EntityKey::new("E-0"))
                .identity(Identity::new("".to_lowercase(), "".to_owned()))
                .try_into()?,
        );

        let all: Vec<Rc<dyn Middleware>> =
            vec![Rc::new(Middle::from("A")), Rc::new(Middle::from("B"))];
        let request_fn =
            Box::new(|_value: Perform| -> Result<Effect, anyhow::Error> { Ok(Effect::Ok) });
        let action = PerformAction::Instance(Rc::new(ExampleAction::default()));
        let perform = Perform::Actor { actor, action };
        let effect = apply_middleware(&all, perform, request_fn)?;
        assert_eq!(effect, Effect::Ok);
        Ok(())
    }
}
