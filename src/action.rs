use crate::node::Ctx;
use thiserror::Error;

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
