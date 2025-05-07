use tracing::{event, span};

use crate::{
    action::{Action, ActionError},
    node::Ctx,
};

pub struct Bind {
    to: std::net::SocketAddr,
}
impl Bind {
    pub fn new(to: std::net::SocketAddr) -> Self {
        Bind { to }
    }

    pub fn to(&self) -> &std::net::SocketAddr {
        &self.to
    }
}
#[async_trait::async_trait]
impl Action for Bind {
    fn name(&self) -> String {
        "BIND".into()
    }

    async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "Binding");
        let _guard = span.enter();

        event!(tracing::Level::INFO, "Binding to {}", self.to);
        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ActionError::BindError)?;

        socket
            .set_reuseaddr(true)
            .map_err(|_| ActionError::BindError)?;
        socket.bind(self.to).map_err(|_| ActionError::BindError)?;

        let listener = socket.listen(1024).map_err(|_| ActionError::BindError)?;

        event!(
            tracing::Level::INFO,
            "TCP server listening on to {}",
            self.to
        );
        ctx.tcp_listeners.lock().await.insert(self.to, listener);

        Ok(())
    }
}
