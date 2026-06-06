//! D-10 identity resolution helpers for the broker actor.
//!
//! Extracted from `handle.rs` (§1). The canonical-holder / proxy-liveness
//! lookups live here so dispatch (`handle`) and the await subsystem
//! (`awaiting`) route through one implementation instead of re-inlining
//! the lookup at each call site.

use crate::broker::state::ClientState;
use crate::{Broker, BrokerEnv, BusErrorKind, ClientId};

/// Pre-D-10 helper kept for callers that need the *registered* name
/// (the canonical-holder slot) explicitly, regardless of any proxy
/// binding. Most ops should use [`resolve_op_identity`] instead.
#[allow(dead_code)]
pub(super) fn registered_name<E: BrokerEnv>(
    broker: &Broker<E>,
    client: ClientId,
) -> Option<String> {
    broker
        .state
        .clients
        .get(&client)
        .filter(|state| state.connected)
        .and_then(|state| state.name.clone())
}

/// D-10: resolve the effective identity for `client`. Returns the
/// registered holder's name (`state.name`) for canonical connections,
/// the bound holder's name (`state.bind_as`) for proxy connections,
/// or `Err(NotRegistered)` if neither is set.
///
/// This is the central identity-resolution entry point — every
/// identity-required op (`Send`, `Inbox`, `Await`, `Join`, `Leave`,
/// `Whoami`) calls into it instead of `state.name` directly.
pub(super) fn effective_identity(state: &ClientState) -> Result<String, BusErrorKind> {
    if let Some(ref name) = state.name {
        return Ok(name.clone());
    }
    if let Some(ref bound) = state.bind_as {
        return Ok(bound.clone());
    }
    Err(BusErrorKind::NotRegistered)
}

/// D-10: per-op liveness re-check for proxy connections. Returns true
/// iff the canonical holder of `bound` is still connected AND its PID
/// answers `is_alive`. Called by every identity-required op when the
/// caller is a proxy (`state.bind_as = Some(_)`).
pub(super) fn proxy_holder_alive<E: BrokerEnv>(broker: &Broker<E>, bound: &str) -> bool {
    broker.state.clients.values().any(|h| {
        h.connected
            && h.name.as_deref() == Some(bound)
            && h.pid.is_some_and(|pid| broker.env.is_alive(pid))
    })
}

/// D-10: `ClientId` of the canonical live holder for `bound`, or
/// `None` if no holder is currently registered. Used by Join/Leave to
/// mutate the canonical holder's `joined` set instead of the proxy's.
pub(super) fn canonical_holder_id<E: BrokerEnv>(
    broker: &Broker<E>,
    bound: &str,
) -> Option<ClientId> {
    broker.state.clients.iter().find_map(|(id, state)| {
        if state.connected && state.name.as_deref() == Some(bound) {
            Some(*id)
        } else {
            None
        }
    })
}

/// D-10: resolve effective identity AND verify proxy liveness in one
/// step. Returns `Err(NotRegistered)` if the connection has no
/// resolvable identity OR if it is a proxy whose holder has died.
pub(super) fn resolve_op_identity<E: BrokerEnv>(
    broker: &Broker<E>,
    client: ClientId,
) -> Result<String, BusErrorKind> {
    let state = broker
        .state
        .clients
        .get(&client)
        .ok_or(BusErrorKind::NotRegistered)?;
    if !state.connected {
        return Err(BusErrorKind::NotRegistered);
    }
    let identity = effective_identity(state)?;
    // Canonical holder owns the slot; no liveness re-check needed.
    if state.name.is_some() {
        return Ok(identity);
    }
    // Proxy: re-verify the canonical holder is still alive.
    if proxy_holder_alive(broker, &identity) {
        Ok(identity)
    } else {
        Err(BusErrorKind::NotRegistered)
    }
}
