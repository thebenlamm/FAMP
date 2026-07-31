//! `ProxiedPrincipal` — one gateway-held UDS connection registered on
//! behalf of a remote (cross-host) principal, carrying the gateway's own
//! `std::process::id()` (Design A — see 07-RESEARCH.md).

use std::path::Path;

use famp::bus_client::{BusClient, BusClientError};
use famp_bus::{BusMessage, BusReply};

use crate::error::GatewayError;

/// One proxied remote principal, backed by its own UDS connection to the
/// local broker. GW-04: never shared across principals — each is its own
/// `ProxiedPrincipal`/connection/`ClientId`.
pub struct ProxiedPrincipal {
    /// Held to keep the connection — and thus the gateway's PID
    /// registration under `name` — alive for as long as this value
    /// lives. The broker's `kill(pid,0)` liveness sweep only sees this
    /// principal as alive while the underlying socket stays open.
    /// Phase 9: also the drain/send channel — [`Self::send_recv`] issues
    /// `Send`/`Await`/`Inbox` ops directly on this same connection, so
    /// liveness and relay traffic share one UDS socket, never two.
    client: BusClient,
    name: String,
}

impl ProxiedPrincipal {
    /// Register `name` on the local broker at `sock`, on a brand-new UDS
    /// connection carrying the gateway's own real, live
    /// `std::process::id()` — never a value tied to the remote
    /// principal. `listen: false`: the gateway is the delivery
    /// mechanism, not a Stop-hook session (07-RESEARCH.md Resolution 2).
    ///
    /// Uses `connect_no_spawn` so an absent daemon fails loud
    /// (`GatewayError::BrokerUnreachable`) instead of being auto-spawned
    /// (07-RESEARCH.md Anti-Patterns: "Gateway auto-spawning a broker").
    pub async fn register(sock: &Path, name: String) -> Result<Self, GatewayError> {
        let mut client = BusClient::connect_no_spawn(sock, None)
            .await
            .map_err(map_bus_client_err)?;

        // Phase 14 D-01/D-17: this is the SOLE place the gateway declares
        // provenance. Its absence would be safe either way — the broker
        // defaults an omitted `origin` to `Origin::Unknown` (fail-closed,
        // D-01), never `Origin::Local` — but declaring `Gateway`
        // explicitly is what lets every downstream rendering surface
        // mark cross-host content as untrusted rather than merely
        // "unknown". This is the D-01 correction to the original
        // (rejected) opt-in design.
        let register = BusMessage::Register {
            name: name.clone(),
            pid: std::process::id(),
            cwd: None,
            listen: false,
            origin: Some(famp_bus::Origin::Gateway),
        };
        match client
            .send_recv(register)
            .await
            .map_err(map_bus_client_err)?
        {
            BusReply::RegisterOk { .. } => Ok(Self { client, name }),
            BusReply::Err { kind, message } => Err(GatewayError::RegisterFailed { kind, message }),
            other => Err(GatewayError::UnexpectedReply(
                other.variant_name().to_string(),
            )),
        }
    }

    /// The principal name this connection backs.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Issue any `BusMessage` on this principal's own UDS connection and
    /// return the raw `BusReply` (Phase 9 D-01/D-03/D-05: the same
    /// connection that keeps this principal live also drains/sends on
    /// its behalf — no second connection, no envelope `from`-vs-name
    /// cross-check here; the broker's `send()` handler trusts the
    /// connection's registration, not the envelope's own `from` field
    /// (09-RESEARCH.md §2), so the gateway's `verify_inbound_any` step
    /// remains the sole authorization boundary).
    pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, GatewayError> {
        self.client.send_recv(msg).await.map_err(map_bus_client_err)
    }
}

/// Map a `BusClientError` (raised at either the connect/Hello stage or
/// the Register `send_recv` stage) onto `GatewayError`.
///
/// A connect-stage `Io` error can only mean `UnixStream::connect`
/// failed — `connect_no_spawn` never auto-spawns a broker to paper over
/// this — so it maps to `BrokerUnreachable`, the must-have "fails loud"
/// truth for this plan.
fn map_bus_client_err(e: BusClientError) -> GatewayError {
    let display = e.to_string();
    match e {
        BusClientError::Io(_) | BusClientError::BrokerDidNotStart(_) => {
            GatewayError::BrokerUnreachable
        }
        BusClientError::Frame(_) | BusClientError::Decode(_) => {
            GatewayError::UnexpectedReply(display)
        }
        BusClientError::HelloFailed { kind, message } => {
            GatewayError::HelloFailed { kind, message }
        }
        BusClientError::ProtocolMismatch { broker_message } => GatewayError::HelloFailed {
            kind: famp_bus::BusErrorKind::BrokerProtoMismatch,
            message: broker_message,
        },
        BusClientError::UnexpectedReply(message) => GatewayError::UnexpectedReply(message),
    }
}
