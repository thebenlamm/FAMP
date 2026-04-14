//! `HttpTransport` — `famp_transport::Transport` impl over axum + reqwest + rustls.
//!
//! D-A2 / D-A5 / D-A6: `addr_map` (client) + inboxes (server) + shared
//! `reqwest::Client`. D-F5: native AFIT, NO `async_trait` macro. D-C8: errors
//! via `HttpTransportError`.
//!
//! This module owns the *client* side: it constructs the rustls-backed
//! `reqwest::Client`, holds the per-recipient address map, and implements
//! `Transport::send` by `POST`ing the raw bytes to
//! `{base}/famp/v0.5.1/inbox/{recipient}`.
//!
//! The *server* side (`build_router` from `server.rs` + `tls_server::serve`
//! from `tls_server.rs`) is wired into the same `HttpTransport` via
//! `attach_server` so that a single struct can be dropped and the spawned
//! axum task is aborted.

use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use famp_core::Principal;
use famp_transport::{Transport, TransportMessage};
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use url::Url;

use crate::{
    error::HttpTransportError,
    server::InboxRegistry,
    tls::build_client_config,
};

const INBOX_CHANNEL_CAPACITY: usize = 64;

pub struct HttpTransport {
    addr_map: Arc<Mutex<HashMap<Principal, Url>>>,
    inboxes: Arc<InboxRegistry>,
    receivers: Mutex<HashMap<Principal, mpsc::Receiver<TransportMessage>>>,
    client: reqwest::Client,
    server_handle: Mutex<Option<JoinHandle<std::io::Result<()>>>>,
}

impl HttpTransport {
    /// Build an `HttpTransport` whose rustls client config trusts OS roots
    /// plus the cert at `trust_cert_path` (D-B5 full combination).
    ///
    /// Pass `None` for `trust_cert_path` to trust only the OS root store.
    /// Pass `Some(path)` for the dev workflow where the peer published a
    /// self-signed cert.
    ///
    /// # Errors
    ///
    /// Returns `HttpTransportError::TlsConfig` if the rustls client config
    /// cannot be built (bad PEM, missing crypto provider, etc.) or
    /// `HttpTransportError::ReqwestFailed` if the underlying reqwest builder
    /// rejects the supplied TLS config.
    pub fn new_client_only(trust_cert_path: Option<&Path>) -> Result<Self, HttpTransportError> {
        let tls = build_client_config(trust_cert_path)
            .map_err(|e| HttpTransportError::TlsConfig(format!("{e:?}")))?;
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .timeout(Duration::from_secs(10))
            .http1_only()
            .build()
            .map_err(HttpTransportError::ReqwestFailed)?;
        Ok(Self {
            addr_map: Arc::new(Mutex::new(HashMap::new())),
            inboxes: Arc::new(Mutex::new(HashMap::new())),
            receivers: Mutex::new(HashMap::new()),
            client,
            server_handle: Mutex::new(None),
        })
    }

    /// Register a peer's HTTPS base URL. The recipient path segment is
    /// appended at `send` time, so `url` should be the scheme+host+port
    /// only (e.g. `https://127.0.0.1:8443`).
    pub async fn add_peer(&self, principal: Principal, url: Url) {
        self.addr_map.lock().await.insert(principal, url);
    }

    /// Register `principal` as receivable on this transport. Idempotent —
    /// re-registering the same principal is a no-op. Mirrors
    /// `MemoryTransport::register` from Phase 3.
    pub async fn register(&self, principal: Principal) {
        let mut inboxes = self.inboxes.lock().await;
        if inboxes.contains_key(&principal) {
            return;
        }
        let (tx, rx) = mpsc::channel(INBOX_CHANNEL_CAPACITY);
        inboxes.insert(principal.clone(), tx);
        drop(inboxes);
        self.receivers.lock().await.insert(principal, rx);
    }

    /// Clone the inbox registry so the caller can pass it to `build_router`.
    /// The registry is `Arc`-shared, so the server-side handler and this
    /// `HttpTransport` write/read the same map.
    #[must_use]
    pub fn inboxes(&self) -> Arc<InboxRegistry> {
        self.inboxes.clone()
    }

    /// Store the spawned axum-server `JoinHandle` so this `HttpTransport` can
    /// abort it on drop.
    pub async fn attach_server(&self, handle: JoinHandle<std::io::Result<()>>) {
        *self.server_handle.lock().await = Some(handle);
    }
}

