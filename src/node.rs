use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tracing::{event, span};

use crate::action::Action;

pub struct NodeContext {
    pub tcp_streams: Mutex<HashMap<SocketAddr, TcpStream>>,
    pub tcp_listeners: Mutex<HashMap<SocketAddr, TcpListener>>,
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
                tcp_listeners: Mutex::new(HashMap::new()),
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
            let span = span!(tracing::Level::INFO, "Action");
            let _enter = span.enter();
            match action.perform(self.ctx.clone()).await {
                Ok(_) => event!(
                    tracing::Level::INFO,
                    "Action {} performed successfully",
                    action.name()
                ),
                Err(e) => event!(tracing::Level::ERROR, "Error performing action: {}", e),
            }
        }
    }
}
