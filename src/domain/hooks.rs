use std::{cell::RefCell, rc::Rc};

pub trait HookOutcome {
    fn or(&self, other: &Self) -> Self;
}

pub struct Hooks<T> {
    instances: Rc<RefCell<Vec<T>>>,
}

impl<T> Hooks<T> {
    pub fn new() -> Self {
        Self {
            instances: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn register(&self, hook: T) {
        self.instances.borrow_mut().push(hook);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::BuildActionArgs,
        kernel::{ActionArgs, Surroundings},
    };
    use anyhow::Result;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[derive(Clone)]
    pub enum CanJump {
        Allow,
        Prevent,
    }

    impl Default for CanJump {
        fn default() -> Self {
            Self::Allow
        }
    }

    impl HookOutcome for CanJump {
        fn or(&self, other: &CanJump) -> CanJump {
            match (self, other) {
                (_, CanJump::Prevent) => CanJump::Prevent,
                (CanJump::Prevent, _) => CanJump::Prevent,
                (_, _) => CanJump::Allow,
            }
        }
    }

    trait JumpingHook {
        fn jumping(&self, surroundings: &Surroundings) -> Result<CanJump>;
    }

    struct AlwaysAllow {}

    impl JumpingHook for AlwaysAllow {
        fn jumping(&self, _surroundings: &Surroundings) -> Result<CanJump> {
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

    impl JumpingHook for FailsEveryOtherTime {
        fn jumping(&self, _surroundings: &Surroundings) -> Result<CanJump> {
            if self.add_one() % 2 == 0 {
                Ok(CanJump::Prevent)
            } else {
                Ok(CanJump::Allow)
            }
        }
    }

    impl JumpingHook for Hooks<Box<dyn JumpingHook>> {
        fn jumping(&self, surroundings: &Surroundings) -> Result<CanJump> {
            Ok(self
                .instances
                .borrow()
                .iter()
                .map(|h| h.jumping(&surroundings))
                .collect::<Result<Vec<CanJump>>>()?
                .iter()
                .fold(CanJump::default(), |c, h| c.or(&h)))
        }
    }

    #[test]
    fn it_should_do_nothing_on_empty_hook() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build.plain().try_into()?;
        let jumping: Hooks<Box<dyn JumpingHook>> = Hooks::new();
        jumping.register(Box::new(FailsEveryOtherTime {
            counter: AtomicI32::new(0),
        }));
        jumping.register(Box::new(AlwaysAllow {}));
        jumping.jumping(&args.surroundings)?;

        Ok(())
    }
}
