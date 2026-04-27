use std::sync::Arc;

use async_trait::async_trait;
use garudust_agent::Agent;
use garudust_core::{
    platform::{MessageHandler, PlatformAdapter},
    tool::CommandApprover,
    types::{InboundMessage, OutboundMessage},
};

use crate::sessions::SessionRegistry;

/// Routes inbound platform messages to an agent and sends the reply back.
pub struct GatewayHandler {
    agent: Arc<Agent>,
    platform: Arc<dyn PlatformAdapter>,
    sessions: Arc<SessionRegistry>,
    approver: Arc<dyn CommandApprover>,
}

impl GatewayHandler {
    pub fn new(
        agent: Arc<Agent>,
        platform: Arc<dyn PlatformAdapter>,
        sessions: Arc<SessionRegistry>,
        approver: Arc<dyn CommandApprover>,
    ) -> Self {
        Self {
            agent,
            platform,
            sessions,
            approver,
        }
    }
}

#[async_trait]
impl MessageHandler for GatewayHandler {
    async fn handle(&self, msg: InboundMessage) -> Result<(), anyhow::Error> {
        self.sessions
            .touch(&msg.session_key, &msg.channel.platform, &msg.user_id)
            .await;

        let channel = msg.channel.clone();
        let agent = self.agent.clone();
        let platform = self.platform.clone();
        let approver = self.approver.clone();
        let task = msg.text.clone();
        let platform_name = msg.channel.platform.clone();

        tokio::spawn(async move {
            match agent.run(&task, approver, &platform_name).await {
                Ok(result) => {
                    let reply = OutboundMessage::markdown(result.output);
                    if let Err(e) = platform.send_message(&channel, reply).await {
                        tracing::error!("send_message failed: {e}");
                    }
                }
                Err(e) => {
                    let reply = OutboundMessage::text(format!("Error: {e}"));
                    let _ = platform.send_message(&channel, reply).await;
                }
            }
        });

        Ok(())
    }
}
