use tokio::io::AsyncReadExt;
use tracing::{Instrument, event, span};

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
        let span = span!(tracing::Level::INFO, "binding");
        let _guard = span.enter();

        event!(tracing::Level::INFO, "Binding to {}", self.to);
        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ActionError::BindError)?;

        socket
            .set_reuseaddr(true)
            .map_err(|_| ActionError::BindError)?;
        socket.bind(self.to).map_err(|_| ActionError::BindError)?;

        let listener = socket.listen(1024).map_err(|_| ActionError::BindError)?;
        let to_clone = self.to;

        let bind_handle = tokio::spawn(async move {
            listen(listener, to_clone).await;
        });

        ctx.bind_tasks.lock().await.insert(self.to, bind_handle);

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
async fn listen(listener: tokio::net::TcpListener, addr: std::net::SocketAddr) {
    let binded_addr = addr.to_string();
    let _ = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    event!(tracing::Level::INFO, "Accepted connection from {}", addr);
                    tokio::spawn(async move {
                        process_socket(socket).await;
                    });
                    event!(tracing::Level::INFO, "Connection closed from {}", addr);
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
async fn process_socket(mut socket: tokio::net::TcpStream) {
    let mut buf = vec![0; 1024];
    loop {
        let span = span!(tracing::Level::INFO, "socket-read");
        let _guard = span.enter();

        match socket.read(&mut buf).await {
            Ok(0) => {
                event!(tracing::Level::INFO, "Connection closed");
                break;
            }
            Ok(n) => {
                event!(tracing::Level::INFO, "Read {} bytes", n);
                // Process the data
            }
            Err(e) => {
                event!(tracing::Level::ERROR, "Error reading from socket: {}", e);
                break;
            }
        }
    }
}
