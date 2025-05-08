use std::net::SocketAddr;

use tokio::io::AsyncWriteExt;
use tracing::{event, span};

use crate::action::Action;

#[derive(Debug, PartialEq, Clone)]
pub struct Send {
    from: SocketAddr,
    to: SocketAddr,
    buffer: Vec<u8>,
}

impl Send {
    pub fn new(from: SocketAddr, to: SocketAddr, buffer: Vec<u8>) -> Self {
        Send { from, to, buffer }
    }

    pub fn from(&self) -> &SocketAddr {
        &self.from
    }

    pub fn to(&self) -> &SocketAddr {
        &self.to
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

#[async_trait::async_trait]
impl Action for Send {
    fn name(&self) -> String {
        "SEND".into()
    }

    async fn perform(&self, ctx: crate::node::Ctx) -> Result<(), crate::action::ActionError> {
        let span = span!(tracing::Level::INFO, "send");
        let _enter = span.enter();

        event!(
            tracing::Level::INFO,
            "Sending {} bytes from {} to {}",
            self.buffer.len(),
            self.from,
            self.to
        );

        // Check if a stream is already connected to the destination
        let mut streams = ctx.tcp_streams.lock().await;
        if let Some(stream) = streams.get_mut(&self.to) {
            // Send the data
            if let Err(e) = stream.write_all(&self.buffer).await {
                event!(
                    tracing::Level::ERROR,
                    "Error sending data to {}: {}",
                    self.to,
                    e
                );
            } else {
                event!(tracing::Level::INFO, "Data sent to {}", self.to);
            }
        } else {
            event!(tracing::Level::ERROR, "No stream connected to {}", self.to);
        }
        Ok(())
    }
}
