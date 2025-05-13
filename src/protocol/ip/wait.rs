use std::net::SocketAddr;

use tokio::time::Instant;
use tracing::event;

use crate::{
    action::{Action, ActionError},
    node::{Ctx, ReceiveEvent},
};

#[async_trait::async_trait]
pub trait Predicate {
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError>;
}

#[derive(Debug, PartialEq, Clone)]
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
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError> {
        let instant = Instant::now();
        loop {
            let notifier = {
                let context = ctx.lock().await;

                if context
                    .connect_events
                    .iter()
                    .filter(|e| e.instant > instant) //  filter out old events
                    .any(|e| {
                        // If the from port is 0, we should ignore the from:port in the check
                        if self.from.port() == 0 {
                            event!(
                                tracing::Level::DEBUG,
                                "Checking connection from {} (portless) to {}",
                                self.from.ip(),
                                self.to
                            );
                            e.from.ip() == self.from.ip() && e.to == self.to
                        } else {
                            event!(
                                tracing::Level::DEBUG,
                                "Checking connection from {} to {}",
                                self.from,
                                self.to
                            );
                            e.from == self.from && e.to == self.to
                        }
                    })
                {
                    event!(
                        tracing::Level::DEBUG,
                        "Connection from {} to {} found",
                        self.from,
                        self.to
                    );
                    return Ok(());
                } else {
                    event!(
                        tracing::Level::DEBUG,
                        "Connection from {} to {} not found",
                        self.from,
                        self.to
                    );
                }

                context.connect_notifier.clone()
            };

            // wait to be notified before checking again
            notifier.notified().await;
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
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
                let events: Vec<_> = context
                    .receive_events
                    .iter()
                    .filter(|e| e.instant > instant)
                    .collect();

                let mut idx = 0;
                let received_ordered = self.messages.iter().all(|pred| {
                    if let Some(pos) = events[idx..].iter().position(|evt| pred.matches(evt)) {
                        idx += pos + 1; // advance past the match
                        true
                    } else {
                        false
                    }
                });

                match received_ordered {
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

#[derive(Debug, PartialEq, Clone)]
pub enum WaitEvent {
    Connection(ConnectPredicate),
    Messages(ReceivePredicate),
}

#[derive(Debug, PartialEq, Clone)]
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
