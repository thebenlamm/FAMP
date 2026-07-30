# Phase 7: Broker-Liveness Fork + Gateway Skeleton - Research

**Researched:** 2026-07-23
**Domain:** Rust workspace-internal — UDS broker protocol semantics (`famp-bus`), process-liveness detection, new crate scaffolding
**Confidence:** HIGH (every load-bearing claim below is grounded in a direct Read/grep of the current codebase, not training-data recall)

## Summary

The single most important finding of this research: **the mechanism Design A needs already exists and already ships in production** — it is not something Phase 7 invents from scratch. `famp-bus` has carried a "D-10 proxy" feature since a 2026-05/06-era refactor (`Hello { bind_as: Option<String> }`) that lets a connection operate *as* an already-registered canonical holder without re-registering. That is NOT the mechanism Phase 7's Design A needs (bind_as requires the holder to already be registered elsewhere with a live local PID) — but reading it revealed the *real* mechanism sitting one layer below it: `register()` (`crates/famp-bus/src/broker/handle.rs:262-376`) puts zero constraint on the `pid` field beyond rejecting `0` — no peer-credential cross-check, no one-name-per-pid uniqueness. Any number of `ClientState` entries may legitimately share one `pid`. This means Design A reduces to: **the gateway opens N independent UDS connections to the local broker, one per proxied remote principal, and issues `Hello{bind_as:None}` + `Register{name: <principal>, pid: <gateway's own getpid()>, ...}` on each.** Because it is the gateway's own real, live PID, the existing `kill(pid,0)` sweep (`crates/famp/src/cli/broker/mailbox_env.rs:103-123`, driven every 1s by `crates/famp/src/cli/broker/mod.rs:53`) reports every proxied principal alive for exactly as long as the gateway process runs, and reports them all dead the instant it doesn't — **with zero `famp-bus` source change**. LIVE-01/LIVE-02 are structurally solved by this connection pattern alone; GW-04 (no cross-talk) falls out of the fact that each proxied principal is its own independent socket/`ClientId`/mailbox file — the isolation is the broker's existing per-name mailbox model, not something the gateway has to build.

**Primary recommendation:** Design A, exactly as scoped in STATE.md — but implement it as "N long-lived plain-Register connections, one per proxied principal, all reporting the gateway's PID," NOT as N `bind_as` proxy connections (that pattern is for a *different* problem: multiplexing one-shot CLI ops against an *already-registered* holder). Do not confuse the two — they are both called "D-10" adjacent in comments but serve different purposes.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Proxied-principal liveness (LIVE-01/02) | Local bus broker (`famp-bus`) | Gateway process (`famp-gateway`) | Broker owns the liveness *check* (`kill(pid,0)` sweep, unmodified); gateway owns *what PID gets reported* by registering with its own — the fix is a caller-behavior change, not a broker change |
| Per-principal registration lifecycle | Gateway process | Local bus broker | Gateway opens/holds/closes one UDS connection per remote principal; broker just accepts `Register` frames as it always has |
| Message routing / no cross-talk (GW-04) | Local bus broker (mailbox-per-name) | Gateway (demux to correct outbound wire session) | Broker already isolates mailboxes by name — GW-04 correctness is really "does the gateway hand each inbound bus message to the right cross-host wire session," a gateway-internal concern |
| Cross-host wire transport | `famp-gateway` (Layer 2) | `famp-transport-http` (reused, not rebuilt) | Out of scope for Phase 7 body — skeleton only; Phase 8 wires the signed envelope path |
| Signing / verification | `famp-crypto` / `famp-keyring` | `famp-gateway` | Untouched in Phase 7; Phase 8 concern |

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LIVE-01 | Proxied principal stays live for as long as gateway process runs, across the broker's `kill(pid,0)` sweep | Solved by Register-with-own-PID pattern; sweep code at `handle.rs:973-991`, real probe at `mailbox_env.rs:103-123` — both unmodified |
| LIVE-02 | When gateway exits, all its proxied principals reap cleanly, no orphans | Solved by the same pattern: gateway PID death → `kill(pid,0)` returns false on next tick (≤1s) for every connection carrying that PID; OS-level UDS EOF likely reaps most connections even faster via `disconnect()` |
| GW-04 | One gateway backs 2+ remote principals concurrently, no cross-talk | Solved structurally: each principal is a distinct `ClientId`/socket/mailbox file; gateway must hold a `HashMap<PrincipalName, BusClient>` and route strictly by key — see Pitfalls |

## Standard Stack

### Core (all reused, zero new external dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `famp-bus` | workspace 0.11.0 (path dep) | UDS broker protocol types + client codec | The existing protocol this phase's fix rides on — not rebuilt |
| `tokio` | 1.51.1 (workspace) `[VERIFIED: crates/Cargo.toml]` | Async runtime for N concurrent gateway-held connections | Already the workspace async runtime; `famp-bus` itself is intentionally tokio-free (CI gate `check-no-tokio-in-bus`), but the CLI/gateway layer uses it |
| `thiserror` | 2.0.18 (workspace) `[VERIFIED: Cargo.toml]` | Gateway error enum | Workspace convention (every crate uses this, not `anyhow`, for library error types) |

No new external crates are required for the Phase 7 skeleton. `famp-gateway` needs no `nix` dependency itself — the `kill(pid,0)` liveness probe lives entirely in `crates/famp/src/cli/broker/mailbox_env.rs` (the broker side), not anywhere the gateway touches.

### Supporting (present, reused later — NOT wired in Phase 7 body)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `famp-transport-http` | workspace (path dep, already in `Cargo.toml` members) | Cross-host HTTP+TLS transport, preserved since v0.9 | Phase 8+ — Phase 7 only needs the crate to exist/compile in the workspace, not be called |
| `famp-keyring` | workspace (path dep) | TOFU peer keyring | Phase 8 (TRUST-01/02) |

### Package Legitimacy Audit

**Not applicable in the standard sense** — this phase adds zero new external (crates.io) packages. It adds exactly one new **internal** workspace member (`famp-gateway`), a `path`-only crate with no registry presence. Its own dependencies (`famp-bus`, `tokio`, `thiserror`) are all already-vetted, already-in-use workspace dependencies (see Standard Stack above); none require registry legitimacy verification.

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### The Liveness Fork — exact current mechanism `[VERIFIED: direct file reads, 2026-07-23]`

**Liveness sweep (periodic backstop):**
- `crates/famp-bus/src/broker/handle.rs:973-991` — `fn tick()`. Every `BrokerInput::Tick`, it filters `broker.state.clients` for entries where `state.pid` is `Some(pid)` and `!broker.env.is_alive(pid)` (line 980), then calls `disconnect(broker, client)` (line 990) for each dead one, which emits `Out::SessionEnded` + `Out::ReleaseClient`.
- Driven by `crates/famp/src/cli/broker/mod.rs:51-53,243-307` — `TICK_INTERVAL = Duration::from_secs(1)` (line 53), a `tokio::time::interval` firing `BrokerInput::Tick` (line 306-307). **This is the "one liveness-sweep interval" in LIVE-02 — 1 second, not something Phase 7 needs to change.**

**The actual `kill(pid,0)` probe:**
- `crates/famp/src/cli/broker/mailbox_env.rs:103-123` — `impl LivenessProbe for DiskMailboxEnv::is_alive`. Rejects pid 0 and non-positive values (BL-05 guard against POSIX's pgrp-targeting semantics for `kill(0, sig)`), then calls `nix::sys::signal::kill(Pid::from_raw(raw), None)` (line 121) — `sig=None` is the POSIX "existence check" form, returns `Ok` iff a process with that PID exists and the caller has permission to signal it. **This is the exact call the 2026-07-19 cross-host spike diagnosed as transport-independent** — it is local-only by construction (a PID from another host means nothing to this syscall).
- **Prompt drift note:** the phase brief cited `handle.rs:916` and `identity.rs:52` as anchors. `identity.rs:52` is exact — that line is inside `proxy_holder_alive()` and reads `&& h.pid.is_some_and(|pid| broker.env.is_alive(pid))`. `handle.rs:916` has drifted (current `disconnect()` starts at line 920, `tick()` at line 973) — the file has grown since the spike; cite `handle.rs:973` (sweep) and `handle.rs:920` (disconnect) going forward.

**Where a holder's PID is recorded:** `crates/famp-bus/src/broker/handle.rs:358` — `register()` sets `state.pid = Some(pid)` directly from the client-supplied `Register { pid, .. }` frame field. **No cross-check against the OS-level peer credentials of the socket exists.** The broker trusts whatever `pid` the client claims (subject only to the `pid != 0` guard at line 274-280). This is the load-bearing fact for Design A: a process may legitimately register with its *own* real PID for a name that is not "itself" in any conversational sense — nothing prevents a gateway process from registering `bob` and `carol` from two sockets, both carrying the gateway's own PID.

### Design A feasibility — CONFIRMED, "zero famp-bus change" holds `[VERIFIED]`

There is a pre-existing, already-shipped, different feature also called "D-10" in code comments: `Hello { bind_as: Option<String> }` (`crates/famp-bus/src/proto.rs:119`, handled at `crates/famp-bus/src/broker/handle.rs:180-260`). **This is NOT the mechanism to use for backing a remote principal.** `bind_as` makes a connection function as a *read/write-through proxy* to a canonical holder that is **already registered elsewhere with its own live PID** (`proxy_holder_alive`, `identity.rs:48-54`, re-checked at Hello time and on every identity-required op). It is what every one-shot CLI subcommand uses today (`famp send`, `famp inbox list`, `famp await`, `famp join/leave`, `famp sessions --me`, `famp whoami`) to operate as an MCP-session-registered identity without re-registering — see `crates/famp/src/bus_client/mod.rs:1-21` and `crates/famp/tests/broker_proxy_semantics.rs` (an existing wire-level test suite that already SIGKILLs a canonical holder and asserts a subsequent `bind_as` proxy connect is refused — a direct, reusable template for Phase 7's own liveness tests). Using `bind_as` for the gateway would require *something else* to already hold a live canonical registration for each remote principal — which doesn't exist; there is no local process backing a remote agent.

**The correct mechanism is plain `Register`, done N times, each carrying the gateway's own PID:**

1. Gateway opens a UDS connection to `~/.famp/bus.sock` (or `FAMP_BUS_SOCKET`).
2. Sends `Hello { bus_proto: BUS_PROTO_VERSION, bind_as: None }` — the canonical-holder shape.
3. Sends `Register { name: "<remote-principal-name>", pid: std::process::id(), cwd: None, listen: true }`.
4. Repeats steps 1-3 on a **separate** connection for each additional remote principal it backs — `register()` rejects a second `Register` on an already-registered/handshaked connection with `NameTaken`-adjacent semantics are per-connection, not needed here since each principal gets its own socket.
5. Broker's existing `tick()` sweep and per-message routing then work unmodified: `is_alive(gateway_pid)` is `true` for every one of these N connections as long as the gateway process runs (same PID reported N times — no uniqueness check on `pid`, confirmed at `handle.rs:262-376`, no pid-collision guard exists), and `false` for all N the moment the gateway process exits.

**No `famp-bus` source change is required.** This confirms the STATE.md-locked architectural invariant #2 ("Liveness fix must not require a `famp-bus` change") is achievable exactly as designed.

**One thing to verify in-phase, not assumed here:** whether `famp register`'s CLI-side auto-spawn-broker path (`crates/famp/src/bus_client/spawn.rs`) is something `famp-gateway` should reuse or bypass (the gateway should almost certainly assume a running daemon per the v0.11 architecture and fail loud if unreachable, rather than auto-spawning a broker — auto-spawn is a CLI convenience for interactive/session use, not appropriate for a long-running service process). Recommend `famp-gateway` connects directly via `UnixStream::connect` (or reuses `famp::bus_client::BusClient::connect`, since `famp` is a lib crate — `crates/famp/src/lib.rs:77` `pub mod bus_client;` — and could be added as a path dependency) WITHOUT the spawn-on-absent fallback.

### Design B fallback (heartbeat/lease) — brief, only if Design A proves infeasible in-phase

If some in-phase blocker rules out Register-with-gateway-PID (none currently visible), the minimal fallback is a wire-level addition: extend `ClientState` (`crates/famp-bus/src/broker/state.rs:7-32`) with a `lease_expires_at: Option<Instant>` field, add a new `BusMessage::Heartbeat { }` (or overload `SetListen`) the gateway sends every N seconds per proxied connection, and change `tick()`'s liveness predicate (`handle.rs:974-982`) to OR together `is_alive(pid)` with `lease_expires_at.is_some_and(|exp| now < exp)`. This DOES require a `famp-bus` change (new wire message + state field + sweep-predicate edit) and a `just install` + broker restart to deploy. Given Design A's confirmed feasibility above, this path should not be needed; document it in the phase but do not build it speculatively.

### GW-04 — mailbox / cursor isolation model `[VERIFIED]`

Routing today is entirely by `name: String` (or `#channel` string) — `MailboxName::Agent(name)` maps 1:1 to an on-disk file `mailboxes/<name>.jsonl` (`crates/famp/src/cli/broker/mailbox_env.rs:57-64`). Cursors (`await_offsets`, `inbox_offsets` on `ClientState`, `state.rs:7-32`) are keyed per-connection per-mailbox-name. Because each proxied remote principal in Design A is registered on its **own** UDS connection with its **own** `ClientId`, there is no shared mutable state between two proxied principals inside the broker at all — cross-talk would require the *gateway itself* to misroute an inbound bus delivery to the wrong outbound wire session. **The isolation the gateway must build is entirely internal**: a `HashMap<PrincipalName, ConnectionHandle>` (each handle owning its UDS `BusClient` + its own outbound-to-remote-machine channel), with the invariant "an `Await`/`Inbox` result read from connection X is written only to X's own outbound channel." No shared demux queue across principals.

