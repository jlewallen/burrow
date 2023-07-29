use anyhow::Result;
use tokio::task::JoinHandle;

use crate::{evaluate_fixture, test_domain_with, HoldingKeyInVessel, Noop, WorldFixture, USERNAME};
use engine::storage::{PersistedEntity, StorageFactory};
use engine::{Domain, Session, SessionOpener};

async fn test_domain() -> Result<AsyncFriendlyDomain> {
    let storage_factory = sqlite::Factory::new(sqlite::MEMORY_SPECIAL)?;

    storage_factory.migrate()?;

    Ok(AsyncFriendlyDomain::wrap(test_domain_with(
        storage_factory,
    )?))
}

#[tokio::test]
async fn it_evaluates_a_simple_look() -> Result<()> {
    let domain = test_domain().await?;
    domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

#[tokio::test]
async fn it_evaluates_two_simple_looks_same_session() -> Result<()> {
    let domain = test_domain().await?;
    domain
        .evaluate::<HoldingKeyInVessel>(&["look", "look"])
        .await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

#[tokio::test]
async fn it_evaluates_two_simple_looks_separate_session() -> Result<()> {
    let domain = test_domain().await?;
    domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
    domain.evaluate::<Noop>(&["look"]).await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

#[tokio::test]
async fn it_can_drop_held_container() -> Result<()> {
    let domain = test_domain().await?;
    domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
    domain.evaluate::<Noop>(&["drop vessel"]).await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

#[tokio::test]
async fn it_can_rehold_dropped_container() -> Result<()> {
    let domain = test_domain().await?;
    domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
    domain.evaluate::<Noop>(&["drop vessel"]).await?;
    domain.evaluate::<Noop>(&["hold vessel"]).await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

#[tokio::test]
async fn it_can_go_east() -> Result<()> {
    let domain = test_domain().await?;
    domain.evaluate::<HoldingKeyInVessel>(&["look"]).await?;
    domain.evaluate::<Noop>(&["go east"]).await?;
    domain.evaluate::<Noop>(&["look"]).await?;
    insta::assert_json_snapshot!(domain.snapshot().await?);
    domain.stop().await?;

    Ok(())
}

/*
#[cfg(test)]
#[ctor::ctor]
fn initialize_tests() {
    plugins_core::log_test();
}
*/

#[derive(Clone)]
pub struct AsyncFriendlyDomain {
    domain: Domain,
}

impl AsyncFriendlyDomain {
    pub fn wrap(domain: Domain) -> Self {
        Self { domain }
    }

    pub async fn query_all(&self) -> Result<Vec<PersistedEntity>> {
        self.domain.query_all()
    }

    #[cfg(test)]
    pub async fn snapshot(&self) -> Result<serde_json::Value> {
        let json: Vec<serde_json::Value> = self
            .query_all()
            .await?
            .into_iter()
            .map(|p| p.to_json_value())
            .collect::<Result<_>>()?;

        Ok(serde_json::Value::Array(json))
    }

    pub async fn evaluate<W>(&self, text: &'static [&'static str]) -> Result<()>
    where
        W: WorldFixture + Default,
    {
        let handle: JoinHandle<Result<()>> = tokio::task::spawn_blocking({
            let sessions = self.clone();
            move || {
                evaluate_fixture::<W, _>(&sessions, USERNAME, text)?;

                Ok(())
            }
        });

        Ok(handle.await??)
    }

    pub async fn stop(&self) -> Result<()> {
        let domain = self.domain.clone();
        tokio::task::spawn_blocking(move || domain.stop()).await?
    }
}

impl SessionOpener for AsyncFriendlyDomain {
    fn open_session(&self) -> Result<std::rc::Rc<Session>> {
        self.domain.open_session()
    }
}