impl Transport for HttpTransport {
    type Error = HttpTransportError;

    fn send(
        &self,
        msg: TransportMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let client = self.client.clone();
        let addr_map = self.addr_map.clone();
        async move {
            let base = {
                let map = addr_map.lock().await;
                map.get(&msg.recipient).cloned().ok_or_else(|| {
                    HttpTransportError::UnknownRecipient {
                        principal: msg.recipient.clone(),
                    }
                })?
            };
            // D-A6: POST to `{base}/famp/v0.5.1/inbox/{recipient}`.
            // `Url::join` requires the base URL to end with `/` for the path
            // segment to be appended rather than replacing the last segment;
            // construct the full URL by string concatenation to keep the
            // semantics simple and predictable.
            let inbox_url_str = format!(
                "{}/famp/v0.5.1/inbox/{}",
                base.as_str().trim_end_matches('/'),
                msg.recipient
            );
            let inbox_url = Url::parse(&inbox_url_str).map_err(HttpTransportError::InvalidUrl)?;

            let resp = client
                .post(inbox_url)
                .header("content-type", "application/famp+json")
                .body(msg.bytes)
                .send()
                .await
                .map_err(HttpTransportError::ReqwestFailed)?;
            let status = resp.status();
            if status == reqwest::StatusCode::ACCEPTED {
                Ok(())
            } else {
                let code = status.as_u16();
                let body = resp.text().await.unwrap_or_default();
                Err(HttpTransportError::ServerStatus { code, body })
            }
        }
    }

    // The receivers map holds `mpsc::Receiver`s by-value; awaiting
    // `recv()` requires `&mut Receiver`, so the outer Mutex guard MUST stay
    // alive across the await. Clippy's `significant_drop_tightening` would
    // suggest dropping the guard early, but doing so would invalidate the
    // borrow on `rx`. Hold the lock for the duration of the await — the
    // single-receiver-per-principal contention model means this is
    // uncontended in practice.
    #[allow(clippy::significant_drop_tightening)]
    fn recv(
        &self,
        as_principal: &Principal,
    ) -> impl std::future::Future<Output = Result<TransportMessage, Self::Error>> + Send {
        let who = as_principal.clone();
        async move {
            let mut guard = self.receivers.lock().await;
            let rx = guard.get_mut(&who).ok_or_else(|| {
                HttpTransportError::InboxClosed {
                    principal: who.clone(),
                }
            })?;
            rx.recv()
                .await
                .ok_or(HttpTransportError::InboxClosed { principal: who })
        }
    }
}

impl Drop for HttpTransport {
    fn drop(&mut self) {
        // Best-effort cleanup of any attached server task on drop. `try_lock`
        // avoids blocking in a destructor; the task will be aborted on the
        // next runtime tick if the lock is uncontended (which it almost
        // always is at drop time).
        if let Ok(mut guard) = self.server_handle.try_lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn new_client_only_builds_without_trust_cert() {
        let _t = HttpTransport::new_client_only(None).expect("client builds");
    }

    #[tokio::test]
    async fn send_unknown_recipient_returns_typed_error() {
        let t = HttpTransport::new_client_only(None).expect("client builds");
        let alice = Principal::from_str("agent:local/alice").unwrap();
        let bob = Principal::from_str("agent:local/bob").unwrap();
        let err = t
            .send(TransportMessage {
                sender: alice,
                recipient: bob.clone(),
                bytes: b"hi".to_vec(),
            })
            .await
            .unwrap_err();
        match err {
            HttpTransportError::UnknownRecipient { principal } => assert_eq!(principal, bob),
            other => panic!("expected UnknownRecipient, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_is_idempotent() {
        let t = HttpTransport::new_client_only(None).expect("client builds");
        let alice = Principal::from_str("agent:local/alice").unwrap();
        t.register(alice.clone()).await;
        t.register(alice.clone()).await;
        // Receivers map should still hold exactly one entry.
        assert_eq!(t.receivers.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn add_peer_populates_addr_map() {
        let t = HttpTransport::new_client_only(None).expect("client builds");
        let bob = Principal::from_str("agent:local/bob").unwrap();
        let url = Url::parse("https://127.0.0.1:8443").unwrap();
        t.add_peer(bob.clone(), url.clone()).await;
        let stored = t.addr_map.lock().await.get(&bob).cloned();
        assert_eq!(stored, Some(url));
    }
}
