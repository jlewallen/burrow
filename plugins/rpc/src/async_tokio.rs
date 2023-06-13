use anyhow::Result;
use std::marker::PhantomData;
use tokio::sync::mpsc;
use tracing::*;

use plugins_rpc_proto::{Inbox, Payload, Query, Sender, Surroundings};

use crate::{
    querying::Querying,
    sessions::{AlwaysErrorsServices, Services},
    SessionServices,
};

#[derive(Debug)]
enum ChannelMessage {
    Query(Query),
    Payload(Payload),
    Shutdown,
}

pub struct TokioChannelServer<P> {
    server_tx: mpsc::Sender<ChannelMessage>,
    agent_tx: mpsc::Sender<ChannelMessage>,
    rx_agent: mpsc::Receiver<ChannelMessage>,
    rx_stopped: Option<tokio::sync::oneshot::Receiver<bool>>,
    _marker: PhantomData<P>,
}

async fn process_payload<T: Inbox<Payload, Query>>(
    payload: Payload,
    agent_tx: &mpsc::Sender<ChannelMessage>,
    agent: &mut T,
) -> Result<()> {
    let mut to_server: Sender<_> = Default::default();
    agent.deliver(&payload, &mut to_server)?;

    for sending in to_server.into_iter() {
        trace!("sending {:?}", sending);
        agent_tx.send(ChannelMessage::Query(sending)).await?;
    }

    Ok(())
}

async fn process_query<H>(
    query: &Query,
    server_tx: &mpsc::Sender<ChannelMessage>,
    services: &H,
) -> Result<bool>
where
    H: Services,
{
    fn apply_query(query: &Query, services: &dyn Services) -> Result<Sender<Payload>> {
        let mut to_agent: Sender<_> = Default::default();
        let querying = Querying::new();
        querying.service(query, &mut to_agent, services)?;

        Ok(to_agent)
    }

    let to_agent: Sender<_> = apply_query(query, services)?;

    for sending in to_agent.into_iter() {
        trace!("sending {:?}", sending);
        server_tx.send(ChannelMessage::Payload(sending)).await?;
    }

    Ok(true)
}

impl<P> TokioChannelServer<P>
where
    P: Inbox<Payload, Query> + Send + Default,
{
    pub async fn new() -> Self {
        let (stopped_tx, rx_stopped) = tokio::sync::oneshot::channel::<bool>();
        let (agent_tx, rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        tokio::spawn({
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = P::default();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    if let ChannelMessage::Payload(payload) = cm {
                        if let Err(e) = process_payload(payload, &agent_tx, &mut agent).await {
                            warn!("Payload error: {:?}", e);
                        }
                    } else {
                        break;
                    }
                }

                if let Err(e) = stopped_tx.send(true) {
                    warn!("Send stopped error: {:?}", e);
                }
            }
        });

        Self {
            server_tx,
            agent_tx,
            rx_agent,
            rx_stopped: Some(rx_stopped),
            _marker: Default::default(),
        }
    }

    async fn drive<H>(&mut self, services: &H) -> Result<()>
    where
        H: Services,
    {
        // Agent is transmitting queries to us and we're receiving from them.
        while let Some(cm) = self.rx_agent.recv().await {
            if let ChannelMessage::Query(query) = cm {
                match process_query(&query, &self.server_tx, services).await {
                    Err(e) => panic!("Processing query: {:?}", e),
                    Ok(false) => continue,
                    Ok(true) => break,
                }
            }
        }

        Ok(())
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.agent_tx
            .send(ChannelMessage::Query(Query::Bootstrap))
            .await?;

        self.drive(&AlwaysErrorsServices {}).await?;

        Ok(())
    }

    pub async fn have_surroundings(&mut self, surroundings: &Surroundings) -> Result<()> {
        self.server_tx
            .send(ChannelMessage::Payload(Payload::Surroundings(
                surroundings.clone(),
            )))
            .await?;

        self.drive(&SessionServices::new_for_my_session()?).await?;

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        trace!("stopping");

        self.server_tx.send(ChannelMessage::Shutdown).await?;

        if let Some(receiver) = self.rx_stopped.take() {
            receiver.await?;
        } else {
            warn!("No channel in stop, were we already stopped?");
        }

        debug!("stopped");

        Ok(())
    }
}