### Recommended Project Structure (skeleton)

```
crates/famp-gateway/
├── Cargo.toml            # workspace member; deps: famp-bus, famp (path, for BusClient reuse) or roll a minimal client, tokio, thiserror
├── src/
│   ├── lib.rs             # pub mod principal; pub mod error;
│   ├── principal.rs       # ProxiedPrincipal: owns one UDS connection registered
│   │                       #   with the gateway's own PID; register/deregister lifecycle
│   ├── registry.rs         # GatewayRegistry: HashMap<String, ProxiedPrincipal>,
│   │                       #   the single demux point enforcing GW-04 isolation
│   └── error.rs            # GatewayError (thiserror)
└── tests/
    └── liveness.rs          # LIVE-01/02/GW-04 integration tests (see Verification Surface)
```

Add to root `Cargo.toml` `[workspace] members`:
```toml
"crates/famp-gateway",
```
placed after `famp-transport-http` per the "v1.0 federation internals" comment block already there (`Cargo.toml:16-19`).

### Anti-Patterns to Avoid

- **Using `bind_as` (proxy) connections to back remote principals.** That mechanism assumes a *separately-live* canonical holder already exists — there is none for a remote agent. Using it here would be a category error, not a liveness fix.
- **Building a heartbeat/lease system before confirming Design A fails.** STATE.md already locks Design A as the default; Design B is fallback-only, not parallel work.
- **Gateway auto-spawning a broker.** The v0.11 daemon model (`famp daemon install`) makes broker-presence a runtime precondition, not something a Layer-2 service should paper over.
- **One shared UDS connection multiplexing N principals via repeated `bind_as` rebinds.** Rebinding on one connection cannot work — `bind_as` is set once at `Hello` time and the connection's `ClientState` is fixed for its lifetime; there is also no "switch identity" frame. One connection per principal is required, not optional.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Liveness detection | A new heartbeat/lease protocol (Design B) speculatively | The existing `kill(pid,0)` sweep, fed the gateway's real PID | It already works, is already tested (`broker_proxy_semantics.rs`), and needs zero wire change |
| Per-principal message isolation | A gateway-side mailbox/cursor re-implementation | The broker's existing per-name mailbox file + per-connection cursor model | Isolation is already structural; the gateway only needs to not cross-wire its own routing table |
| UDS framing / handshake | A hand-rolled length-prefixed JSON codec | `famp_bus::codec` + `BusMessage`/`BusReply` (already used by `famp::bus_client::BusClient`) | Byte-exact wire compatibility with the broker is mandatory; a second implementation risks drift |

