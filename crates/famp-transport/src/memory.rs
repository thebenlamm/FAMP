//! In-process byte transport for same-process examples and tests.
//!
//! Sized to TRANS-02's ~50 `LoC` budget for the `impl Transport` body + inbox
//! hub struct. `register`, `send_raw_for_test`, and the error variants live
//! outside that count (D-C6).

use crate::{MemoryTransportError, Transport, TransportMessage};
use famp_core::Principal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

type Inbox = mpsc::UnboundedReceiver<TransportMessage>;
type Outbox = mpsc::UnboundedSender<TransportMessage>;

#[derive(Clone, Default)]
pub struct MemoryTransport {
    senders: Arc<Mutex<HashMap<Principal, Outbox>>>,
    receivers: Arc<Mutex<HashMap<Principal, Arc<Mutex<Inbox>>>>>,
}

impl MemoryTransport {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate inbox channels for `principal`. Must be called before `send`
    /// or `recv` references the principal. Idempotent: re-registering the
    /// same principal is a no-op (returns existing inbox).
    pub async fn register(&self, principal: Principal) {
        let mut senders = self.senders.lock().await;
        if senders.contains_key(&principal) {
            return;
        }
        let (tx, rx) = mpsc::unbounded_channel();
        senders.insert(principal.clone(), tx);
        drop(senders);
        let mut receivers = self.receivers.lock().await;
        receivers.insert(principal, Arc::new(Mutex::new(rx)));
    }

    /// **Test-only** raw-bytes send path for adversarial injection.
    ///
    /// Gated behind the `test-util` feature and reachable only from
    /// `[dev-dependencies]` in the top `famp` crate. Production builds
    /// cannot link this symbol. See Plan 03-04 for the adversarial matrix
    /// that consumes this method.
    #[cfg(feature = "test-util")]
    pub async fn send_raw_for_test(
        &self,
        msg: TransportMessage,
    ) -> Result<(), MemoryTransportError> {
        self.send(msg).await
    }
}

impl Transport for MemoryTransport {
    type Error = MemoryTransportError;

    fn send(
        &self,
        msg: TransportMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let senders = self.senders.clone();
        async move {
            let map = senders.lock().await;
            let tx = map
                .get(&msg.recipient)
                .ok_or_else(|| MemoryTransportError::UnknownRecipient {
                    principal: msg.recipient.clone(),
                })?;
            tx.send(msg.clone())
                .map_err(|_| MemoryTransportError::InboxClosed {
                    principal: msg.recipient,
                })
        }
    }

    fn recv(
        &self,
        as_principal: &Principal,
    ) -> impl std::future::Future<Output = Result<TransportMessage, Self::Error>> + Send {
        let receivers = self.receivers.clone();
        let who = as_principal.clone();
        async move {
            let map = receivers.lock().await;
            let inbox = map
                .get(&who)
                .ok_or_else(|| MemoryTransportError::UnknownRecipient {
                    principal: who.clone(),
                })?
                .clone();
            drop(map);
            let mut guard = inbox.lock().await;
            guard
                .recv()
                .await
                .ok_or(MemoryTransportError::InboxClosed { principal: who })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::str::FromStr;
    use std::time::Duration;

    fn p(s: &str) -> Principal {
        Principal::from_str(s).unwrap()
    }

    #[tokio::test]
    async fn happy_path_roundtrip() {
        let t = MemoryTransport::new();
        let alice = p("agent:local/alice");
        let bob = p("agent:local/bob");
        t.register(alice.clone()).await;
        t.register(bob.clone()).await;

        let msg = TransportMessage {
            sender: alice.clone(),
            recipient: bob.clone(),
            bytes: b"hi".to_vec(),
        };
        t.send(msg).await.unwrap();

        let got = t.recv(&bob).await.unwrap();
        assert_eq!(got.sender, alice);
        assert_eq!(got.recipient, bob);
        assert_eq!(got.bytes, b"hi".to_vec());
    }

    #[tokio::test]
    async fn fifo_ordering() {
        let t = MemoryTransport::new();
        let alice = p("agent:local/alice");
        let bob = p("agent:local/bob");
        t.register(alice.clone()).await;
        t.register(bob.clone()).await;

        for payload in [b"one".as_slice(), b"two".as_slice(), b"three".as_slice()] {
            t.send(TransportMessage {
                sender: alice.clone(),
                recipient: bob.clone(),
                bytes: payload.to_vec(),
            })
            .await
            .unwrap();
        }

        assert_eq!(t.recv(&bob).await.unwrap().bytes, b"one".to_vec());
        assert_eq!(t.recv(&bob).await.unwrap().bytes, b"two".to_vec());
        assert_eq!(t.recv(&bob).await.unwrap().bytes, b"three".to_vec());
    }

    #[tokio::test]
    async fn unknown_recipient_returns_typed_error() {
        let t = MemoryTransport::new();
        let alice = p("agent:local/alice");
        let bob = p("agent:local/bob");
        t.register(alice.clone()).await;

        let err = t
            .send(TransportMessage {
                sender: alice,
                recipient: bob.clone(),
                bytes: b"x".to_vec(),
            })
            .await
            .unwrap_err();

        match err {
            MemoryTransportError::UnknownRecipient { principal } => {
                assert_eq!(principal, bob);
            }
            other @ MemoryTransportError::InboxClosed { .. } => {
                panic!("expected UnknownRecipient, got {other:?}")
            }
        }
    }

    #[cfg(feature = "test-util")]
    #[tokio::test]
    async fn send_raw_for_test_is_gated_and_works() {
        let t = MemoryTransport::new();
        let alice = p("agent:local/alice");
        let bob = p("agent:local/bob");
        t.register(alice.clone()).await;
        t.register(bob.clone()).await;

        let raw = vec![0xFF, 0xFF, 0xFF];
        t.send_raw_for_test(TransportMessage {
            sender: alice,
            recipient: bob.clone(),
            bytes: raw.clone(),
        })
        .await
        .unwrap();

        let got = t.recv(&bob).await.unwrap();
        assert_eq!(got.bytes, raw);
    }

    #[tokio::test]
    async fn cross_principal_isolation() {
        let t = MemoryTransport::new();
        let alice = p("agent:local/alice");
        let bob = p("agent:local/bob");
        let carol = p("agent:local/carol");
        t.register(alice.clone()).await;
        t.register(bob.clone()).await;
        t.register(carol.clone()).await;

        t.send(TransportMessage {
            sender: alice,
            recipient: bob,
            bytes: b"for-bob".to_vec(),
        })
        .await
        .unwrap();

        let timed = tokio::time::timeout(Duration::from_millis(50), t.recv(&carol)).await;
        assert!(timed.is_err(), "carol must not receive bob's message");
    }
}
