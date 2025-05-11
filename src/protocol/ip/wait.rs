use std::net::SocketAddr;

use tokio::time::Instant;
use tracing::{event, span};

use crate::{
    action::{Action, ActionError},
    node::{ConnectEvent, Ctx, ReceiveEvent},
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
                let guard = ctx.lock().await;

                if guard
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

                guard.connect_notifier.clone()
            };

            // wait to be notified before checking again
            notifier.notified().await;
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ReceivePredicate {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub messages: Vec<ReceiveEvent>,
}

impl ReceivePredicate {
    pub fn new(from: SocketAddr, to: SocketAddr, messages: Vec<ReceiveEvent>) -> Self {
        ReceivePredicate { from, to, messages }
    }

    pub fn from(&self) -> &SocketAddr {
        &self.from
    }

    pub fn to(&self) -> &SocketAddr {
        &self.to
    }

    pub fn messages(&self) -> &[ReceiveEvent] {
        &self.messages
    }
}

#[async_trait::async_trait]
impl Predicate for ReceivePredicate {
    async fn check(&self, ctx: Ctx) -> Result<(), ActionError> {
        loop {
            let notifier = {
                let guard = ctx.lock().await;
                if let Some(received) = guard.receive_events.get(&self.from) {
                    if received
                        .iter()
                        .any(|e| e.from == self.from && e.to == self.to)
                    {
                        return Ok(());
                    }
                }

                guard.connect_notifier.clone()
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
            WaitEvent::Connection(predicate) => {
                let span = span!(tracing::Level::INFO, "wait-connection");
                let _enter = span.enter();

                event!(
                    tracing::Level::INFO,
                    "Waiting for connection from {} to {}",
                    predicate.from,
                    predicate.to
                );

                predicate.check(ctx).await
            }
            WaitEvent::Messages(predicate) => {
                let span = span!(tracing::Level::INFO, "wait-messages");
                let _enter = span.enter();

                event!(
                    tracing::Level::INFO,
                    "Waiting for messages from {}",
                    predicate.messages.len()
                );

                unimplemented!();
            }
        }
    }
}