**Key insight:** every piece of this phase's "hard problem" (liveness across a fork the spike diagnosed as transport-independent) is already solved by an existing, unrelated feature's side effect (no pid-uniqueness constraint in `register()`). The engineering work in Phase 7 is almost entirely: (1) prove this via tests, (2) scaffold the crate that opens N such connections and keeps a demux table, (3) write the tests that pin the behavior under SIGKILL/exit.

## Runtime State Inventory

Not applicable — Phase 7 is greenfield addition (new crate) plus behavior confirmation, not a rename/refactor/migration. No stored data, live service config, OS-registered state, secrets, or build artifacts carry a name that is changing.

## Common Pitfalls

### Pitfall 1: Confusing `bind_as` proxy semantics with Register-as-gateway-PID semantics
**What goes wrong:** An implementer skims for "D-10" / "bind_as" in the codebase (it's everywhere — CLI, tests, comments) and concludes the gateway should connect with `bind_as: Some(remote_principal)`.
**Why it happens:** Both mechanisms are labeled "D-10" in code comments (from different, unrelated quick-tasks), and both involve identity binding at Hello time.
**How to avoid:** `bind_as` requires `proxy_holder_alive` to already be true (line `identity.rs:48-54`) — i.e., someone else already registered that name with a live PID. For a remote principal there is no "someone else." Use plain `Register` with the gateway's own PID instead.
**Warning signs:** A gateway prototype that fails at Hello with `HelloErr{NotRegistered, "bind_as identity '...' is not registered"}` (`handle.rs:204-212`) is hitting exactly this confusion.

### Pitfall 2: macOS has no `setsid`; broker/register-spawn helper patterns don't port blindly
**What goes wrong:** `crates/famp/src/bus_client/spawn.rs` uses a Unix-only `pre_exec(setsid)` pattern (per the project's own prior documented landmine). A gateway test harness that copies broker-spawn patterns for spawning test brokers may hit the same platform gap if it assumes `setsid` exists everywhere.
**Why it happens:** it's a documented FAMP landmine already (`ChildGuard` / broker-spawn test conventions).
**How to avoid:** For Phase 7 tests, spawn a test broker exactly the way `crates/famp/tests/broker_proxy_semantics.rs` already does — via `Command::cargo_bin("famp").args(["broker","--socket",...])` — not via the interactive spawn-on-absent path.

### Pitfall 3: Forgetting `ChildGuard` on any test that spawns `famp register` or `famp broker` as a child process
**What goes wrong:** A test panics mid-assertion, the spawned broker/register child leaks, and a subsequent test run finds a stale UDS socket / orphan holder.
**Why it happens:** documented FAMP incident class (project memory: "Test ChildGuard convention").
**How to avoid:** Every Phase 7 test that spawns a child MUST wrap it in `ChildGuard` (`crates/famp/tests/common/child_guard.rs`, already used by `broker_proxy_semantics.rs` and `child_guard_reaps.rs`) — RAII kill+wait on drop.

### Pitfall 4: Testing LIVE-02 reaping with a fixed `sleep()` instead of driving the tick deterministically
**What goes wrong:** A test asserts "reaped within 1 tick" using `std::thread::sleep(Duration::from_secs(2))` against a real broker subprocess — flaky under CI load (scheduling jitter, or the broker's `tokio::time::interval` missing a tick under load).
**Why it happens:** the sweep interval is wall-clock-driven in production (`TICK_INTERVAL`, `Duration::from_secs(1)`), which is inherently timing-sensitive to test against black-box.
**How to avoid:** Prefer **pure-broker unit tests** (not subprocess integration tests) using `Broker<E>` directly with a `FakeLiveness` env (`crates/famp-bus/src/liveness.rs:18-41`, already exists) and synthetic `Instant` ticks — this is exactly the pattern used elsewhere in the codebase (e.g., `crates/famp-bus/src/broker/handle/tests.rs`) for other timing-sensitive sweep behavior (the 23h-await test). For subprocess-level (real SIGKILL) confirmation, follow `broker_proxy_semantics.rs`'s existing pattern (poll with a generous timeout, not a fixed sleep) rather than inventing a new one.

### Pitfall 5: `just install` / broker restart needed after any famp-bus OR broker-CLI change
**What goes wrong:** If Design A needs no `famp-bus` change (expected) but the gateway skeleton still needs to compile/link correctly against the currently-installed broker's wire protocol, forgetting `just install` after any change to `crates/famp/src/cli/broker/` leaves the *running* daemon on stale code while tests pass against a freshly-built one.
**Why it happens:** documented FAMP landmine — "the installed `~/.cargo/bin/famp` is what every agent session reads; `target/release/famp` is not the deployment target" (CLAUDE.md, project convention).
**How to avoid:** If Phase 7 touches anything under `crates/famp/src/cli/broker/` (it likely won't, if Design A holds and needs zero broker-side change) run `just install` + `famp daemon restart` before any manual verification. If Phase 7 truly touches zero broker-side code (the expected outcome), no restart is needed — but this must be explicitly confirmed, not assumed, since "zero famp-bus change" was the recommendation, not yet a fact verified against the actual implementation.

### Pitfall 6: `cargo nextest -p famp` list-phase hang
**What goes wrong:** `cargo nextest run -p famp-gateway` (or `-p famp`, if gateway tests land there) may stall in the test-binary `--list` phase.
**Why it happens:** documented FAMP-wide flake (project memory: "cargo nextest -p famp hangs").
**How to avoid:** fall back to `cargo test -p famp-gateway --lib` / `--test <name>` if nextest stalls.

## Code Examples

### Existing wire-level test template to copy for Phase 7's own liveness tests
```rust
// Source: crates/famp/tests/broker_proxy_semantics.rs (existing, shipped test)
// Pattern: spawn a real broker subprocess, register a holder, SIGKILL it,
// then assert the broker's liveness sweep reaps it. Phase 7's LIVE-01/02
// tests should follow this exact shape but register N names all carrying
// the TEST PROCESS's own pid (standing in for "the gateway"), then kill
// that one process and assert all N are reaped.
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

### The register-as-gateway pattern (new — Phase 7 must implement this shape)
```rust
// Not yet in the codebase — the core Phase 7 pattern, wire-compatible with
// the existing BusMessage/BusReply enums (famp-bus/src/proto.rs).
use famp_bus::{BusMessage, BusReply, BUS_PROTO_VERSION};

// One of these per proxied remote principal, on its OWN UnixStream:
let hello = BusMessage::Hello { bus_proto: BUS_PROTO_VERSION, client: /* impl-defined */, bind_as: None };
// ... send hello, await HelloOk ...
let register = BusMessage::Register {
    name: remote_principal_name.clone(),
    pid: std::process::id(),   // the GATEWAY's own real, live PID — not the remote's
    cwd: None,
    listen: true,
};
// ... send register, await RegisterOk ...
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| v0.8 per-identity TLS listener mesh (federation-era) | v0.9+ single UDS broker, `kill(pid,0)` liveness | 2026-05-04 (v0.9 ship) | The liveness fork this phase resolves is a *consequence* of the v0.9 same-host simplification — v0.8's transport had no such fork (each agent WAS a listener) |
| Cross-host socat/relay spike (2026-07-19) | Design A local-proxy (this phase) | 2026-07-23 (roadmap) | Confirmed by spike: reaping is transport-independent; the fix must live at the "what PID does the broker see" layer, not the transport layer |

**Deprecated/outdated:** none specific to this phase; the `famp-transport-http`/`famp-keyring` crates are intentionally preserved-not-rebuilt (tag `v0.8.1-federation-preserved`).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | OS-level UDS socket close (gateway process death) triggers the broker's `disconnect()` handler near-instantly, faster than the 1s tick sweep, for the common case (not just SIGKILL) | Design A feasibility | If wrong, LIVE-02 still holds (the 1s sweep is the guaranteed backstop, already covered by an existing test pattern) — this only affects *how fast*, not *whether*, reaping happens. Low risk. |
| A2 | `famp-gateway` should NOT reuse `bus_client::spawn`'s auto-spawn-broker fallback and should instead fail loud if the daemon is unreachable | Design A feasibility, Anti-Patterns | If wrong (i.e., some UAT expects gateway to also work standalone without `famp daemon install`), an extra task would be needed in-phase to wire the spawn fallback. Medium — worth confirming with Ben/CONTEXT before implementation, not blocking research. |
| A3 | `famp-gateway` can depend on `famp` (the umbrella crate) as a path dependency to reuse `BusClient`, rather than depending directly on `famp-bus` and rolling its own minimal UDS client | Recommended Project Structure | If wrong (e.g., `famp` pulls in clap/CLI-only deps unsuitable for a service crate), the gateway would need to reimplement a thin `BusClient`-equivalent directly against `famp-bus`. Low effort either way; flag for planner to pick during task-breakdown. |

**If this table is empty:** N/A — see above; none of these three assumptions materially threaten the Design A recommendation itself, only implementation-detail choices.

## Open Questions (RESOLVED at plan-phase 2026-07-23)

**Resolution 1 (bin):** RESOLVED → include a minimal killable `[[bin]] famp-gateway`. LIVE-02's process-exit test needs a genuine OS process to SIGKILL. Adopted in plan 07-01.
**Resolution 2 (listen flag):** RESOLVED → gateway-registered principals use `listen: false` (the gateway is the delivery mechanism, not a Stop-hook session). Adopted in plan 07-01 (`ProxiedPrincipal::register`).

1. **Does `famp-gateway` need a `[[bin]]` in Phase 7, or is a lib-only skeleton sufficient?**
   - What we know: the phase description says "skeleton exists to back concurrent remote principals" and explicitly scopes OUT the full federation transport (Phase 8/9). No CLI surface (`famp gateway ...` subcommand) is named in any Phase 7 success criterion.
   - What's unclear: whether verification (LIVE-01/02/GW-04) requires a runnable binary the tests can spawn as a subprocess (matching the `broker_proxy_semantics.rs` pattern), or whether pure-library integration tests (spinning up the registry in-process against a real broker subprocess) suffice.
   - Recommendation: plan for a lib crate with a thin test-only or minimal `[[bin]]` entry point sufficient for integration tests to spawn a real "gateway" process for the LIVE-02 process-exit test (this test fundamentally needs a real OS process to kill).

2. **Should the gateway's per-principal `listen: true` on Register interact with the existing Stop-hook / MCP listen-mode wake path?**
   - What we know: `Register { listen: bool }` today controls whether a Claude-Code-session-style listen-mode wake applies (`crates/famp/src/cli/mcp/session.rs`, `famp-await.sh`). A remote principal proxied through the gateway is not a Claude Code session.
   - What's unclear: whether `listen: true` is even meaningful/desired for a gateway-backed principal, or whether it should register `listen: false` (since the gateway itself handles forwarding, not a Stop-hook).
   - Recommendation: default to `listen: false` for gateway-registered principals in Phase 7 (the gateway is the delivery mechanism, not an interactive session waiting on a Stop hook) — flag as a discretion point for discuss-phase/plan-phase, not a locked research finding.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | entire phase | ✓ | per `rust-toolchain.toml` (workspace) | — |
| `famp` daemon running locally | manual verification of LIVE-01/02/GW-04 | Assume ✓ per v0.11 (`famp daemon install`) | 0.11.0 | `famp broker --no-idle-exit` bridge if daemon absent |
| `cargo-nextest` | test execution | ✓ (workspace convention) | — | plain `cargo test` if nextest list-phase hangs (documented pitfall) |

**Missing dependencies with no fallback:** none identified.
**Missing dependencies with fallback:** nextest hang has a documented `cargo test` fallback.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo nextest` (workspace convention) + plain `cargo test` fallback |
| Config file | `.config/nextest.toml` |
| Quick run command | `cargo test -p famp-gateway --lib` (new crate) / `cargo nextest run -p famp-bus --lib` (if any famp-bus pure-broker tests are added for Design A confirmation) |
| Full suite command | `cargo nextest run --workspace` (or `just test`) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LIVE-01 | N proxied principals, registered with one PID, all show alive across a Tick sweep while that PID lives | unit (pure `Broker<E>` + `FakeLiveness`) | `cargo test -p famp-bus --lib live01_gateway_pid_survives_sweep` | ❌ Wave 0 — new test, pattern mirrors `crates/famp-bus/src/broker/handle/tests.rs` |
| LIVE-02 | Killing the backing process reaps all N proxied principals within one sweep, no orphans | integration (real subprocess, SIGKILL) | `cargo test -p famp-gateway --test liveness live02_gateway_exit_reaps_all_principals -- --nocapture` | ❌ Wave 0 — new test, pattern mirrors `crates/famp/tests/broker_proxy_semantics.rs` (SIGKILL + poll-with-timeout) |
| GW-04 | Two concurrent proxied principals under one gateway process; message to A never appears in B's mailbox | integration (real broker + 2 registered connections + `famp inspect messages`) | `cargo test -p famp-gateway --test liveness gw04_no_cross_talk_between_proxied_principals` | ❌ Wave 0 — new test |

### Sampling Rate
- **Per task commit:** `cargo test -p famp-gateway --lib` (fast unit feedback)
- **Per wave merge:** `cargo nextest run -p famp-bus -p famp-gateway -p famp`
- **Phase gate:** `just ci` full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/famp-gateway/` crate scaffold + workspace `Cargo.toml` member entry — does not exist yet
- [ ] `crates/famp-bus/src/broker/handle/tests.rs` — add a pure-broker LIVE-01 test using the existing `FakeLiveness` harness (no new test file needed, extend existing)
- [ ] `crates/famp-gateway/tests/liveness.rs` — new integration test file for LIVE-02 + GW-04 (subprocess-level, following `broker_proxy_semantics.rs`'s `ChildGuard` + poll pattern)
- [ ] `crates/famp-gateway/tests/common/child_guard.rs` — reuse (copy or extract to shared test-util) the existing `ChildGuard` helper from `crates/famp/tests/common/child_guard.rs`

## Security Domain

`security_enforcement` not set to `false` in `.planning/config.json` — treated as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-------------------|
| V2 Authentication | No (local UDS, same-host trust boundary; cross-host auth is Phase 8's WIRE/TRUST scope) | — |
| V3 Session Management | Partial — gateway-held UDS connections ARE sessions in the broker's model | Reuse existing `ClientState`/disconnect lifecycle unmodified; no new session-management surface introduced |
| V4 Access Control | Yes — GW-04's no-cross-talk requirement IS an access-control property | Per-principal `HashMap` keying inside the gateway; broker's existing per-name mailbox isolation |
| V5 Input Validation | Partial — `pid` field on `Register` is unvalidated against OS peer credentials today | Out of scope to fix in Phase 7 (would be a `famp-bus` behavior change beyond Design A's zero-change goal); note as accepted local-trust-boundary risk, not a new one introduced by this phase |
| V6 Cryptography | No | Untouched — Phase 8 scope |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|----------------------|
| A malicious local process claims an arbitrary `pid` on `Register` to impersonate liveness of a process it doesn't own | Spoofing | Already an accepted same-host trust-boundary risk (no peer-credential check exists today, pre-dating this phase); Phase 7 does not widen this surface — it uses the *gateway's own real pid*, same as any other legitimate `famp register` caller. Do not treat this as a new vulnerability introduced by Phase 7; it is a pre-existing local-trust assumption of the v0.9+ bus model. |
| Gateway crash leaves stale mailbox files for proxied principals | Denial of Service (data) | Mailbox files are append-only JSONL, unaffected by registration reaping — LIVE-02 only removes the live `ClientState`/holder slot, not historical mailbox content; a re-registering gateway or holder drains from the persisted file exactly like any reconnecting agent today |
| Two proxied principals sharing one gateway process/PID create ambiguity in `famp inspect identities`' PID column | Repudiation (observability) | Expected and benign — `famp inspect identities` will correctly show N rows with the same PID; this is the intended visible signature of Design A and should be documented, not treated as a bug |

## Sources

### Primary (HIGH confidence — direct codebase reads, 2026-07-23)
- `crates/famp-bus/src/broker/handle.rs` (1178 lines, read in full for relevant sections) — `tick()` (973-991), `disconnect()` (920-971), `register()` (262-376), `hello()` (180-260), `handle_wire()`/dispatch (26-104)
- `crates/famp-bus/src/broker/identity.rs` (105 lines, read in full) — `effective_identity`, `proxy_holder_alive`, `canonical_holder_id`, `resolve_op_identity`
- `crates/famp-bus/src/broker/state.rs` — `ClientState` struct fields (`pid`, `bind_as`, `name`, cursors)
- `crates/famp-bus/src/liveness.rs` (41 lines, read in full) — `LivenessProbe` trait, `AlwaysAliveLiveness`, `FakeLiveness` test doubles
- `crates/famp/src/cli/broker/mailbox_env.rs` (production `is_alive` impl, lines 96-156 read) — the real `kill(pid,0)` call site
- `crates/famp/src/cli/broker/mod.rs` (tick interval + driving loop, lines around 51-53, 243-307)
- `crates/famp/src/bus_client/mod.rs` (BusClient, `bind_as` doc comments, lines 1-60)
- `crates/famp/tests/broker_proxy_semantics.rs` (existing wire-level test suite proving liveness-under-SIGKILL semantics — direct template for Phase 7 tests)
- root `Cargo.toml` (workspace members, dependency versions)
- `crates/famp-transport-http/Cargo.toml`, `crates/famp-keyring/Cargo.toml` (confirms these crates exist, are not touched)
- `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, `.planning/ROADMAP.md` (phase scope, locked architectural invariants)

### Secondary (MEDIUM confidence)
- None used — all findings above are grounded in direct primary reads; no external web search was needed since this phase's domain is entirely internal-codebase.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new external dependencies, all path deps already in workspace
- Architecture (Design A mechanism): HIGH — traced end-to-end through actual source, cross-referenced against an existing shipped test suite exercising the adjacent (but distinct) `bind_as` liveness path
- Pitfalls: HIGH — five of six pitfalls are directly sourced from this project's own documented prior incidents (auto-memory), one (nextest list-hang) independently confirmed as a known FAMP-repo flake

**Research date:** 2026-07-23
**Valid until:** 30 days (stable internal codebase; re-verify file:line citations if Phase 8/9 land first or if any `famp-bus` refactor touches `handle.rs`/`identity.rs` before Phase 7 executes)
