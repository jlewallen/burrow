use anyhow::Result;
use std::marker::PhantomData;
use tokio::sync::mpsc;
use tracing::*;

use kernel::Surroundings;

use crate::{
    proto::{
        AlwaysErrorsServer, Completed, Inbox, Message, Payload, PayloadMessage, Query,
        QueryMessage, Sender, Server, ServerProtocol, SessionKey,
    },
    ExampleAgent, SessionServer,
};

#[derive(Debug)]
enum ChannelMessage {
    Query(Option<Query>),
    Payload(Payload),
    Shutdown,
}

#[allow(dead_code)]
pub struct TokioChannelServer<P> {
    session_key: SessionKey,
    server_tx: mpsc::Sender<ChannelMessage>,
    agent_tx: mpsc::Sender<ChannelMessage>,
    rx_agent: mpsc::Receiver<ChannelMessage>,
    server: ServerProtocol,
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

async fn process_query(
    session_key: &SessionKey,
    query: Option<Query>,
    server_tx: &mpsc::Sender<ChannelMessage>,
    mut server: &mut ServerProtocol,
) -> Result<Completed> {
    fn apply_query(
        server: &mut ServerProtocol,
        message: &Message<Option<Query>>,
        session_server: &dyn Server,
    ) -> Result<Sender<PayloadMessage>> {
        let mut to_agent: Sender<_> = Default::default();
        server.apply(&message, &mut to_agent, session_server)?;

        Ok(to_agent)
    }

    let message = session_key.message(query);
    let to_agent: Sender<_> = if message.body().is_none() {
        apply_query(&mut server, &message, &AlwaysErrorsServer {})?
    } else {
        apply_query(&mut server, &message, &SessionServer::new_for_my_session()?)?
    };

    for sending in to_agent.into_iter() {
        trace!("sending {:?}", sending);
        server_tx
            .send(ChannelMessage::Payload(sending.into_body()))
            .await?;
    }

    Ok(server.completed())
}

impl TokioChannelServer<ExampleAgent> {
    pub async fn new() -> Self {
        let (stopped_tx, rx_stopped) = tokio::sync::oneshot::channel::<bool>();
        let (agent_tx, rx_agent) = tokio::sync::mpsc::channel::<ChannelMessage>(4);
        let (server_tx, mut rx_server) = tokio::sync::mpsc::channel::<ChannelMessage>(4);

        let session_key = SessionKey::new("SessionKey");

        tokio::spawn({
            let session_key = session_key.clone();
            let agent_tx = agent_tx.clone();

            async move {
                let mut agent = ExampleAgent::new();

                // Server is transmitting paylods to us and we're receiving from them.
                while let Some(cm) = rx_server.recv().await {
                    if let ChannelMessage::Payload(payload) = cm {
                        match process_payload(&session_key, payload, &agent_tx, &mut agent).await {
                            Err(e) => warn!("Payload error: {:?}", e),
                            Ok(()) => {}
                        }
                    } else {
                        debug!("{:?}", cm);
                        break;
                    }
                }

                match stopped_tx.send(true) {
                    Err(e) => warn!("Send stopped error: {:?}", e),
                    Ok(_) => {}
                }
            }
        });

        let server = ServerProtocol::new(session_key.clone());

        Self {
            session_key,
            server_tx,
            agent_tx,
            rx_agent,
            server,
            rx_stopped: Some(rx_stopped),
            _marker: Default::default(),
        }
    }

    async fn drive(&mut self) -> Result<()> {
        // Agent is transmitting queries to us and we're receiving from them.
        while let Some(cm) = self.rx_agent.recv().await {
            if let ChannelMessage::Query(query) = cm {
                match process_query(&self.session_key, query, &self.server_tx, &mut self.server)
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

        self.drive().await?;

        Ok(())
    }

    pub async fn have_surroundings(
        &mut self,
        surroundings: &Surroundings,
        _server: &dyn Server,
    ) -> Result<()> {
        self.server_tx
            .send(ChannelMessage::Payload(Payload::Surroundings(
                surroundings.try_into()?,
            )))
            .await?;

        self.drive().await?;

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        debug!("stopping");

        self.server_tx.send(ChannelMessage::Shutdown).await?;

        if let Some(receiver) = self.rx_stopped.take() {
            receiver.await?;
        } else {
            warn!("No rx_stopped in stop? Were we stopped before?");
        }

        info!("stopped");

        Ok(())
    }
}
