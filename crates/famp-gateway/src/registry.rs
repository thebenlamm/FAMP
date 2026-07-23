//! `GatewayRegistry` — demux table enforcing GW-04 (no cross-talk):
//! each proxied principal owns its own [`crate::ProxiedPrincipal`], keyed
//! strictly by name, never sharing a connection.
//!
//! Skeleton only in this task: the real `back()` constructor (which
//! registers a new proxied principal and rejects duplicates) lands in
//! Task 2 of this plan.

use std::collections::HashMap;

use crate::principal::ProxiedPrincipal;

/// `HashMap<PrincipalName, ProxiedPrincipal>` — the single demux point a
/// gateway process uses to route bus operations to the right proxied
/// principal, never crossing wires between two names (GW-04).
#[derive(Default)]
pub struct GatewayRegistry {
    principals: HashMap<String, ProxiedPrincipal>,
}

impl GatewayRegistry {
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
