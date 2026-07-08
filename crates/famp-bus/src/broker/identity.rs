//! D-10 identity resolution helpers for the broker actor.
//!
//! Extracted from `handle.rs` (§1). The canonical-holder / proxy-liveness
//! lookups live here so dispatch (`handle`) and the await subsystem
//! (`awaiting`) route through one implementation instead of re-inlining
//! the lookup at each call site.

use crate::broker::state::{BrokerState, ClientState};
use crate::{Broker, BrokerEnv, BusErrorKind, ClientId};

/// Pre-D-10 helper kept for callers that need the *registered* name
/// (the canonical-holder slot) explicitly, regardless of any proxy
/// binding. Most ops should use [`resolve_op_identity`] instead.
///
/// Takes `&BrokerState`, not `&Broker<E>`: it never consults `broker.env`.
/// Narrowing it lets `BrokerState`'s own `&self` methods reuse it.
#[allow(dead_code)]
pub(super) fn registered_name(state: &BrokerState, client: ClientId) -> Option<String> {
    state
        .clients
        .get(&client)
        .filter(|client_state| client_state.connected)
        .and_then(|client_state| client_state.name.clone())
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
/// mutate the canonical holder's `joined` set instead of the proxy's,
/// and by [`BrokerState::view`] to attribute a parked await's cursors
/// to the holder rather than the proxy.
///
/// Takes `&BrokerState`, not `&Broker<E>`: liveness is a `connected`-flag
/// check, not a `broker.env.is_alive` probe (that is [`proxy_holder_alive`]).
/// This is the SINGLE canonical-holder-by-`bind_as` lookup in the broker —
/// `view()` used to re-inline the same `find_map`, which is drift waiting to
/// happen.
///
/// [`BrokerState::view`]: crate::broker::state::BrokerState::view
pub(super) fn canonical_holder_id(state: &BrokerState, bound: &str) -> Option<ClientId> {
    state.clients.iter().find_map(|(id, client_state)| {
        if client_state.connected && client_state.name.as_deref() == Some(bound) {
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
