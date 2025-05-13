use std::{net::SocketAddr, sync::Arc};
use tokio::net::UdpSocket;

use tokio::io::AsyncReadExt;
use tracing::{Instrument, event, span};

use crate::node::ConnectEvent;
use crate::{
    action::{Action, ActionError},
    node::{Ctx, ReceiveEvent},
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

        event!(tracing::Level::INFO, "Binding TCP socket to {}", self.to);
        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ActionError::BindError)?;

        socket
            .set_reuseaddr(true)
            .map_err(|_| ActionError::BindError)?;
        socket.bind(self.to).map_err(|_| ActionError::BindError)?;

        let listener = socket.listen(1024).map_err(|_| ActionError::BindError)?;
        let to_clone = self.to;

        // Accept incomming tcp connections
        let ctx_clone = Arc::clone(&ctx);
        tokio::spawn(async move {
            accept_tcp(listener, to_clone, ctx_clone).await;
        });

        // Accept incoming udp messages
        tokio::spawn(async move {
            accept_udp(to_clone, ctx).await;
        });

        event!(
            tracing::Level::DEBUG,
            "Started listening task server on {}",
            self.to
        );

        Ok(())
    }
}

async fn accept_udp(to: SocketAddr, ctx: Ctx) {
    let ip = to.ip();
    let port = 49999;

    event!(
        tracing::Level::DEBUG,
        "Binding UDP socket to {}:{}",
        ip,
        port
    );
    let udp_socket = UdpSocket::bind(format!("{}:{}", ip, port))
        .await
        .expect("Failed to bind UDP socket");

    let mut buf = vec![0; 1024];
    loop {
        let span = span!(tracing::Level::INFO, "accept-udp");
        let _guard = span.enter();
        let (len, addr) = udp_socket.recv_from(&mut buf).await.unwrap();
        event!(
            tracing::Level::INFO,
            "Received {} UDP bytes from {}",
            len,
            addr
        );

        let received_message = ReceiveEvent {
            instant: tokio::time::Instant::now(),
            from: addr,
            to: udp_socket.local_addr().unwrap(),
            buffer: buf[..len].to_vec(),
        };

        ctx.lock().await.receive_events.push(received_message);
        ctx.lock().await.receive_notifier.notify_waiters();
    }
}

/// Listens for incoming connections on the given listener
/// and processes each connection in a separate task.
async fn accept_tcp(listener: tokio::net::TcpListener, addr: std::net::SocketAddr, ctx: Ctx) {
    let binded_addr = addr.to_string();
    let _ = tokio::spawn(async move {
        loop {
            let span = span!(tracing::Level::INFO, "accept-tcp");
            let _guard = span.enter();
            match listener.accept().await {
                Ok((socket, addr)) => {
                    event!(tracing::Level::INFO, "Accepted connection from {}", addr);

                    // Store the event in the context and signal every task waiting for it
                    ctx.lock().await.connect_events.push(ConnectEvent {
                        instant: tokio::time::Instant::now(),
                        from: addr,
                        to: socket.local_addr().unwrap(),
                    });
                    ctx.lock().await.connect_notifier.notify_waiters();

                    let ctx_clone = ctx.clone();
                    tokio::spawn(async move {
                        process_socket(socket, ctx_clone).await;
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
async fn process_socket(mut socket: tokio::net::TcpStream, ctx: Ctx) {
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
                let addr = socket.peer_addr().unwrap();
                event!(tracing::Level::INFO, "Read {} TCP bytes from {}", n, addr);

                let received_message = ReceiveEvent {
                    instant: tokio::time::Instant::now(),
                    from: addr,
                    to: socket.local_addr().unwrap(),
                    buffer: buf[..n].to_vec(),
                };

                ctx.lock().await.receive_events.push(received_message);
                ctx.lock().await.receive_notifier.notify_waiters();
            }
            Err(e) => {
                event!(tracing::Level::ERROR, "Error reading from socket: {}", e);
                break;
            }
        }
    }
}
