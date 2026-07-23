# Phase 7: Broker-Liveness Fork + Gateway Skeleton - Pattern Map

**Mapped:** 2026-07-23
**Files analyzed:** 6 (new) + 1 (extended existing test file)
**Analogs found:** 6 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/famp-gateway/Cargo.toml` | config | — | `crates/famp-transport-http/Cargo.toml` | exact (workspace-member skeleton) |
| `Cargo.toml` (root, add member) | config | — | existing `members` list, `famp-transport-http` entry | exact |
| `crates/famp-gateway/src/lib.rs` | module-root | — | `crates/famp-transport-http/src/lib.rs` | exact |
| `crates/famp-gateway/src/principal.rs` | service | CRUD (register/deregister lifecycle) | `crates/famp/src/bus_client/mod.rs` (`BusClient::connect`) | role-match (client-side register flow) |
| `crates/famp-gateway/src/registry.rs` | store | event-driven (demux by name) | `crates/famp-bus/src/broker/state.rs` (`ClientState` map) | partial (broker-side map, adapted client-side) |
| `crates/famp-gateway/src/error.rs` | utility | — | `crates/famp/src/bus_client/mod.rs` `BusClientError` (thiserror enum) | exact |
| `crates/famp-bus/src/broker/handle/tests.rs` (extend, add LIVE-01) | test | event-driven (pure `Broker<E>` + `FakeLiveness`) | same file, existing register/tick test helpers | exact |
| `crates/famp-gateway/tests/liveness.rs` (LIVE-02/GW-04) | test | request-response / process-lifecycle | `crates/famp/tests/broker_proxy_semantics.rs` | exact |
| `crates/famp-gateway/tests/common/child_guard.rs` | test-util | — | `crates/famp/tests/common/child_guard.rs` | exact (copy) |

## Pattern Assignments

### `crates/famp-gateway/Cargo.toml` (config)

**Analog:** `crates/famp-transport-http/Cargo.toml` (full file read above)

Copy the workspace-member skeleton shape verbatim (package block + `[lints] workspace = true`), trim deps to what Phase 7 needs:
```toml
[package]
name = "famp-gateway"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "FAMP v0.5.2 — famp-gateway crate (Layer 2 skeleton)"

[lints]
workspace = true

[dependencies]
famp-bus  = { path = "../famp-bus", version = "0.11.0" }
famp      = { path = "../famp", version = "0.11.0" }   # reuse BusClient (A3, discretion)
tokio     = { workspace = true, features = ["rt", "sync", "macros", "net"] }
thiserror = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }   # matches broker_proxy_semantics.rs pattern
```
Root `Cargo.toml` member entry — insert after the `famp-transport-http` line inside the existing "v1.0 federation internals" comment block (root `Cargo.toml:16-19`):
```toml
  "crates/famp-transport-http",
  "crates/famp-gateway",
```

---

### `crates/famp-gateway/src/lib.rs` (module-root)

**Analog:** `crates/famp-transport-http/src/lib.rs` (full file, 20 lines)

```rust
//! `famp-gateway` — FAMP v0.11 Layer 2 gateway skeleton.

#![forbid(unsafe_code)]

pub mod error;
pub mod principal;
pub mod registry;

pub use error::GatewayError;
pub use principal::ProxiedPrincipal;
pub use registry::GatewayRegistry;
```
Note: the transport-http analog uses `use famp_crypto as _;` silencer lines for not-yet-wired deps during incremental landing — reuse this idiom only if Phase 7 similarly stages deps ahead of use (e.g. if `famp` is added as a dep before `BusClient` reuse is wired in the same commit).

---

### `crates/famp-gateway/src/principal.rs` (service, CRUD register/deregister)

**Analog:** `crates/famp/src/bus_client/mod.rs` — `BusClient::connect` (lines 112-136) and its doc comment on `bind_as` semantics (lines 1-21).

**Imports pattern** (mod.rs lines 24-29):
```rust
use std::path::{Path, PathBuf};

