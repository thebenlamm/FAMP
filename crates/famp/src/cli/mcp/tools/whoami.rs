// PLAN 02-09: implement
//! `famp_whoami` MCP tool — D-04 rewire stub.
//!
//! The real body reads `session::active_identity()` and returns
//! `{ "identity": <name>|null, "source": "explicit"|"unregistered" }`.
//! Lands in plan 02-09 alongside the other tool rewires.

use serde_json::Value;

use crate::cli::error::CliError;

/// Dispatch a `famp_whoami` tool call. Stub — see plan 02-09.
#[allow(clippy::unused_async)] // body is `unimplemented!()` until plan 02-09 wires the bus.
pub async fn call(_input: &Value) -> Result<Value, CliError> {
    unimplemented!("rewired in plan 02-09")
}
