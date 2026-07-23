//! `ProxiedPrincipal` — one gateway-held UDS connection registered on
//! behalf of a remote (cross-host) principal.
//!
//! Skeleton only in this task: the real `register()` constructor (which
//! performs the no-spawn connect + Register-with-own-PID handshake)
//! lands in Task 2 of this plan.

/// One proxied remote principal, backed by its own UDS connection to the
/// local broker. GW-04: never shared across principals — each is its own
/// `ProxiedPrincipal`/connection/`ClientId`.
pub struct ProxiedPrincipal {
    name: String,
}

impl ProxiedPrincipal {
    /// The principal name this connection backs.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}
