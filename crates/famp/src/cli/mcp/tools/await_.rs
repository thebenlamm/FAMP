// PLAN 02-09: implement
//! `famp_await` MCP tool — D-04 rewire stub.
//!
//! The real body uses `session::ensure_bus()` and sends a
//! `BusMessage::Await { … }` frame to the broker. Lands in plan 02-09.

use serde_json::Value;

use crate::cli::error::CliError;

/// Dispatch a `famp_await` tool call. Stub — see plan 02-09.
#[allow(clippy::unused_async)] // body is `unimplemented!()` until plan 02-09 wires the bus.
pub async fn call(_input: &Value) -> Result<Value, CliError> {
    unimplemented!("rewired in plan 02-09")
}
