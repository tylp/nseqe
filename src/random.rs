#[derive(Debug, PartialEq, Clone)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SendMode {
    Unicast,
    Multicast,
    Broadcast,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Connect {
    to: Endpoint,
    timeout_ms: u64,
}

impl Connect {
    pub fn new(to: Endpoint, timeout_ms: u64) -> Self {
        Connect { to, timeout_ms }
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "Connecting");
        let _enter = span.enter();

        let addr = self.to.address().to_string();
        let port = self.to.port();
        let socket_addr = format!("{}:{}", addr, port);

        let stream = tokio::time::timeout(
            Duration::from_millis(self.timeout_ms),
            tokio::net::TcpStream::connect(socket_addr.clone()),
        )
        .await
        .map_err(|_| {
            ActionError::ConnectError(format!(
                "Timeout connecting to {socket_addr} ({}ms)",
                self.timeout_ms
            ))
        })?
        .map_err(|error| {
            ActionError::ConnectError(format!(
                "Error while trying to connect to {socket_addr} ({error})"
            ))
        })?;

        event!(tracing::Level::INFO, "Connected to {}", self.to.address());
        ctx.tcp_streams.lock().await.insert(self.to.clone(), stream);

        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Disconnect {
    target: Endpoint,
}

impl Disconnect {
    pub fn new(target: Endpoint) -> Self {
        Disconnect { target }
    }

    pub fn target(&self) -> &Endpoint {
        &self.target
    }

    pub async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "Disconnecting");
        let _enter = span.enter();

        let mut streams = ctx.tcp_streams.lock().await;
        if let Some(mut stream) = streams.remove(&self.target) {
            event!(
                tracing::Level::INFO,
                "Disconnecting from {}",
                self.target.address()
            );
            stream
                .shutdown()
                .await
                .map_err(|_| ActionError::DisconnectError)?;
        } else {
            event!(
                tracing::Level::ERROR,
                "No stream found for {}",
                self.target.address()
            );
            return Err(ActionError::DisconnectError);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Bind {
    interface: Endpoint,
    protocol: Protocol,
}

impl Bind {
    pub fn new(interface: Endpoint, protocol: Protocol) -> Self {
        Bind {
            interface,
            protocol,
        }
    }

    pub fn interface(&self) -> &Endpoint {
        &self.interface
    }

    pub fn protocol(&self) -> &Protocol {
        &self.protocol
    }

    pub async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        let span = span!(tracing::Level::INFO, "Binding");
        let _guard = span.enter();

        let addr = self.interface.address().to_string();
        let port = self.interface.port();
        let socket_addr = format!("{}:{}", addr, port)
            .parse()
            .map_err(|_| ActionError::BindError)?;

        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ActionError::BindError)?;

        socket
            .set_reuseaddr(true)
            .map_err(|_| ActionError::BindError)?;

        socket
            .bind(socket_addr)
            .map_err(|_| ActionError::BindError)?;

        let listener = socket.listen(1024).map_err(|_| ActionError::BindError)?;

        event!(
            tracing::Level::INFO,
            "Binding tcp server to {}",
            socket_addr
        );

        ctx.tcp_listeners
            .lock()
            .await
            .insert(self.interface.clone(), listener);

        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Send {
    mode: SendMode,
    from: Endpoint,
    to: Vec<Endpoint>,
    buffer: Vec<u8>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Sleep {
    duration_ms: u64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Wait {
    event: WaitEvent,
}

impl Action {
    pub async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        match self {
            Action::Connect(connect) => connect.perform(ctx).await,
            Action::Disconnect(disconnect) => disconnect.perform(ctx).await,
            Action::Bind(bind) => bind.perform(ctx).await,
            Action::Send(send) => todo!(),
            Action::Sleep(sleep) => todo!(),
            Action::Wait(wait) => todo!(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ConnectionSpec {
    from: Endpoint,
    to: Endpoint,
    protocol: Protocol,
}

impl ConnectionSpec {
    pub fn new(from: Endpoint, to: Endpoint, protocol: Protocol) -> Self {
        ConnectionSpec { from, to, protocol }
    }

    pub fn from(&self) -> &Endpoint {
        &self.from
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
    }

    pub fn protocol(&self) -> &Protocol {
        &self.protocol
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MessageMatch {
    from: Endpoint,
    to: Endpoint,
    protocol: Protocol,
    buffer: Vec<u8>,
}

impl MessageMatch {
    pub fn new(from: Endpoint, to: Endpoint, protocol: Protocol, buffer: Vec<u8>) -> Self {
        MessageMatch {
            from,
            to,
            protocol,
            buffer,
        }
    }

    pub fn from(&self) -> &Endpoint {
        &self.from
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
    }

    pub fn protocol(&self) -> &Protocol {
        &self.protocol
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum OrderType {
    Unordered,
    Ordered,
}

#[derive(Debug, PartialEq, Clone)]
pub enum WaitEvent {
    Connection(Vec<ConnectionSpec>),
    Messages {
        order: OrderType,
        list: Vec<MessageMatch>,
        timeout_ms: Option<u64>,
    },
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use tokio::sync::Mutex;

    use crate::node::NodeContext;

    use super::*;

    #[tokio::test]
    async fn test_connect() {
        let ctx = Arc::new(NodeContext {
            tcp_streams: Mutex::new(HashMap::new()),
            tcp_listeners: Mutex::new(HashMap::new()),
        });

        let endpoint = Endpoint::new("127.0.0.1", 3000);
        let connect = Connect::new(endpoint.clone(), 1000);
        let result: Result<(), ActionError> = connect.perform(ctx.clone()).await;

        assert!(result.is_err());
        assert_eq!(
            result,
            Err(ActionError::ConnectError(
                "Error while trying to connect to 127.0.0.1:3000".into()
            ))
        )
    }
}
