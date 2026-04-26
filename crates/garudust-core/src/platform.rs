use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;

use crate::{
    error::PlatformError,
    types::{ChannelId, InboundMessage, OutboundMessage},
};

#[async_trait]
pub trait PlatformAdapter: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), PlatformError>;

    async fn send_message(
        &self,
        channel: &ChannelId,
        message: OutboundMessage,
    ) -> Result<(), PlatformError>;

    async fn send_stream(
        &self,
        channel: &ChannelId,
        stream: Pin<Box<dyn Stream<Item = String> + Send>>,
    ) -> Result<(), PlatformError>;
}

#[async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    async fn handle(&self, msg: InboundMessage) -> Result<(), anyhow::Error>;
}
