// PLAN 02-09: implement
//! `famp_register` MCP tool — D-04/D-10 rewire stub.
//!
//! Plan 02-08 reshaped `cli::mcp::session` to drop the v0.8
//! `IdentityBinding` / `BindingSource` / `home_path` surface in favour
//! of a `BusClient` + `active_identity` model. The real `famp_register`
//! body (which sends a `BusMessage::Register { name }` frame to the
//! broker via `session::ensure_bus()` and then calls
//! `session::set_active_identity` on `RegisterOk`) lands in plan 02-09.
//!
//! Until then, this file holds the `pub async fn call(input: &Value)`
//! signature and a stub body so `cli::mcp::server::dispatch_tool`
//! compiles. Calling the stub at runtime panics — but the MCP server
//! is not part of any 02-08 test path that exercises tool dispatch
//! (the binding-required branch returns `NotRegistered` before
//! reaching here, and `mcp_error_kind_exhaustive` only exercises the
//! pure error-mapping table).

use serde_json::Value;

use crate::cli::error::CliError;

/// Dispatch a `famp_register` tool call. Stub — see plan 02-09.
#[allow(clippy::unused_async)] // body is `unimplemented!()` until plan 02-09 wires the bus.
pub async fn call(_input: &Value) -> Result<Value, CliError> {
    unimplemented!("rewired in plan 02-09")
}
