use std::collections::HashMap;

use crate::{
    handlers::{WorkflowHandler, WorkflowPayload},
    payloads::{TaskRequestPayload, TaskResponsePayload},
    utils::DriaMessage,
};
use dkn_p2p::{
    libp2p::{
        gossipsub::{Message, MessageId},
        PeerId,
    },
    DriaP2PCommander,
};
use eyre::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const TASK_PRINT_INTERVAL_SECS: u64 = 20;

pub struct DriaMonitorNode {
    pub p2p: DriaP2PCommander,
    pub msg_rx: mpsc::Receiver<(PeerId, MessageId, Message)>,

    // task monitoring
    pub tasks: HashMap<String, TaskRequestPayload<WorkflowPayload>>,
    pub results: HashMap<String, TaskResponsePayload>,
}

impl DriaMonitorNode {
    pub fn new(
        p2p: DriaP2PCommander,
        msg_rx: mpsc::Receiver<(PeerId, MessageId, Message)>,
    ) -> Self {
        Self {
            p2p,
            msg_rx,
            tasks: HashMap::new(),
            results: HashMap::new(),
        }
    }
    pub async fn setup(&self) -> Result<()> {
        self.p2p.subscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.p2p.subscribe(WorkflowHandler::RESPONSE_TOPIC).await?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down monitor");
        self.p2p.unsubscribe(WorkflowHandler::LISTEN_TOPIC).await?;
        self.p2p
            .unsubscribe(WorkflowHandler::RESPONSE_TOPIC)
            .await?;

        self.p2p.shutdown().await?;
        self.msg_rx.close();

        Ok(())
    }

    pub async fn run(&mut self, token: CancellationToken) {
        let mut task_print_interval =
            tokio::time::interval(tokio::time::Duration::from_secs(TASK_PRINT_INTERVAL_SECS));

        loop {
            tokio::select! {
                message = self.msg_rx.recv() => match message {
                    Some(message) => match self.handle_message(message).await {
                        Ok(_) => {}
                        Err(e) => log::error!("Error handling message: {:?}", e),
                    }
                    None => break, // channel closed
                },
                _ = task_print_interval.tick() => {
                    log::info!("Current seen tasks: {:#?}", self.tasks.keys().collect::<Vec<_>>());
                }
                _ = token.cancelled() => break,
            }
        }
    }

    async fn handle_message(
        &mut self,
        (peer_id, message_id, gossipsub_message): (PeerId, MessageId, Message),
    ) -> Result<()> {
        log::info!(
            "Received {} message {} from {}",
            gossipsub_message.topic,
            message_id,
            peer_id
        );

        // accept all message regardless immediately
        self.p2p
            .validate_message(
                &message_id,
                &peer_id,
                dkn_p2p::libp2p::gossipsub::MessageAcceptance::Accept,
            )
            .await?;

        // parse message, ignore signatures
        let message: DriaMessage = serde_json::from_slice(&gossipsub_message.data)?;

        match message.topic.as_str() {
            WorkflowHandler::LISTEN_TOPIC => {
                let payload: TaskRequestPayload<WorkflowPayload> = message.parse_payload(true)?;
                self.tasks.insert(payload.task_id.clone(), payload);
            }
            WorkflowHandler::RESPONSE_TOPIC => {
                let payload: TaskResponsePayload = message.parse_payload(false)?;
                self.results.insert(payload.task_id.clone(), payload);
            }
            _ => { /* ignore */ }
        }
        Ok(())
    }
}
