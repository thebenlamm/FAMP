// PLAN 02-09: implement
//! `famp_peers` MCP tool — D-04 rewire stub.
//!
//! The real body uses `session::ensure_bus()` and sends a
//! `BusMessage::Peers { … }` frame to the broker. Lands in plan 02-09.
//!
//! NOTE: signature is `pub async fn` (was `pub fn` in v0.8) so the
//! dispatcher can `.await` uniformly across all tool calls.

use serde_json::Value;

use famp_bus::BusErrorKind;

/// Dispatch a `famp_peers` tool call. Stub — see plan 02-09.
#[allow(clippy::unused_async)] // body is `unimplemented!()` until plan 02-09 wires the bus.
pub async fn call(_input: &Value) -> Result<Value, BusErrorKind> {
    unimplemented!("rewired in plan 02-09")
}
