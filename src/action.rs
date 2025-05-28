use crate::node::Ctx;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::event;

/// Action trait defines the interface for actions that can be performed in the system.
#[async_trait::async_trait]
#[typetag::serde]
pub trait Action: Send + Sync {
    fn name(&self) -> String;
    async fn perform(&self, ctx: Ctx) -> Result<(), ActionError>;
}

/// ActionError defines the possible errors that can occur during action execution.
#[derive(Debug, PartialEq, Clone, Error)]
pub enum ActionError {
    #[error("Connection error")]
    ConnectError(String),
    #[error("Disconnection error")]
    DisconnectError,
    #[error("Bind error")]
    BindError,
    #[error("Send error")]
    SendError(String),
    #[error("Sleep error")]
    SleepError,
    #[error("Wait error")]
    WaitError,
}

/// Sleep action represents a delay in the execution of the action sequence.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Sleep {
    duration_ms: u64,
}

impl Sleep {
    pub fn new(duration_ms: u64) -> Self {
        Sleep { duration_ms }
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Action for Sleep {
    fn name(&self) -> String {
        "SLEEP".into()
    }

    /// Performs the sleep action by waiting for the specified duration.
    async fn perform(&self, _ctx: Ctx) -> Result<(), ActionError> {
        event!(tracing::Level::INFO, "Sleeping for {}ms", self.duration_ms);
        tokio::time::sleep(tokio::time::Duration::from_millis(self.duration_ms)).await;

        Ok(())
    }
}
