use std::{any::Any, cell::RefCell, collections::HashMap};

pub trait HookOutcome {
    fn fold(&self, other: &Self) -> Self;
}

pub struct Hooks<T> {
    pub instances: RefCell<Vec<T>>, // TODO Move fold call into member
}

impl<T> Default for Hooks<T> {
    fn default() -> Self {
        Self {
            instances: Default::default(),
        }
    }
}

impl<T> Hooks<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, hook: T) {
        self.instances.borrow_mut().push(hook);
    }
}

pub trait HooksSet {
    fn hooks_key() -> &'static str
    where
        Self: Sized;
}

#[derive(Default)]
pub struct ManagedHooks {
    hooks: RefCell<HashMap<&'static str, Box<dyn Any>>>,
}

impl ManagedHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<T, F>(&self, with_fn: F) -> anyhow::Result<()>
    where
        F: Fn(&mut T) -> anyhow::Result<()>,
        T: HooksSet + Default + 'static,
    {
        self.invoke::<T, (), _>(with_fn)
    }

    pub fn invoke<T, V, F>(&self, with_fn: F) -> anyhow::Result<V>
    where
        F: Fn(&mut T) -> anyhow::Result<V>,
        T: HooksSet + Default + 'static,
    {
        let mut all_hooks = self.hooks.borrow_mut();
        // Would love to use .or_default here, only the 'as' call to produce a
        // Box<dyn Any> throws a wrench in that plan.
        let hooks = all_hooks
            .entry(<T as HooksSet>::hooks_key())
            .or_insert_with(|| Box::<T>::default() as Box<dyn Any>);
        let hooks = hooks
            .downcast_mut()
            .expect("Hooks of unexpected type, duplicate hooks_key?");
        with_fn(hooks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::sync::atomic::{AtomicI32, Ordering};

    pub enum FakeSurroundings {
        Void,
    }

    #[derive(Clone, PartialEq, Debug, Default)]
    pub enum CanJump {
        #[default]
        Allow,
        Prevent,
    }

    impl HookOutcome for CanJump {
        fn fold(&self, other: &CanJump) -> CanJump {
            match (self, other) {
                (_, CanJump::Prevent) => CanJump::Prevent,
                (CanJump::Prevent, _) => CanJump::Prevent,
                (_, _) => CanJump::Allow,
            }
        }
    }

    trait BeforeJumpingHook {
        fn before_jumping(&self, surroundings: &FakeSurroundings) -> Result<CanJump>;
    }

    #[derive(Default)]
    struct JumpingHooks {
        before_jumping: Hooks<Box<dyn BeforeJumpingHook>>,
    }

    impl HooksSet for JumpingHooks {
        fn hooks_key() -> &'static str
        where
            Self: Sized,
        {
            "jumping"
        }
    }

    struct AlwaysAllow {}

    impl BeforeJumpingHook for AlwaysAllow {
        fn before_jumping(&self, _surroundings: &FakeSurroundings) -> Result<CanJump> {
            Ok(CanJump::Allow)
        }
    }

    struct FailsEveryOtherTime {
        counter: AtomicI32,
    }

    impl FailsEveryOtherTime {
        fn add_one(&self) -> i32 {
            self.counter.fetch_add(1, Ordering::Relaxed)
        }
    }

    impl BeforeJumpingHook for FailsEveryOtherTime {
        fn before_jumping(&self, _surroundings: &FakeSurroundings) -> Result<CanJump> {
            if self.add_one() % 2 == 0 {
                Ok(CanJump::Prevent)
            } else {
                Ok(CanJump::Allow)
            }
        }
    }

    impl BeforeJumpingHook for Hooks<Box<dyn BeforeJumpingHook>> {
        fn before_jumping(&self, surroundings: &FakeSurroundings) -> Result<CanJump> {
            Ok(self
                .instances
                .borrow()
                .iter()
                .map(|h| h.before_jumping(surroundings))
                .collect::<Result<Vec<CanJump>>>()?
                .iter()
                .fold(CanJump::default(), |c, h| c.fold(h)))
        }
    }

    #[test]
    fn it_should_do_nothing_on_empty_hook() -> Result<()> {
        let jumping: Hooks<Box<dyn BeforeJumpingHook>> = Hooks::new();
        assert_eq!(
            jumping.before_jumping(&FakeSurroundings::Void)?,
            CanJump::Allow
        );

        Ok(())
    }

    #[test]
    fn it_should_return_single_hook_outcome() -> Result<()> {
        let jumping: Hooks<Box<dyn BeforeJumpingHook>> = Hooks::new();
        jumping.register(Box::new(FailsEveryOtherTime {
            counter: AtomicI32::new(0),
        }));
        assert_eq!(
            jumping.before_jumping(&FakeSurroundings::Void)?,
            CanJump::Prevent
        );

        Ok(())
    }

    #[test]
    fn it_should_fold_multiple_hook_outcomes() -> Result<()> {
        let jumping: Hooks<Box<dyn BeforeJumpingHook>> = Hooks::new();
        jumping.register(Box::new(FailsEveryOtherTime {
            counter: AtomicI32::new(0),
        }));
        jumping.register(Box::new(AlwaysAllow {}));
        assert_eq!(
            jumping.before_jumping(&FakeSurroundings::Void)?,
            CanJump::Prevent
        );
        assert_eq!(
            jumping.before_jumping(&FakeSurroundings::Void)?,
            CanJump::Allow
        );

        Ok(())
    }

    #[test]
    fn it_should_allow_easy_registration_of_new_hooks() -> Result<()> {
        let managed_hooks = ManagedHooks::new();

        managed_hooks.with::<JumpingHooks, _>(|h| {
            h.before_jumping.register(Box::new(AlwaysAllow {}));
            Ok(())
        })?;

        managed_hooks.with::<JumpingHooks, _>(|h| {
            h.before_jumping.register(Box::new(AlwaysAllow {}));
            Ok(())
        })?;

        Ok(())
    }
}
