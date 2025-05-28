use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::UdpSocket};
use tracing::event;

use crate::{
    action::{Action, ActionError},
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
                perform_unicast(ctx, &self.to, &self.from, &self.buffer).await?;
            }
            SendMode::Broadcast => {
                perform_broadcast(ctx, &self.to, &self.from, &self.buffer).await?;
            }
        }

        Ok(())
    }
}

async fn perform_broadcast(
    ctx: Ctx,
    to: &SocketAddr,
    from: &SocketAddr,
    buffer: &[u8],
) -> Result<(), ActionError> {
    let socket: UdpSocket = UdpSocket::bind(from)
        .await
        .map_err(|_| crate::action::ActionError::SendError("Failed to bind udp socket".into()))?;

    socket
        .set_broadcast(true)
        .map_err(|_| crate::action::ActionError::SendError("Failed to set broadcast".into()))?;

    socket.send_to(buffer, to).await.map_err(|e| {
        event!(
            tracing::Level::ERROR,
            "Error sending broadcast data to {}: {}",
            to,
            e
        );
        crate::action::ActionError::SendError(e.to_string())
    })?;

    ctx.lock().await.send_events.push(SendEvent {
        instant: tokio::time::Instant::now(),
        from: *from,
        to: *to,
        buffer: Vec::from(buffer),
    });

    Ok(())
}

async fn perform_unicast(
    ctx: Ctx,
    to: &SocketAddr,
    from: &SocketAddr,
    buffer: &[u8],
) -> Result<(), ActionError> {
    let ctx = &mut ctx.lock().await;
    let streams = &mut ctx.tcp_streams;

    let stream = streams.get_mut(to).ok_or_else(|| {
        event!(tracing::Level::ERROR, "No stream connected to {}", to);
        ActionError::SendError(format!("No stream connected to {}", to))
    })?;

    stream.write_all(buffer).await.map_err(|e| {
        event!(
            tracing::Level::ERROR,
            "Error writing to stream {}: {}",
            to,
            e
        );
        ActionError::SendError(e.to_string())
    })?;

    ctx.send_events.push(SendEvent {
        instant: tokio::time::Instant::now(),
        from: *from,
        to: *to,
        buffer: Vec::from(buffer),
    });
    event!(tracing::Level::INFO, "Data sent to {}", to);

    Ok(())
}
