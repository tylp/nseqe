use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{net::TcpStream, sync::Mutex, time::Instant};
use tracing::{event, span};

use crate::action::Action;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReceivedMessage {
    pub instant: Instant,
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub buffer: Vec<u8>,
}

pub type ReceivedMessages = HashMap<SocketAddr, Vec<ReceivedMessage>>;

pub struct NodeContext {
    pub tcp_streams: Mutex<HashMap<SocketAddr, TcpStream>>,
    pub received_messages: Arc<Mutex<ReceivedMessages>>,
}

pub type Ctx = Arc<NodeContext>;

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
            ctx: Arc::new(NodeContext {
                tcp_streams: Mutex::new(HashMap::new()),
                received_messages: Arc::new(Mutex::new(HashMap::new())),
            }),
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
            match action.perform(self.ctx.clone()).await {
                Ok(_) => event!(
                    tracing::Level::INFO,
                    "[OK] Action {} performed successfully",
                    action.name()
                ),
                Err(e) => event!(tracing::Level::ERROR, "Error performing action: {}", e),
            }
        }

        event!(tracing::Level::INFO, "All actions performed");
        event!(tracing::Level::INFO, "Node {} finished", self.name);
        event!(
            tracing::Level::INFO,
            "Received messages: {:?}",
            self.ctx.received_messages.lock().await
        );
    }
}
