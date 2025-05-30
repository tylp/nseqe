use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::event;

use crate::{
    action::{Action, ActionError},
    node::{ConnectEvent, Ctx, ReceiveEvent},
};

#[async_trait::async_trait]
pub trait Predicate {
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError>;
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ConnectPredicate {
    pub from: SocketAddr,
    pub to: SocketAddr,
}

impl ConnectPredicate {
    pub fn new(from: SocketAddr, to: SocketAddr) -> Self {
        ConnectPredicate { from, to }
    }

    pub fn from(&self) -> &SocketAddr {
        &self.from
    }

    pub fn to(&self) -> &SocketAddr {
        &self.to
    }
}

#[async_trait::async_trait]
impl Predicate for ConnectPredicate {
    /// Checks if a connection event from `from` to `to` exists in the context.
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError> {
        let instant = Instant::now();
        loop {
            let notifier = {
                let context = ctx.lock().await;
                let connect_events = &context.connect_events;

                match connect_match(self.from, self.to, instant, connect_events) {
                    true => {
                        event!(
                            tracing::Level::DEBUG,
                            "Connection from {} to {} found",
                            self.from,
                            self.to
                        );
                        return Ok(());
                    }
                    false => {
                        event!(
                            tracing::Level::DEBUG,
                            "Connection from {} to {} not found",
                            self.from,
                            self.to
                        );
                    }
                }

                context.connect_notifier.clone()
            };

            // wait to be notified before checking again
            notifier.notified().await;
        }
    }
}

fn connect_match(
    expected_from: SocketAddr,
    expected_to: SocketAddr,
    expected_instant: Instant,
    connect_events: &[ConnectEvent],
) -> bool {
    connect_events
        .iter()
        .filter(|e| e.instant > expected_instant) //  filter out old events
        .any(|e| {
            // If the from port is 0, we should ignore the from:port in the check
            if expected_from.port() == 0 {
                event!(
                    tracing::Level::DEBUG,
                    "Checking connection from {} (portless) to {}",
                    expected_from.ip(),
                    expected_to
                );
                e.from.ip() == expected_from.ip() && e.to == expected_to
            } else {
                event!(
                    tracing::Level::DEBUG,
                    "Checking connection from {} to {}",
                    expected_from,
                    expected_to
                );
                e.from == expected_from && e.to == expected_to
            }
        })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct MessagesPredicate {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub buffer: Vec<u8>,
}

impl MessagesPredicate {
    pub fn matches(&self, event: &ReceiveEvent) -> bool {
        if self.from.port() == 0 {
            event.from.ip() == self.from.ip() && event.to == self.to && event.buffer == self.buffer
        } else {
            event.from == self.from && event.to == self.to && event.buffer == self.buffer
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ReceivePredicate {
    pub messages: Vec<MessagesPredicate>,
}

impl ReceivePredicate {
    pub fn new(messages: Vec<MessagesPredicate>) -> Self {
        ReceivePredicate { messages }
    }

    pub fn messages(&self) -> &[MessagesPredicate] {
        &self.messages
    }
}

#[async_trait::async_trait]
impl Predicate for ReceivePredicate {
    /// Checks if the specified messages have been received in the context.
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError> {
        let instant = Instant::now();
        loop {
            event!(
                tracing::Level::DEBUG,
                "Checking receive predicate for messages {:?}",
                self.messages
            );

            let notifier = {
                let context = ctx.lock().await;
                let events: Vec<&ReceiveEvent> = context
                    .receive_events
                    .iter()
                    .filter(|e| e.instant > instant)
                    .collect();

                let expected_messages = &self.messages;

                match receive_exact_match(&events, expected_messages) {
                    true => {
                        event!(
                            tracing::Level::DEBUG,
                            "Receive predicate found for messages {:?}",
                            self.messages
                        );
                        return Ok(());
                    }
                    false => {
                        event!(
                            tracing::Level::DEBUG,
                            "Receive predicate not found for messages {:?}",
                            self.messages
                        );
                        context.receive_notifier.clone()
                    }
                }
            };

            // wait to be notified before checking again
            notifier.notified().await;
        }
    }
}

fn receive_exact_match(events: &[&ReceiveEvent], expected_messages: &[MessagesPredicate]) -> bool {
    let mut idx = 0;
    expected_messages.iter().all(|pred| {
        if let Some(pos) = events[idx..].iter().position(|evt| pred.matches(evt)) {
            idx += pos + 1; // advance past the match
            true
        } else {
            false
        }
    })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum WaitEvent {
    Connection(ConnectPredicate),
    Messages(ReceivePredicate),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Wait {
    event: WaitEvent,
}

impl Wait {
    pub fn new(event: WaitEvent) -> Self {
        Wait { event }
    }

    pub fn event(&self) -> &WaitEvent {
        &self.event
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Action for Wait {
    fn name(&self) -> String {
        "WAIT".into()
    }

    async fn perform(&self, ctx: Ctx) -> Result<(), ActionError> {
        match &self.event {
            WaitEvent::Connection(predicate) => predicate.check(ctx).await,
            WaitEvent::Messages(predicate) => predicate.check(ctx).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use tokio::time::Instant;

    use crate::{
        ReceiveEvent,
        node::ConnectEvent,
        protocol::ip::wait::{connect_match, receive_exact_match},
    };

    use super::MessagesPredicate;

    #[test]
    fn test_receive_exact_match() {
        let event1 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let event2 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:30000".parse().unwrap(),
            to: "127.0.0.1:30000".parse().unwrap(),
            buffer: vec![4, 5, 6],
        };

        let predicate1 = MessagesPredicate {
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let predicate2 = MessagesPredicate {
            from: "127.0.0.1:30000".parse().unwrap(),
            to: "127.0.0.1:30000".parse().unwrap(),
            buffer: vec![4, 5, 6],
        };

        let received_events: Vec<&ReceiveEvent> = vec![&event1, &event2];
        let expected_messages: Vec<MessagesPredicate> = vec![predicate1, predicate2];

        assert!(receive_exact_match(&received_events, &expected_messages));
    }

    #[test]
    fn test_receive_exact_match_different_length() {
        let event1 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let predicate1 = MessagesPredicate {
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let predicate2 = MessagesPredicate {
            from: "127.0.0.1:30000".parse().unwrap(),
            to: "127.0.0.1:30000".parse().unwrap(),
            buffer: vec![4, 5, 6],
        };

        let received_events: Vec<&ReceiveEvent> = vec![&event1];
        let expected_messages: Vec<MessagesPredicate> = vec![predicate1, predicate2];

        assert!(!receive_exact_match(&received_events, &expected_messages));
    }

    #[test]
    fn test_receive_exact_match_different_buffer() {
        let event1 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let predicate1 = MessagesPredicate {
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![],
        };

        let received_events: Vec<&ReceiveEvent> = vec![&event1];
        let expected_messages: Vec<MessagesPredicate> = vec![predicate1];

        assert!(!receive_exact_match(&received_events, &expected_messages));
    }

    #[test]
    fn test_receive_exact_match_different_order() {
        let event1 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let event2 = ReceiveEvent {
            instant: Instant::now(),
            from: "127.0.0.1:30000".parse().unwrap(),
            to: "127.0.0.1:30000".parse().unwrap(),
            buffer: vec![4, 5, 6],
        };

        let predicate1 = MessagesPredicate {
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:3000".parse().unwrap(),
            buffer: vec![1, 2, 3],
        };

        let predicate2 = MessagesPredicate {
            from: "127.0.0.1:30000".parse().unwrap(),
            to: "127.0.0.1:30000".parse().unwrap(),
            buffer: vec![4, 5, 6],
        };

        let received_events: Vec<&ReceiveEvent> = vec![&event2, &event1];
        let expected_messages: Vec<MessagesPredicate> = vec![predicate1, predicate2];

        assert!(!receive_exact_match(&received_events, &expected_messages));
    }

    #[test]
    fn test_connect_match() {
        let expected_from: SocketAddr = "127.0.0.1:3000".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_different_from() {
        let expected_from: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(!connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_different_to() {
        let expected_from: SocketAddr = "127.0.0.1:3000".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(!connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_portless() {
        let expected_from: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_portless_different_from() {
        let expected_from: SocketAddr = "127.0.0.100:0".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(!connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_portless_different_to() {
        let expected_from: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.100:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant + Duration::from_millis(1);

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(!connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }

    #[test]
    fn test_connect_match_same_instant() {
        let expected_from: SocketAddr = "127.0.0.1:3000".parse().unwrap();
        let expected_to: SocketAddr = "127.0.0.1:4000".parse().unwrap();
        let expected_instant = Instant::now();

        let event_instant = expected_instant;

        let connect_event = ConnectEvent {
            instant: event_instant,
            from: "127.0.0.1:3000".parse().unwrap(),
            to: "127.0.0.1:4000".parse().unwrap(),
        };

        assert!(!connect_match(
            expected_from,
            expected_to,
            expected_instant,
            &[connect_event],
        ));
    }
}
