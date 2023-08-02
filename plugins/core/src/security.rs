use crate::library::plugin::*;

#[derive(Default)]
pub struct SecurityPluginFactory {}

impl PluginFactory for SecurityPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(SecurityPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct SecurityPlugin {}

impl Plugin for SecurityPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "security"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for SecurityPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::ChangePasswordActionParser {}, i)
    }
}

pub mod model {}

pub mod actions {
    use crate::library::actions::*;
    use engine::Passwords;

    #[action]
    pub struct ChangePasswordAction {
        pub password: String,
    }

    impl Action for ChangePasswordAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (_world, living, _area) = surroundings.unpack();
            let mut passwords = living.scope_mut::<Passwords>()?;
            passwords.set(self.password.to_owned());
            passwords.save()?;

            Ok(Effect::Reply(EffectReply::Instance(Rc::new(
                SimpleReply::Done,
            ))))
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct ChangePasswordActionParser {}

    impl ParsesActions for ChangePasswordActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(
                    pair(alt((tag("@password"), tag("\""))), spaces),
                    text_to_end_of_line,
                ),
                |text| {
                    use argon2::{
                        password_hash::{rand_core::OsRng, SaltString},
                        Argon2, PasswordHasher,
                    };

                    let salt = SaltString::generate(&mut OsRng);
                    let hashed_password = Argon2::default()
                        .hash_password(text.as_bytes(), &salt)
                        .map(|hash| hash.to_string())
                        .expect("hashing password failed");

                    Box::new(ChangePasswordAction {
                        password: hashed_password,
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use crate::library::tests::*;

    #[test]
    fn it_sets_the_users_default_password() -> Result<()> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().build()?;

        let action = try_parsing(ChangePasswordActionParser {}, "@password hellohello")?;
        let action = action.unwrap();
        let reply = action.perform(session.clone(), &surroundings)?;
        let (_, _person, _area) = surroundings.unpack();

        insta::assert_json_snapshot!(reply.to_debug_json()?);

        build.close()?;

        Ok(())
    }
}
