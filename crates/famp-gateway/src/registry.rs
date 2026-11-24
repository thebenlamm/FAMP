//! `GatewayRegistry` — demux table enforcing GW-04 (no cross-talk):
//! each proxied principal owns its own [`crate::ProxiedPrincipal`], keyed
//! strictly by name, never sharing a connection.
//!
//! Locking: every principal sits behind its OWN mutex, because its one bus
//! connection is shared by its egress loop (a 1 s `Await` long-poll) and by
//! ingress deliveries from that peer, and a request and its reply must never
//! interleave on it. The lock around the whole registry (held by callers as
//! `Arc<Mutex<GatewayRegistry>>`) is held only for a lookup, never across a bus
//! round trip, so one peer's long-poll never delays another peer's delivery.
//! Nobody holds two principal locks at once, and nobody takes the registry lock
//! while holding a principal lock.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::GatewayError;
use crate::principal::ProxiedPrincipal;

/// The single demux point a gateway process uses to route bus operations
/// to the right proxied principal (GW-04).
///
/// `HashMap<PrincipalName, ProxiedPrincipal>` — never crosses wires
/// between two names. Each value owns its own UDS connection; there is
/// no shared connection or demux queue across keys.
#[derive(Default)]
pub struct GatewayRegistry {
    principals: HashMap<String, Arc<Mutex<ProxiedPrincipal>>>,
    /// Test-only: pretend the local bus is slow by sleeping, with the sender's
    /// connection held, before each delivery's bus round trip. Compiled out of every
    /// non-test build.
    #[cfg(test)]
    test_delivery_delay: Option<std::time::Duration>,
}

impl GatewayRegistry {
    /// Test-only: see `test_delivery_delay`.
    #[cfg(test)]
    pub(crate) const fn set_test_delivery_delay(&mut self, delay: Option<std::time::Duration>) {
        self.test_delivery_delay = delay;
    }

    #[cfg(test)]
    pub(crate) const fn test_delivery_delay(&self) -> Option<std::time::Duration> {
        self.test_delivery_delay
    }

    /// Register `name` on the broker at `sock` and back it with a new
    /// `ProxiedPrincipal`. Rejects an already-backed name with
    /// `GatewayError::DuplicatePrincipal` rather than silently replacing
    /// (a silent replace would drop the old connection's own PID
    /// registration mid-flight — a correctness hazard for GW-04).
    pub async fn back(&mut self, sock: &Path, name: String) -> Result<(), GatewayError> {
        if self.principals.contains_key(&name) {
            return Err(GatewayError::DuplicatePrincipal(name));
        }
        let principal = ProxiedPrincipal::register(sock, name.clone()).await?;
        self.principals
            .insert(name, Arc::new(Mutex::new(principal)));
        Ok(())
    }

    /// Whether this gateway backs `name`.
    #[must_use]
    pub fn backs(&self, name: &str) -> bool {
        self.principals.contains_key(name)
    }

    /// The `ProxiedPrincipal` backing `name`, behind its own lock, if this gateway
    /// backs it. Drop the registry lock BEFORE locking the principal (see the module
    /// doc): outbound drain and inbound deliver both lock it to call `send_recv`.
    #[must_use]
    pub fn principal(&self, name: &str) -> Option<Arc<Mutex<ProxiedPrincipal>>> {
        self.principals.get(name).cloned()
    }

    /// Names of every principal currently backed by this gateway.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.principals.keys().map(String::as_str)
    }
}
