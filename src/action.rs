use crate::node::Ctx;
use thiserror::Error;
use tracing::{event, span};

#[async_trait::async_trait]
pub trait Action: Send + Sync {
    fn name(&self) -> String;
    async fn perform(&self, ctx: Ctx) -> Result<(), ActionError>;
}

#[derive(Debug, PartialEq, Clone, Error)]
pub enum ActionError {
    #[error("Connection error")]
    ConnectError(String),
    #[error("Disconnection error")]
    DisconnectError,
    #[error("Bind error")]
    BindError,
    #[error("Send error")]
    SendError,
    #[error("Sleep error")]
    SleepError,
    #[error("Wait error")]
    WaitError,
}

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
impl Action for Sleep {
    fn name(&self) -> String {
        "SLEEP".into()
    }

    async fn perform(&self, _ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "sleeping");
        let _enter = span.enter();

        event!(tracing::Level::INFO, "Sleeping for {}ms", self.duration_ms);
        tokio::time::sleep(tokio::time::Duration::from_millis(self.duration_ms)).await;

        event!(tracing::Level::INFO, "Woke up after {}ms", self.duration_ms);
        Ok(())
    }
}
