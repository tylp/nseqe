use nse::Node;
use nse::action::Sleep;
use nse::protocol::ip::{Bind, Connect};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut node = Node::new("test_node");
    let bind_action = Bind::new("127.0.0.1:3000".parse().unwrap());
    let sleep_action = Sleep::new(1000);
    let connection_action = Connect::new("127.0.0.1:3000".parse().unwrap(), 1000);

    node.add_action(bind_action);
    node.add_action(sleep_action);
    node.add_action(connection_action);
    node.start().await;
}
