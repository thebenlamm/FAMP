---
phase: 01-broker-diagnosis-identity-inspection
reviewed: 2026-05-10T18:01:22Z
depth: standard
files_reviewed: 31
files_reviewed_list:
  - Cargo.toml
  - .config/nextest.toml
  - Justfile
  - crates/famp-inspect-proto/Cargo.toml
  - crates/famp-inspect-proto/src/lib.rs
  - crates/famp-inspect-server/Cargo.toml
  - crates/famp-inspect-server/src/lib.rs
  - crates/famp-inspect-client/Cargo.toml
  - crates/famp-inspect-client/src/lib.rs
  - crates/famp-bus/Cargo.toml
  - crates/famp-bus/src/lib.rs
  - crates/famp-bus/src/proto.rs
  - crates/famp-bus/src/broker/mod.rs
  - crates/famp-bus/src/broker/state.rs
  - crates/famp-bus/src/broker/handle.rs
  - crates/famp/Cargo.toml
  - crates/famp/src/bin/famp.rs
  - crates/famp/src/lib.rs
  - crates/famp/src/cli/mod.rs
  - crates/famp/src/cli/error.rs
  - crates/famp/src/cli/register.rs
  - crates/famp/src/cli/broker/mod.rs
  - crates/famp/src/cli/broker/mailbox_env.rs
  - crates/famp/src/cli/broker/idle.rs
  - crates/famp/src/cli/inspect/mod.rs
  - crates/famp/src/cli/inspect/broker.rs
  - crates/famp/src/cli/inspect/identities.rs
  - crates/famp/src/cli/mcp/error_kind.rs
  - crates/famp/src/cli/mcp/tools/register.rs
  - crates/famp/tests/inspect_broker.rs
  - crates/famp/tests/inspect_identities.rs
findings:
  critical: 2
  warning: 0
  info: 0
  total: 2
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-05-10T18:01:22Z
**Depth:** standard
**Files Reviewed:** 31
**Status:** issues_found

## Summary

Reviewed the Phase 01 inspector protocol, broker sentinel integration, CLI rendering, and integration tests. The broker diagnosis path is mostly coherent, but identity inspection ships two incorrect data fields: `last_activity_unix_seconds` is never refreshed after registration, and `last_received_at_unix_seconds` is always absent for normal FAMP envelopes.

## Critical Issues

### CR-01: `last_activity_unix_seconds` Never Updates After Register

**Classification:** BLOCKER

**File:** `crates/famp-bus/src/broker/handle.rs:62`

**Issue:** `ClientState.last_activity` is set only in `register` at lines 224-226, then every authenticated operation (`Send`, `Inbox`, `Await`, `Join`, `Leave`, `Sessions`, `Whoami`, and `Inspect`) dispatches without touching it. This contradicts the state contract in `crates/famp-bus/src/broker/state.rs:40-44` and makes `famp inspect identities` report registration time as "last activity" forever, even after the identity sends, reads, joins, or is inspected. The current integration test only checks that the JSON key exists (`crates/famp/tests/inspect_identities.rs:206-217`), so the behavioral regression is untested.

**Fix:**
```rust
fn touch_activity<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId) {
    if let Some(state) = broker.state.clients.get_mut(&client) {
        if state.connected && (state.name.is_some() || state.bind_as.is_some()) {
            state.last_activity = std::time::SystemTime::now();
        }
    }
}

// In handle_wire, after rejecting invalid Hello/second Hello and before dispatching
// authenticated non-Hello messages:
if !matches!(msg, BusMessage::Hello { .. } | BusMessage::Register { .. }) {
    touch_activity(broker, client);
}
```

Also add an integration test that registers an identity, performs a later broker operation through that identity, then asserts `last_activity_unix_seconds >= registered_at_unix_seconds` and advances after a delay or controlled operation.

### CR-02: `last_received_at_unix_seconds` Is Always `None` For Real Messages

**Classification:** BLOCKER

**File:** `crates/famp/src/cli/broker/mod.rs:404`

**Issue:** `read_mailbox_meta_for` reads the last envelope's `ts` with `serde_json::Value::as_u64`, but FAMP envelopes write `ts` as an RFC3339 string (`crates/famp/src/cli/send/mod.rs:401-447`). As a result, identity inspection never reports `last_received_at_unix_seconds` for normal messages, violating the INSP-IDENT-02 mailbox metadata surface. The test at `crates/famp/tests/inspect_identities.rs:255-308` verifies unread/total and sender only; it does not assert that a sent message produces a non-null last-received timestamp.

**Fix:**
```rust
let last_received_at_unix_seconds = entries
    .last()
    .and_then(|value| value.get("ts").and_then(serde_json::Value::as_str))
    .and_then(|ts| time::OffsetDateTime::parse(
        ts,
        &time::format_description::well_known::Rfc3339,
    ).ok())
    .and_then(|dt| u64::try_from(dt.unix_timestamp()).ok());
```

Add a regression assertion to `inspect_identities_mailbox_metadata_unread_total` or the JSON schema test that sends a message and checks `last_received_at_unix_seconds` is a positive integer.

---

_Reviewed: 2026-05-10T18:01:22Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
