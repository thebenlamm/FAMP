//! `GatewayRegistry` — demux table enforcing GW-04 (no cross-talk):
//! each proxied principal owns its own [`crate::ProxiedPrincipal`], keyed
//! strictly by name, never sharing a connection.

use std::collections::HashMap;
use std::path::Path;

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
    principals: HashMap<String, ProxiedPrincipal>,
}

impl GatewayRegistry {
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
        self.principals.insert(name, principal);
        Ok(())
    }

    /// The `ProxiedPrincipal` backing `name`, if this gateway backs it.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ProxiedPrincipal> {
        self.principals.get(name)
    }

    /// Names of every principal currently backed by this gateway.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.principals.keys().map(String::as_str)
    }
}
