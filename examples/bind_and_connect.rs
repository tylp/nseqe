use nseseq::Node;
use nseseq::action::Sleep;
use nseseq::protocol::ip::{
    Bind, Connect, ConnectPredicate, MessagesPredicate, ReceivePredicate, Send, SendMode, Wait,
    WaitEvent,
};
use tokio::task::JoinSet;
use tracing::{Instrument, span};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut node_1 = create_node_1();
    let mut node_2 = create_node_2();

    let mut set = JoinSet::new();

    set.spawn(
        async move {
            node_1.start().await;
        }
        .instrument(span!(tracing::Level::INFO, "node_1")),
    );

    set.spawn(
        async move {
            node_2.start().await;
        }
        .instrument(span!(tracing::Level::INFO, "node_2")),
    );

    while let Some(res) = set.join_next().await {
        match res {
            Ok(()) => {
                tracing::info!("Node finished successfully");
            }
            Err(e) => {
                tracing::error!("Node failed: {:?}", e);
            }
        }
    }
}

fn create_node_1() -> Node {
    let mut node = Node::new("test-node-1");
    let bind_action = Bind::new("192.168.1.10:3000".parse().unwrap());
    let sleep_action = Sleep::new(1000);
    let connection_action = Connect::new(
        "192.168.1.10:0".parse().unwrap(),
        "192.168.1.11:4000".parse().unwrap(),
        1000,
    );
    let send_unicast_action = Send::new(
        SendMode::Unicast,
        "192.168.1.10:3000".parse().unwrap(),
        "192.168.1.11:4000".parse().unwrap(),
        vec![1, 2, 3, 4],
    );

    let send_broadcast_action = Send::new(
        SendMode::Broadcast,
        "192.168.1.10:0".parse().unwrap(),
        "192.168.1.255:49999".parse().unwrap(),
        vec![1, 2, 3, 4],
    );

    let wait_connection_action = Wait::new(WaitEvent::Connection(ConnectPredicate::new(
        "192.168.1.11:0".parse().unwrap(),
        "192.168.1.10:3000".parse().unwrap(),
    )));

    let wait_message_action = Wait::new(WaitEvent::Messages(ReceivePredicate::new(vec![
        MessagesPredicate {
            from: "192.168.1.11:0".parse().unwrap(),
            to: "192.168.1.10:3000".parse().unwrap(),
            buffer: vec![1, 1],
        },
    ])));

    node.add_action(bind_action);
    node.add_action(sleep_action.clone());
    node.add_action(send_broadcast_action);
    node.add_action(wait_connection_action);
    node.add_action(wait_message_action);
    node.add_action(connection_action);
    node.add_action(send_unicast_action);
    node.add_action(sleep_action);

    node
}

fn create_node_2() -> Node {
    let mut node = Node::new("test-node-2");
    let bind_action = Bind::new("192.168.1.11:4000".parse().unwrap());
    let sleep_action = Sleep::new(5000);
    let connection_action = Connect::new(
        "192.168.1.11:0".parse().unwrap(),
        "192.168.1.10:3000".parse().unwrap(),
        1000,
    );

    let send_unicast_action = Send::new(
        SendMode::Unicast,
        "192.168.1.11:4000".parse().unwrap(),
        "192.168.1.10:3000".parse().unwrap(),
        vec![1, 1],
    );

    node.add_action(bind_action);
    node.add_action(sleep_action.clone());
    node.add_action(connection_action);
    node.add_action(sleep_action);
    node.add_action(send_unicast_action);

    node
}
