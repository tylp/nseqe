use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{
    net::TcpStream,
    sync::{Mutex, Notify},
    time::Instant,
};
use tracing::{event, span};

use crate::action::Action;

#[derive(Debug, Clone)]
pub struct ReceiveEvent {
    pub instant: Instant,
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub buffer: Vec<u8>,
}

pub struct SendEvent {
    pub instant: Instant,
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub buffer: Vec<u8>,
}

pub struct ConnectEvent {
    pub instant: Instant,
    pub from: SocketAddr,
    pub to: SocketAddr,
}

pub struct NodeContext {
    pub tcp_streams: HashMap<SocketAddr, TcpStream>,
    pub receive_events: Vec<ReceiveEvent>,
    pub receive_notifier: Arc<Notify>,
    pub send_events: Vec<SendEvent>,
    pub connect_events: Vec<ConnectEvent>,
    pub connect_notifier: Arc<Notify>,
}

pub type Ctx = Arc<Mutex<NodeContext>>;

pub struct Node {
    name: String,
    actions: Vec<Box<dyn Action>>,
    ctx: Ctx,
}

impl Node {
    pub fn new(name: &str) -> Self {
        Node {
            name: name.to_string(),
            actions: Vec::new(),
            ctx: Arc::new(Mutex::new(NodeContext {
                tcp_streams: HashMap::new(),
                receive_events: Vec::new(),
                receive_notifier: Arc::new(Notify::new()),
                send_events: Vec::new(),
                connect_events: Vec::new(),
                connect_notifier: Arc::new(Notify::new()),
            })),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_action<T>(&mut self, action: T)
    where
        T: Action + 'static,
    {
        self.actions.push(Box::new(action));
    }

    pub async fn start(&mut self) {
        for action in self.actions.drain(..) {
            let span = span!(tracing::Level::INFO, "node");
            let _enter = span.enter();

            event!(
                tracing::Level::INFO,
                "[..] Performing action {}",
                action.name()
            );

            match action.perform(self.ctx.clone()).await {
                Ok(_) => event!(
                    tracing::Level::INFO,
                    "[OK] Action {} performed successfully",
                    action.name()
                ),
                Err(e) => event!(tracing::Level::ERROR, "Error performing action: {:?}", e),
            }
        }

        event!(tracing::Level::INFO, "All actions performed");
        event!(tracing::Level::INFO, "Node {} finished", self.name);
        event!(
            tracing::Level::INFO,
            "Received messages: {:?}",
            self.ctx.lock().await.receive_events
        );
    }
}
