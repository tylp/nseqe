use nse::Node;
use nse::action::Sleep;
use nse::protocol::ip::{Bind, Connect, Send, SendMode};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut node = Node::new("test_node");
    let bind_action = Bind::new("127.0.0.1:3000".parse().unwrap());
    let sleep_action = Sleep::new(1000);
    let connection_action = Connect::new(
        "127.0.0.1:4000".parse().unwrap(),
        "127.0.0.1:3000".parse().unwrap(),
        1000,
    );
    let send_unicast_action = Send::new(
        SendMode::Unicast,
        "127.0.0.1:4000".parse().unwrap(),
        "127.0.0.1:3000".parse().unwrap(),
        vec![1, 2, 3, 4],
    );

    let send_broadcast_action = Send::new(
        SendMode::Broadcast,
        "127.0.0.1:0".parse().unwrap(),
        "127.0.0.255:0".parse().unwrap(),
        vec![1, 2, 3, 4],
    );

    node.add_action(bind_action);
    node.add_action(sleep_action.clone());
    node.add_action(connection_action);
    node.add_action(send_broadcast_action);
    node.add_action(send_unicast_action.clone());
    node.add_action(send_unicast_action.clone());
    node.add_action(send_unicast_action.clone());
    node.add_action(send_unicast_action);
    node.add_action(sleep_action);
    node.start().await;
}
