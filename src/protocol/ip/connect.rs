use crate::{
    action::{Action, ActionError},
    node::Ctx,
};
use std::time::Duration;
use tracing::{event, span};

#[derive(Debug, PartialEq, Clone)]
pub struct Connect {
    to: std::net::SocketAddr,
    timeout_ms: u64,
}

impl Connect {
    pub fn new(to: std::net::SocketAddr, timeout_ms: u64) -> Self {
        Connect { to, timeout_ms }
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

        event!(tracing::Level::INFO, "Connecting to {}", self.to);
        let stream = tokio::time::timeout(
            Duration::from_millis(self.timeout_ms),
            tokio::net::TcpStream::connect(self.to),
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
        ctx.tcp_streams.lock().await.insert(self.to, stream);

        Ok(())
    }
}
