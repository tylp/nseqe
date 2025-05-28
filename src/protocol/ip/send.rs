use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::UdpSocket};
use tracing::event;

use crate::{
    action::Action,
    node::{Ctx, SendEvent},
};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum SendMode {
    Unicast,
    Broadcast,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Send {
    mode: SendMode,
    from: SocketAddr,
    to: SocketAddr,
    buffer: Vec<u8>,
}

impl Send {
    /// Creates a new `Send` action.
    pub fn new(mode: SendMode, from: SocketAddr, to: SocketAddr, buffer: Vec<u8>) -> Self {
        Send {
            mode,
            from,
            to,
            buffer,
        }
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
#[typetag::serde]
impl Action for Send {
    fn name(&self) -> String {
        "SEND".into()
    }

    /// Sends data from `from` to `to` using the specified `mode`.
    async fn perform(&self, ctx: Ctx) -> Result<(), crate::action::ActionError> {
        event!(
            tracing::Level::INFO,
            "Sending {} bytes from {} to {} in {:?} mode",
            self.buffer.len(),
            self.from,
            self.to,
            self.mode
        );

        match self.mode {
            SendMode::Unicast => {
                // Check if a stream is already connected to the destination
                let ctx = &mut ctx.lock().await;
                let streams = &mut ctx.tcp_streams;
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
                        ctx.send_events.push(SendEvent {
                            instant: tokio::time::Instant::now(),
                            from: self.from,
                            to: self.to,
                            buffer: self.buffer.clone(),
                        });
                        event!(tracing::Level::INFO, "Data sent to {}", self.to);
                    }
                } else {
                    event!(tracing::Level::ERROR, "No stream connected to {}", self.to);
                }
            }
            SendMode::Broadcast => {
                let socket: UdpSocket = UdpSocket::bind(self.from).await.map_err(|_| {
                    crate::action::ActionError::SendError("Failed to bind udp socket".into())
                })?;

                socket.set_broadcast(true).map_err(|_| {
                    crate::action::ActionError::SendError("Failed to set broadcast".into())
                })?;

                socket.send_to(self.buffer(), self.to).await.map_err(|e| {
                    event!(
                        tracing::Level::ERROR,
                        "Error sending broadcast data to {}: {}",
                        self.to,
                        e
                    );
                    crate::action::ActionError::SendError(e.to_string())
                })?;

                ctx.lock().await.send_events.push(SendEvent {
                    instant: tokio::time::Instant::now(),
                    from: self.from,
                    to: self.to,
                    buffer: self.buffer.clone(),
                });
            }
        }

        Ok(())
    }
}
