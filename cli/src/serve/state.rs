use anyhow::Result;
use chrono::{DateTime, Utc};
use std::rc::Rc;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tracing::*;

use engine::{
    AfterTick, DevNullNotifier, Domain, HasUsernames, Notifier, Passwords, SessionOpener,
};
use kernel::{DomainEvent, EntityKey, EntryResolver};

use super::ServerMessage;

pub struct AppState {
    pub domain: Domain,
    pub tick_deadline: Mutex<Option<DateTime<Utc>>>,
    pub tx: broadcast::Sender<ServerMessage>,
    pub env: Config,
}

pub struct Config {
    pub jwt_secret: String,
    pub jwt_expires_in: String,
    pub jwt_maxage: i32,
}

impl Config {
    pub fn from_env() -> Option<Self> {
        let jwt_secret = std::env::var("JWT_SECRET").ok()?;
        let jwt_expires_in = std::env::var("JWT_EXPIRED_IN").ok()?;
        let jwt_maxage = std::env::var("JWT_MAXAGE").ok()?;
        Some(Self {
            jwt_secret,
            jwt_expires_in,
            jwt_maxage: jwt_maxage.parse::<i32>().ok()?,
        })
    }
}

pub struct SenderNotifier {
    tx: broadcast::Sender<ServerMessage>,
}

impl Notifier for SenderNotifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn DomainEvent>) -> Result<()> {
        debug!("notify {:?} -> {:?}", audience, observed);

        let serialized = observed.to_tagged_json()?;
        let outgoing = ServerMessage::Notify(audience.to_string(), serialized);
        self.tx.send(outgoing)?;

        Ok(())
    }
}

impl AppState {
    pub fn new(domain: Domain) -> Self {
        let env_config = Config::from_env();
        let (tx, _rx) = broadcast::channel(100);
        AppState {
            domain: domain.clone(),
            tick_deadline: Default::default(),
            tx,
            env: env_config.unwrap_or(Config {
                jwt_secret: "RSgTQSRXNxeVIZfPOK1dIQ==".to_owned(),
                jwt_expires_in: "24h".to_owned(),
                jwt_maxage: 60,
            }),
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

        let world = session.world()?.expect("No world");
        let maybe_key = match world.find_name_key(name)? {
            Some(key) => {
                let user = session.entry(&kernel::LookupBy::Key(&key))?;
                let user = user.unwrap();
                let hash = user
                    .maybe_scope::<Passwords>()?
                    .map(|s| s.get().map(|s| s.clone()))
                    .flatten();

                Some((key, hash))
            }
            None => None,
        };

        session.close(&DevNullNotifier::default())?;

        Ok(maybe_key)
    }

    pub fn remove_session(&self, _session: &ClientSession) {}
}

pub struct ClientSession {
    pub key: EntityKey,
}