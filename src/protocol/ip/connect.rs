use crate::{
    action::{Action, ActionError},
    node::{ConnectEvent, Ctx},
};
use std::time::Duration;
use tokio::net::TcpSocket;
use tracing::{event, span};

#[derive(Debug, PartialEq, Clone)]
pub struct Connect {
    from: std::net::SocketAddr,
    to: std::net::SocketAddr,
    timeout_ms: u64,
}

impl Connect {
    pub fn new(from: std::net::SocketAddr, to: std::net::SocketAddr, timeout_ms: u64) -> Self {
        Connect {
            from,
            to,
            timeout_ms,
        }
    }

    pub fn from(&self) -> &std::net::SocketAddr {
        &self.from
    }

    pub fn to(&self) -> &std::net::SocketAddr {
        &self.to
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[async_trait::async_trait]
impl Action for Connect {
    fn name(&self) -> String {
        "CONNECT".into()
    }

    async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "connect");
        let _enter = span.enter();

        event!(
            tracing::Level::INFO,
            "Connecting from {} to {}",
            self.from,
            self.to
        );

        let socket = TcpSocket::new_v4().map_err(|error| {
            ActionError::ConnectError(format!("Error creating socket for {} ({})", self.to, error))
        })?;

        socket.bind(self.from).map_err(|error| {
            ActionError::ConnectError(format!("Error binding socket to {} ({})", self.from, error))
        })?;

        let stream = tokio::time::timeout(
            Duration::from_millis(self.timeout_ms),
            socket.connect(self.to),
        )
        .await
        .map_err(|_| {
            ActionError::ConnectError(format!(
                "Timeout connecting to {} ({}ms)",
                self.to, self.timeout_ms
            ))
        })?
        .map_err(|error| {
            ActionError::ConnectError(format!(
                "Error while trying to connect to {} ({})",
                self.to, error
            ))
        })?;

        event!(tracing::Level::INFO, "Connected to {}", self.to);
        ctx.lock().await.tcp_streams.insert(self.to, stream);

        let connect_event = ConnectEvent {
            instant: tokio::time::Instant::now(),
            from: self.from,
            to: self.to,
        };

        // Store the connect event in the context and signal every task waiting for it
        ctx.lock().await.connect_events.push(connect_event);
        ctx.lock().await.connect_notifier.notify_waiters();

        Ok(())
    }
}
