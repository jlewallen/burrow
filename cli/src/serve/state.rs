use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tracing::*;

use engine::prelude::*;
use kernel::prelude::*;

use plugins_core::carrying::model::Containing;
use plugins_core::fashion::model::Wearing;
use plugins_core::tools;
use plugins_rune::Behaviors;

use super::handlers::RegisterUser;
use super::ServerMessage;

pub struct AppState {
    pub domain: Domain,
    pub tick_deadline: Mutex<Option<DateTime<Utc>>>,
    pub tx: broadcast::Sender<ServerMessage>,
    pub env: Config,
}

pub struct Config {
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> Option<Self> {
        let jwt_secret = std::env::var("JWT_SECRET").ok()?;
        Some(Self { jwt_secret })
    }
}

pub struct SenderNotifier {
    tx: broadcast::Sender<ServerMessage>,
}

impl Notifier for SenderNotifier {
    fn notify(&self, audience: &EntityKey, observed: &TaggedJson) -> Result<()> {
        trace!("notify {:?} -> {:?}", audience, observed);

        let serialized = observed.clone().into_tagged();
        let outgoing = ServerMessage::Notify(audience.to_string(), serialized);
        self.tx.send(outgoing)?;

        Ok(())
    }
}

impl AppState {
    pub fn new(domain: Domain) -> Self {
        let env = Config::from_env().expect("no config");
        let (tx, _rx) = broadcast::channel(100);
        AppState {
            domain: domain.clone(),
            tick_deadline: Default::default(),
            tx,
            env,
        }
    }

    pub fn try_start_session(&self, key: &EntityKey) -> Result<ClientSession> {
        Ok(ClientSession { key: key.clone() })
    }

    pub async fn tick(&self, now: DateTime<Utc>) -> Result<AfterTick> {
        let can_tick = {
            let tick_deadline = self.tick_deadline.lock().await;

            tick_deadline.filter(|deadline| *deadline > now)
        };

        match can_tick {
            Some(deadline) => Ok(AfterTick::Deadline(deadline)),
            None => Ok(self.domain.tick(now, &self.notifier())?),
        }
    }

    pub fn notifier(&self) -> SenderNotifier {
        SenderNotifier {
            tx: self.tx.clone(),
        }
    }

    pub fn find_user_key(&self, name: &str) -> Result<Option<(EntityKey, Option<String>)>> {
        let session = self.domain.open_session().expect("Error opening session");
        let session = session.set_session()?;

        let world = session.world()?.expect("No world");
        let maybe_key = match world.find_name_key(name)? {
            Some(key) => {
                let user = session.entity(&kernel::prelude::LookupBy::Key(&key))?;
                let user = user.unwrap();
                let hash = user.scope::<Credentials>()?.and_then(|s| s.get().cloned());

                Some((key, hash))
            }
            None => None,
        };

        session.close(&DevNullNotifier::default())?;

        Ok(maybe_key)
    }

    pub fn register_user(&self, user: &RegisterUser) -> Result<EntityKey> {
        let session = self.domain.open_session().expect("Error opening session");
        let session = session.set_session()?;

        let world = session.world()?.expect("No world");
        let existing_key = world
            .find_name_key(&user.email)
            .with_context(|| "find-name")?;
        if existing_key.is_some() {
            warn!("already registered");
            return Err(anyhow::anyhow!("already registered"));
        }

        info!("registering");

        use argon2::{
            password_hash::{rand_core::OsRng, SaltString},
            Argon2, PasswordHasher,
        };

        let salt = SaltString::generate(&mut OsRng);
        let hashed_password = Argon2::default()
            .hash_password(user.password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .expect("hashing password failed");

        let welcome_area_key = world.get_welcome_area()?.expect("no welcome area");
        let welcome_area = session
            .entity(&kernel::prelude::LookupBy::Key(&welcome_area_key))?
            .expect("no welcome area");

        let creating = build_entity()
            .creator(world.entity_ref())
            .living()
            .default_scope::<Wearing>()?
            .default_scope::<Containing>()?
            .default_scope::<Behaviors>()?
            .name(&user.name)
            .try_into()?;

        let creating = session.add_entity(creating)?;
        tools::set_occupying(&welcome_area, &vec![creating.clone()])?;
        let mut credentials = creating.scope_mut::<Credentials>()?;
        credentials.set(hashed_password);
        credentials.save()?;

        let key = creating.key().clone();
        world.add_username_to_key(&user.email, &key)?;

        session.close(&DevNullNotifier::default())?;

        info!("registered!");

        Ok(key)
    }

    pub fn remove_session(&self, _session: &ClientSession) {}
}

pub struct ClientSession {
    pub key: EntityKey,
}
