#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut node = nse::Node::new("test_node");
    let bind_action = nse::protocol::ip::Bind::new("127.0.0.1:3000".parse().unwrap());
    let connection_action =
        nse::protocol::ip::Connect::new("127.0.0.1:3000".parse().unwrap(), 1000);

    node.add_action(bind_action);
    node.add_action(connection_action);
    node.start().await;
}
