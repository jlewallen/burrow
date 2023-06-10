use anyhow::Result;
use std::marker::PhantomData;
use tokio::sync::mpsc;
use tracing::*;

use plugins_rpc_proto::{
    AlwaysErrorsServices, Completed, Inbox, Message, Payload, PayloadMessage, Query, QueryMessage,
    Sender, ServerProtocol, Services, SessionKey, Surroundings,
};

use crate::SessionServices;

#[derive(Debug)]
enum ChannelMessage {
    Query(Option<Query>),
    Payload(Payload),
    Shutdown,
}

pub struct TokioChannelServer<P> {
    session_key: SessionKey,
    server: ServerProtocol,
    server_tx: mpsc::Sender<ChannelMessage>,
    agent_tx: mpsc::Sender<ChannelMessage>,
    rx_agent: mpsc::Receiver<ChannelMessage>,
    rx_stopped: Option<tokio::sync::oneshot::Receiver<bool>>,
    _marker: PhantomData<P>,
}

async fn process_payload<T: Inbox<PayloadMessage, QueryMessage>>(
    session_key: &SessionKey,
    payload: Payload,
    agent_tx: &mpsc::Sender<ChannelMessage>,
    agent: &mut T,
) -> Result<()> {
    let mut to_server: Sender<_> = Default::default();
    let message = session_key.message(payload);
    agent.deliver(&message, &mut to_server)?;

    for sending in to_server.into_iter() {
        trace!("sending {:?}", sending);
        agent_tx
            .send(ChannelMessage::Query(sending.into_body()))
            .await?;
    }

    Ok(())
}

async fn process_query<H>(
    session_key: &SessionKey,
    query: Option<Query>,
    server_tx: &mpsc::Sender<ChannelMessage>,
    server: &mut ServerProtocol,
    services: &H,
) -> Result<Completed>
where
    H: Services,
{
    fn apply_query(
        server: &mut ServerProtocol,
        message: &Message<Option<Query>>,
        services: &dyn Services,
    ) -> Result<Sender<PayloadMessage>> {
        let mut to_agent: Sender<_> = Default::default();
        server.apply(message, &mut to_agent, services)?;

        Ok(to_agent)
    }

    let message = session_key.message(query);
    let to_agent: Sender<_> = apply_query(server, &message, services)?;

    for sending in to_agent.into_iter() {
        trace!("sending {:?}", sending);
        server_tx
            .send(ChannelMessage::Payload(sending.into_body()))
            .await?;
    }

    Ok(server.completed())
}

impl<P> TokioChannelServer<P>
where
    P: Inbox<PayloadMessage, QueryMessage> + Send + Default,
{
    pub async fn new() -> Self {
        let (stopped_tx, rx_stopped) = tokio::sync::oneshot::channel::<bool>();
        let (agent_tx, rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        let session_key: SessionKey = "session-tokio".into();

        tokio::spawn({
            let session_key = session_key.clone();
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = P::default();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    if let ChannelMessage::Payload(payload) = cm {
                        if let Err(e) =
                            process_payload(&session_key, payload, &agent_tx, &mut agent).await
                        {
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

        let server = ServerProtocol::new(session_key.clone());

        Self {
            session_key,
            server,
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
                match process_query(
                    &self.session_key,
                    query,
                    &self.server_tx,
                    &mut self.server,
                    services,
                )
                .await
                {
                    Err(e) => panic!("Processing query: {:?}", e),
                    Ok(completed) => match completed {
                        Completed::Busy => continue,
                        Completed::Continue => break,
                    },
                }
            }
        }

        Ok(())
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.agent_tx.send(ChannelMessage::Query(None)).await?;

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
