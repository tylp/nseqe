use std::sync::Arc;
use tokio::sync::Mutex;

use tokio::io::AsyncReadExt;
use tracing::{Instrument, event, span};

use crate::{
    action::{Action, ActionError},
    node::{Ctx, ReceivedMessage, ReceivedMessages},
};

#[derive(Debug, PartialEq, Clone)]
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
        let span = span!(tracing::Level::INFO, "bind");
        let _guard = span.enter();

        event!(tracing::Level::INFO, "Binding to {}", self.to);
        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ActionError::BindError)?;

        socket
            .set_reuseaddr(true)
            .map_err(|_| ActionError::BindError)?;
        socket.bind(self.to).map_err(|_| ActionError::BindError)?;

        let listener = socket.listen(1024).map_err(|_| ActionError::BindError)?;
        let to_clone = self.to;

        let _bind_handle = tokio::spawn(async move {
            accept(listener, to_clone, ctx).await;
        });

        event!(
            tracing::Level::INFO,
            "Started listening task server on {}",
            self.to
        );

        Ok(())
    }
}

/// Listens for incoming connections on the given listener
/// and processes each connection in a separate task.
async fn accept(listener: tokio::net::TcpListener, addr: std::net::SocketAddr, ctx: Ctx) {
    let binded_addr = addr.to_string();
    let received_messages = ctx.received_messages.clone();
    let _ = tokio::spawn(async move {
        loop {
            let span = span!(tracing::Level::INFO, "accept");
            let _guard = span.enter();
            match listener.accept().await {
                Ok((socket, addr)) => {
                    event!(tracing::Level::INFO, "Accepted connection from {}", addr);
                    let received_messages = received_messages.clone();
                    tokio::spawn(async move {
                        process_socket(socket, received_messages).await;
                    });
                }
                Err(e) => {
                    event!(tracing::Level::ERROR, "Error accepting connection: {}", e);
                }
            }
        }
    })
    .instrument(tracing::info_span!("listen_task", addr = %binded_addr))
    .await;
}

/// Processes the incoming socket connection.
/// Reads data from the socket and handles it accordingly.
async fn process_socket(
    mut socket: tokio::net::TcpStream,
    received_messages: Arc<Mutex<ReceivedMessages>>,
) {
    let mut buf = vec![0; 1024];
    loop {
        let span = span!(tracing::Level::INFO, "process-socket");
        let _guard = span.enter();

        match socket.read(&mut buf).await {
            Ok(0) => {
                event!(tracing::Level::INFO, "Buffer is empty, closing connection");
                break;
            }
            Ok(n) => {
                event!(tracing::Level::INFO, "Read {} bytes", n);
                let mut messages = received_messages.lock().await;
                let addr = socket.peer_addr().unwrap();

                let received_message = ReceivedMessage {
                    instant: tokio::time::Instant::now(),
                    from: addr,
                    to: socket.local_addr().unwrap(),
                    buffer: buf[..n].to_vec(),
                };

                messages
                    .entry(addr)
                    .or_insert_with(Vec::new)
                    .push(received_message);
            }
            Err(e) => {
                event!(tracing::Level::ERROR, "Error reading from socket: {}", e);
                break;
            }
        }
    }
}
