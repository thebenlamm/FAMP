//! `GatewayError` — error enum for `famp-gateway` operations.
//!
//! Mirrors the shape of `famp::bus_client::BusClientError` (thiserror,
//! narrow phase-appropriate enum per project convention) but scoped to
//! what the gateway itself can fail at: connecting to the local broker
//! without auto-spawning it, and backing a proxied principal.

/// Errors produced while backing a proxied remote principal on the
/// local UDS bus.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// I/O error talking to the broker (connect, read, or write failure).
    #[error("io error talking to broker")]
    Io(#[source] std::io::Error),

    /// The local broker daemon is not reachable. The gateway never
    /// auto-spawns a broker (unlike CLI `connect`) — it fails loud so a
    /// long-running Layer-2 service does not paper over a down daemon.
    #[error("broker unreachable — is the famp daemon running? (`famp daemon install`)")]
    BrokerUnreachable,

    /// The broker refused the Hello handshake.
    #[error("Hello handshake refused: {kind:?}: {message}")]
    HelloFailed {
        kind: famp_bus::BusErrorKind,
        message: String,
    },

    /// The broker refused the Register frame for a proxied principal.
    #[error("Register refused for '{kind:?}': {message}")]
    RegisterFailed {
        kind: famp_bus::BusErrorKind,
        message: String,
    },

    /// The broker replied with something other than what the gateway
    /// expected for the operation in progress.
    #[error("unexpected broker reply: {0}")]
    UnexpectedReply(String),

    /// `GatewayRegistry::back` was asked to back a principal name that
    /// is already backed by this gateway process (GW-04: one
    /// `ProxiedPrincipal` per name, never shared).
    #[error("principal '{0}' is already backed by this gateway")]
    DuplicatePrincipal(String),
}