use famp_bus::{BusErrorKind, BusMessage, BusReply, BUS_PROTO_VERSION};
use tokio::io::AsyncWriteExt as _;
use tokio::net::UnixStream;
```

**Core pattern — connect-with-own-PID, NOT `bind_as`** (RESEARCH.md Code Examples, wire-compatible with `BusClient::connect`'s retry loop, mod.rs:112-136):
```rust
// One UDS connection per proxied remote principal, gateway's own PID:
use famp_bus::{BusMessage, BusReply, BUS_PROTO_VERSION};

let hello = BusMessage::Hello { bus_proto: BUS_PROTO_VERSION, bind_as: None };
// ... send hello, await HelloOk (classify via classify_hello_reply-equivalent) ...
let register = BusMessage::Register {
    name: remote_principal_name.clone(),
    pid: std::process::id(),   // gateway's own real PID, not the remote's
    cwd: None,
    listen: false,             // A2/open-question-2: gateway is not a Stop-hook session
};
// ... send register, await RegisterOk ...
```

**Do NOT reuse `BusClient::connect`'s `spawn_broker_if_absent` call** (mod.rs:117) — Anti-Pattern per RESEARCH.md: gateway must fail loud if the daemon is unreachable, not auto-spawn. Either roll a minimal connect (skip `spawn::spawn_broker_if_absent`) or add a `BusClient::connect_no_spawn` variant during planning — flag as an in-phase task, not assumed.

**Error handling pattern** (mod.rs `BusClientError` enum, thiserror) — mirror shape for `GatewayError` (see error.rs below).

---

### `crates/famp-gateway/src/registry.rs` (store, demux by name — GW-04)

**Analog (structure only, not code copy):** `crates/famp-bus/src/broker/state.rs` `ClientState`/`clients: HashMap<ClientId, ClientState>` pattern — same shape, inverted to client-side:
```rust
pub struct GatewayRegistry {
    principals: std::collections::HashMap<String, ProxiedPrincipal>,
}
```
Invariant (RESEARCH.md GW-04 section): "an `Await`/`Inbox` result read from connection X is written only to X's own outbound channel" — no shared demux queue across principals. This is the gateway-internal isolation the broker's per-name mailbox model does NOT provide automatically at the gateway layer.

---

### `crates/famp-gateway/src/error.rs` (utility)

**Analog:** `crates/famp/src/bus_client/mod.rs` `BusClientError` (thiserror derive, lines ~39-59):
```rust
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("io error talking to broker")]
    Io(#[source] std::io::Error),
    #[error("broker unreachable — is the famp daemon running? (`famp daemon install`)")]
    BrokerUnreachable,
    #[error("Hello handshake refused: {kind:?}: {message}")]
    HelloFailed { kind: famp_bus::BusErrorKind, message: String },
    #[error("unexpected broker reply: {0}")]
    UnexpectedReply(String),
}
```

---

### `crates/famp-bus/src/broker/handle/tests.rs` — extend with LIVE-01 (test, pure Broker<E>)

**Analog:** same file, existing `TestEnv` harness (lines 1-60+) using `Rc<RefCell<FakeLiveness>>` (line 12), `hello_canonical` (line 31), `register` helper (line 45) — pattern already establishes N clients via `register(broker, client, name, pid, now)` calls.

**Core pattern for LIVE-01** — register N clients sharing one PID, tick, assert all still alive:
```rust
let mut broker = /* TestEnv::new() */;
let now = Instant::now();
hello_canonical(&mut broker, 1, "alice", now);
register(&mut broker, 1, "alice", /*pid=*/4242, now);
hello_canonical(&mut broker, 2, "bob", now);
register(&mut broker, 2, "bob", /*pid=*/4242, now); // SAME pid — Design A
// drive tick(); assert both clients remain in broker.state.clients
```

**Liveness plumbing being exercised (unmodified, `handle.rs:973-991`):**
```rust
fn tick<E: BrokerEnv>(broker: &mut Broker<E>, now: Instant) -> Vec<Out> {
    let dead_clients: Vec<ClientId> = broker.state.clients.iter()
        .filter_map(|(client, state)| {
            let pid = state.pid?;
            (!broker.env.is_alive(pid)).then_some(*client)
        })
        .collect();
    // ... disconnect() each dead one ...
}
```
`register()` sets `state.pid = Some(pid)` unconditionally (`handle.rs:358`) — no pid-uniqueness check anywhere in `register()` (`handle.rs:262-376`), which is the load-bearing fact this test pins.

---

### `crates/famp-gateway/tests/liveness.rs` — LIVE-02 + GW-04 (test, subprocess/process-lifecycle)

**Analog:** `crates/famp/tests/broker_proxy_semantics.rs` (full pattern read above, 80 lines shown) — copy the shape wholesale:

**Imports pattern** (broker_proxy_semantics.rs lines 33-40):
```rust
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::{BusClient, BusClientError};
use famp_bus::{BusErrorKind, BusMessage, BusReply, Target};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;
```

**Broker-subprocess spawn helper** (lines 66-74, copy verbatim, adjust binary/args only if gateway gets its own bin):
```rust
fn spawn_broker_subprocess(sock: &std::path::Path) -> std::process::Child {
    Command::cargo_bin("famp")
        .unwrap()
        .args(["broker", "--socket", sock.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}
```

**LIVE-02 shape** (adapt `spawn_register` pattern, lines 44-56, to instead spawn ONE "gateway-standin" process that opens N Register connections, then SIGKILL that one process and poll for all N to be reaped — mirrors the existing invariant-3 test `test_proxy_send_after_holder_dies` which already SIGKILLs a holder and polls with a timeout rather than a fixed sleep, per Pitfall 4).

**ChildGuard usage** — every spawned child (test broker, gateway-standin process) MUST be wrapped:
```rust
ChildGuard::new(Command::cargo_bin("famp").unwrap()./* ...args... */.spawn().unwrap())
```

---

### `crates/famp-gateway/tests/common/child_guard.rs` (test-util)

**Analog:** `crates/famp/tests/common/child_guard.rs` (full file, 41 lines) — copy verbatim (RAII kill+wait on drop):
```rust
pub struct ChildGuard(pub Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
```

## Shared Patterns

### Liveness sweep (unmodified — DO NOT edit)
**Source:** `crates/famp-bus/src/broker/handle.rs:973-991` (`tick()`), `crates/famp/src/cli/broker/mailbox_env.rs:103-123` (`is_alive` / `kill(pid,0)`), driven by `crates/famp/src/cli/broker/mod.rs:51-53,243-307` (1s `TICK_INTERVAL`).
**Apply to:** Nothing in this phase touches these files — they are the mechanism the gateway rides, confirmed zero-change.

### PID-carrying Register (the core Phase 7 mechanism)
**Source:** `crates/famp-bus/src/broker/handle.rs:262-376` (`register()`), no uniqueness check on `pid` (line 358 `state.pid = Some(pid)`).
**Apply to:** `principal.rs` — every `ProxiedPrincipal::register()` call must send `pid: std::process::id()` (the gateway's own), never a value tied to the remote principal.

### Error enum shape (thiserror)
**Source:** `crates/famp/src/bus_client/mod.rs` `BusClientError`.
**Apply to:** `crates/famp-gateway/src/error.rs`.

### ChildGuard test convention
**Source:** `crates/famp/tests/common/child_guard.rs`.
**Apply to:** `crates/famp-gateway/tests/liveness.rs` and any other test spawning `famp broker`/`famp register`/gateway-standin children.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/famp-gateway/src/registry.rs` | store | event-driven demux | No existing client-side "N-connections-one-process" demux table exists in the codebase; `ClientState` map is the closest structural cousin but lives broker-side, not client-side — build fresh per RESEARCH.md's `HashMap<PrincipalName, ConnectionHandle>` recommendation. |

## Metadata

**Analog search scope:** `crates/famp-bus/src/broker/`, `crates/famp/src/bus_client/`, `crates/famp/tests/`, `crates/famp-transport-http/`, root `Cargo.toml`
**Files scanned:** `handle.rs`, `mailbox_env.rs`, `mod.rs` (broker CLI), `liveness.rs`, `bus_client/mod.rs`, `broker_proxy_semantics.rs`, `child_guard.rs`, `handle/tests.rs`, `famp-transport-http/Cargo.toml` + `lib.rs`, `famp-bus/Cargo.toml`
**Pattern extraction date:** 2026-07-23
