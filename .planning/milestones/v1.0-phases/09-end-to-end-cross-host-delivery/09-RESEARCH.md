# Phase 9: End-to-End Cross-Host Delivery - Research

**Researched:** 2026-07-27
**Domain:** Rust — wiring a UDS local bus (famp-bus) to a preserved axum/rustls HTTP transport (famp-transport-http) through a Layer-2 gateway (famp-gateway), composing Phase 7 liveness + Phase 8 signed envelope/trust primitives.
**Confidence:** HIGH (all findings are direct source citations from this repository; no external library research was needed — this phase composes existing in-repo primitives, it does not add new dependencies)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Symmetric "back the remote principal locally" model. Each gateway `back()`s the *remote* peer's principal as a local stand-in on its own bus (reusing Phase 7's `GatewayRegistry::back`/`ProxiedPrincipal`). On machine A, the gateway backs principal `bob`; when A's local agent sends to `bob`, the message lands in the gateway-backed mailbox, the gateway drains it, wraps + signs the cross-host envelope, and ships it over HTTP to B's gateway. B's gateway verifies it and delivers into B's real `bob` mailbox. *Rejected:* teaching the local broker to route by `authority`/domain (a `famp-bus` change).
- **D-02:** `to_domain` → remote gateway URL via a small peer-endpoint map, sibling to the pinned keyring (`~/.famp/gateway/peers.toml` or `--peer <domain>=<url>` — planner picks against the clap tree + `paths.rs`/`home.rs`). Own-machines-first ⇒ hand-entered endpoints, no discovery/directory.
- **D-03:** Drain the backed principal's mailbox via its existing UDS `ProxiedPrincipal` connection; ship byte-exact. For each drained envelope: resolve `to_domain` → URL (D-02), populate + sign the federation fields if not already cross-host-signed, POST to the remote gateway's inbox. The gateway is content-transparent — never rewrites `task_id`, `MessageClass`, or body; only adds the outer federation fields + INV-10 signature over the single canonical form.
- **D-04:** Reuse `famp-transport-http`'s axum server + TLS + body-limit scaffolding, but route the raw body through the gateway's own `verify_inbound` against the gateway peers keyring — do NOT double-verify via the transport's `FampSigVerifyLayer`. Mount the preserved `build_router`/`INBOX_ROUTE` (`POST /famp/v0.5.1/inbox/{principal}`) + rustls server for HTTP/TLS plumbing, but the handler feeds the body to `verify_inbound` (not the transport's middleware keyring). Add `famp-transport-http` to `famp-gateway/Cargo.toml`. *Rejected:* rolling a fresh axum surface; letting `FampSigVerifyLayer` verify against a second keyring.
- **D-05:** Verified envelope → local bus via the backed *sender* stand-in. On B, the gateway backs the remote sender principal (`alice`) as a local stand-in and uses that `ProxiedPrincipal`'s UDS connection to `send` the verified envelope to the local recipient (`bob`). Keeps delivery on the existing bus `send` path — the local broker's normal task-record machinery advances the FSM. A rejected envelope produces zero bus writes and surfaces as an HTTP 4xx.
- **D-06:** The gateway is FSM-transparent; both local brokers own their own FSM. Real signed envelopes (unchanged `task_id`/`MessageClass`) flow onto each local bus; each side's existing broker/taskdir advances its own task record with no gateway-side FSM logic. Verification asserts the terminal state on both sides via `famp inspect tasks --id <task_id>`.
- **D-07:** Two-process loopback E2E is the Phase 9 gate. Two brokers (distinct `FAMP_HOME`/socket paths), two gateways, real signed envelopes over loopback HTTPS, driving a full `request → commit → deliver → ack` cycle, asserting terminal FSM state on both sides via `famp inspect tasks`. Uses `ChildGuard` RAII for every spawned broker/gateway child. The live two-physical-machine run, `just ci`-gated E2E (TEST-02), and setup guide (DOC-04) are Phase 10.
- **D-08:** TLS is channel encryption, not the trust boundary — the Ed25519 envelope signature is. Reuse `famp-transport-http`'s existing rustls setup (`tls.rs`/`tls_server.rs`, the fixture-cert pattern from `cross_machine_two_agents`). Own-machines-first ⇒ self-signed/pinned cert, no PKI.

### Claude's Discretion

- Exact peer-endpoint config surface (D-02): `~/.famp/gateway/peers.toml` vs `--peer <domain>=<url>` flags vs both — planner picks against the clap tree in `cli/mod.rs` + `paths.rs`.
- Which `famp-transport-http` layers to keep vs. bypass on ingress (D-04) — planner reconciles against `server.rs`/`middleware.rs`. **This research recommends Option A (§3): a fully separate, gateway-owned router that does not call `build_router()` at all**, since that function unconditionally mounts `FampSigVerifyLayer`.
- Cert provisioning for own-machines loopback (D-08) — planner confirms against `tls.rs`/the `cross_machine_two_agents` fixture certs. **This research confirms the fixture certs still exist** at `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}`.
- Whether outbound drain uses `famp await`-style blocking or a poll loop per backed principal — planner picks against the `bus_client` await/inbox API. **This research recommends `Await` in a loop** (push-latency for free via the broker's existing wake-on-send logic) over polling `Inbox` on a timer.

### Deferred Ideas (OUT OF SCOPE)

- Live two-physical-machine run + `just ci`-gated two-process E2E — Phase 10 (TEST-02).
- Deferred federation test triage (`crates/famp/tests/_deferred_v1/`, ~27 tests) — Phase 10 (TEST-01).
- Two-machine setup guide (bind address, out-of-band key exchange, connect/verify) — Phase 10 (DOC-04).
- Active nonce/replay cache + expiry rejection — v1.1 (INGRESS-01); fields ride the wire now, enforcement is the public-internet layer.
- Public-internet dumb relay, signed peer directory, no-implicit-peering, inbound-taint provenance — v1.1 (RELAY-01/DIR-01/PEER-01/TAINT-01).
- FAMP-Sec capability/approval/tool-admission plane — v2.0+ (SEC-01..N).
- Peer/endpoint discovery — v1.1 directory; Phase 9 uses a hand-configured domain→URL map.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GW-01 | A user registers an agent on machine A, addresses an agent on machine B by name/principal, and the message is delivered to B's local bus. | §1 (`ProxiedPrincipal`/`GatewayRegistry` gaps), §2 (`BusMessage::Send`/`Inbox`/`Await` primitives), §3 (D-04 ingress seam), §4 (`verify_inbound_any` gap), §5 (sign site), §7 (E2E harness pattern) |
| GW-02 | An agent on machine B can reply within the same task/conversation, and the reply is delivered back to machine A. | Same mechanism as GW-01, symmetric direction — §1–§5 apply identically since D-01/D-05 are direction-agnostic (each gateway backs whichever principal is remote to it) |
| GW-03 | A full task exchange (`request → commit → deliver → ack`) completes across the two machines with the task FSM advancing correctly on both sides. | §6 (critical correction: no persisted FSM/taskdir — `famp inspect tasks` derives state live from mailbox content; content-transparent relay is sufficient), §3 D-03 note on byte-exact `task_id`/`class`/`body` preservation |

</phase_requirements>

## Summary

Phase 9 wires four already-built pieces together: (1) Phase 7's `GatewayRegistry`/`ProxiedPrincipal`, which today can only `register()` a proxied principal and hold the connection open — it has **no send/drain capability yet**; (2) Phase 8's `verify_inbound`, a pure function that is generic over **one** body class `B` — Phase 9's HTTP ingress will see mixed classes (request/commit/deliver/ack) and needs a class-dispatching wrapper; (3) the preserved `famp-transport-http` axum router, whose `build_router()` always mounts `FampSigVerifyLayer` against its own middleware keyring — D-04 requires bypassing that layer and feeding raw bytes to the gateway's `verify_inbound` instead, so Phase 9 cannot call `build_router()` unmodified; (4) the local bus `BusMessage::Send`/`Inbox`/`Await` wire ops, which already support everything the drain/deliver flow needs — `Send` does not check that the envelope's `from` matches the connection's registered name, so a `ProxiedPrincipal`'s connection can deliver on behalf of any principal it fronts.

The single most important correction to the CONTEXT.md's framing: **there is no persisted "taskdir" or broker-side FSM record.** `famp inspect tasks --id <task_id>` derives FSM state live, on every call, by scanning the recipient's mailbox JSONL for envelopes whose `causality.ref` / `body.details.task` / `id`-of-a-new-task matches, then classifying each envelope's state from `(class, body.details.mode, body.details.terminal, body.details.action)` (`crates/famp-inspect-server/src/parse.rs::derive_fsm_state`). GW-03 is proven the instant the right envelope classes land in each mailbox in the right order — there is no FSM object to "advance," only mailbox content to relay byte-exact.

Also load-bearing: Phase 8's own docs state the trust model is **one signing key per remote principal name** (`crates/famp/src/cli/peer/mod.rs` docstring) — the single gateway keypair at `~/.famp/gateway/identity.ed25519` is exported under exactly one `--as <principal>` name at a time, and a second distinct principal on the same peer keyring needs its own keypair (deferred to v1.1). Phase 9's E2E test should use exactly one named agent per host side (e.g. `alice`@A, `bob`@B) to stay inside this documented limitation.

**Primary recommendation:** Add drain/send methods to `ProxiedPrincipal` (mutable `send_recv` pass-through), build the outbound loop as "drain via `Await`/`Inbox` on the backed connection → mutate the raw drained `serde_json::Value` in place to add federation fields → `famp::sign_value` with the gateway's persisted key → HTTP POST," and build inbound ingress as a **hand-assembled** axum route (NOT `famp_transport_http::build_router`) that extracts raw `Bytes`, calls a new class-dispatching `verify_inbound_any`, then `Send`s the verified bytes on the backed *sender* stand-in's connection.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Outbound envelope drain (mailbox → wire) | API/Backend (`famp-gateway` process) | Database/Storage (local UDS mailbox) | Gateway is a standalone service process reading its own backed principal's mailbox via the bus protocol — not a browser/frontend concern |
| Cross-host HTTP transport (TLS, routing) | API/Backend (`famp-transport-http` inside `famp-gateway`) | — | Preserved axum/rustls crate wrapped by the gateway process; no UI tier involved |
| Ingress signature verification (`verify_inbound`) | API/Backend (`famp-gateway`) | — | Pure function called from the axum handler before any bus write; this is the v1.0 trust boundary (INV-10), analogous to an API auth middleware |
| Inbound envelope delivery (verified bytes → local mailbox) | API/Backend (`famp-gateway`) → Database/Storage (local UDS mailbox write via `BusMessage::Send`) | — | Gateway acts as a privileged local-bus client on behalf of the verified remote sender |
| Task FSM state derivation (`famp inspect tasks`) | API/Backend (`famp-inspect-server`, invoked via broker `Inspect` RPC) | — | Read-only aggregation over mailbox JSONL; no separate FSM persistence tier exists |
| Peer-endpoint config (domain → URL map, D-02) | API/Backend (gateway CLI/config layer, `~/.famp/gateway/`) | — | Operator-maintained static config, same tier as the existing peer keyring |

## Standard Stack

This phase adds **zero new external dependencies**. Every crate needed already exists in the workspace at the pinned version below (all `[VERIFIED: workspace Cargo.toml]` — read directly from `/Users/benlamm/Workspace/FAMP/Cargo.toml` and `famp-gateway/Cargo.toml`, not fetched from a registry).

### Core (already-pinned workspace versions)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | 0.8.8 | HTTP routing for the inbound listener | Already used by `famp-transport-http`; Phase 9 reuses `INBOX_ROUTE` constant and body-limit pattern |
| `reqwest` (rustls-tls-native-roots) | 0.13.2 | HTTP client for outbound POST to peer gateway | `HttpTransport::send` already implements this exact POST; Phase 9 reuses `HttpTransport` client-side as-is |
| `rustls` | 0.23.38 | TLS for the loopback HTTPS hop (D-08) | `famp-transport-http::tls`/`tls_server` already wrap this; reuse fixture certs at `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` |
| `famp-crypto` (workspace) | 0.11.0 | `sign_value`/`verify_value`, re-exported via `famp::{sign_value, verify_value, ...}` | Already a transitive dep of `famp-gateway` via `famp`; add direct dep only if the planner wants an explicit import path |
| `famp-envelope` (workspace) | 0.11.0 | `AnySignedEnvelope`, `EnvelopeView`, `peek_sender`, `WireEnvelope` field shapes | Already a direct `famp-gateway` dependency (07-03/08-03) |
| `famp-transport-http` (workspace) | 0.11.0 | `build_router`/`INBOX_ROUTE`/`HttpTransport`/`tls`/`tls_server` | **NOT currently a `famp-gateway` dependency — must be added to `famp-gateway/Cargo.toml`** (confirmed by grep: absent from the file read below) |

### Package Legitimacy Audit

Not applicable — this phase adds zero new external (non-workspace) packages. No `npm view`/`cargo search` gate is needed; every crate above is either already a workspace member or already a pinned external dep used unchanged.

## Existing-Code Facts the Planner Needs (cited, not summarized)

### 1. `famp-gateway`'s current surface — what it can and cannot do today

`crates/famp-gateway/src/registry.rs` — `GatewayRegistry`:
```rust
pub async fn back(&mut self, sock: &Path, name: String) -> Result<(), GatewayError>
pub fn get(&self, name: &str) -> Option<&ProxiedPrincipal>   // IMMUTABLE — no get_mut today
pub fn names(&self) -> impl Iterator<Item = &str>
```

`crates/famp-gateway/src/principal.rs` — `ProxiedPrincipal`:
```rust
pub struct ProxiedPrincipal {
    _client: BusClient,   // PRIVATE, unused apart from keep-alive — leading underscore
    name: String,
}
pub async fn register(sock: &Path, name: String) -> Result<Self, GatewayError>
pub fn name(&self) -> &str
```

**Gap Phase 9 must close:** `ProxiedPrincipal._client` is private and the field is prefixed `_` (dead-code-suppressed — it exists ONLY to keep the socket open for liveness). There is **no method that sends or drains anything** on this connection. Phase 9 must:
- Rename `_client` → `client` (or add an accessor) and add `&mut self` methods, e.g. `pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, GatewayError>` delegating to `self.client.send_recv(msg).await.map_err(map_bus_client_err)` (the existing private `map_bus_client_err` fn in the same file already does the `BusClientError → GatewayError` mapping — reuse it).
- Add `GatewayRegistry::get_mut(&mut self, name: &str) -> Option<&mut ProxiedPrincipal>` alongside the existing immutable `get`.

`famp-gateway/main.rs` currently just backs principals then `tokio::signal::ctrl_c().await`s — Phase 9 replaces/augments this park loop with the outbound drain loop(s) + inbound HTTP server task, per CONTEXT.md's own note ("Phase 9 adds the inbound listener + outbound drain loops here").

**Key mechanism confirmed:** `ProxiedPrincipal::register` calls `BusClient::connect_no_spawn(sock, None)` then sends `BusMessage::Register{name, pid, cwd: None, listen: false}` — this makes the gateway's connection the **canonical holder** of `name` (not a `bind_as` proxy). This matters for D-01/D-05: the connection backing `bob` on A *is* `bob`'s live registration; the connection backing `alice` on B (the "backed sender" stand-in per D-05) *is* `alice`'s live registration on B's bus. Both directions use the same `register()` path — no new bus primitive.

### 2. Local-bus `BusMessage`/`BusReply` — the drain (D-03) and send (D-05) primitives already exist

`crates/famp-bus/src/proto.rs:118-203` (`BusMessage`):
```rust
Send { to: Target, envelope: serde_json::Value }
Inbox { since: Option<u64>, include_terminal: Option<bool> }
Await { timeout_ms: u64, task: Option<uuid::Uuid> }
```
`crates/famp-bus/src/proto.rs:205-273` (`BusReply`):
```rust
SendOk { task_id: uuid::Uuid, delivered: Vec<Delivered> }
InboxOk { envelopes: Vec<serde_json::Value>, next_offset: u64 }
AwaitOk { envelopes: Vec<serde_json::Value>, mailbox: MailboxName, next_offset: u64 }
AwaitTimeout {}
```

`crates/famp-bus/src/broker/handle.rs:376-395` (`send` handler) — **critical fact**: the broker's `send()` handler calls `resolve_op_identity(broker, client)` to confirm the *connection* is registered/live, but it does **not** cross-check that the envelope's `from` field matches the connection's registered name:
```rust
// D-10: resolve via effective_identity so a proxy connection can
// send under the bound canonical holder's name. ... the envelope
// already carries `from` from the higher layer.
if resolve_op_identity(broker, client).is_err() { return vec![err(...)]; }
```
This means D-05 (inbound delivery via the backed **sender** stand-in) works with zero new broker code: the gateway's `alice`-backing connection can `send_recv(BusMessage::Send{ to: Target::Agent{name:"bob"}, envelope: <verified JSON Value, from="alice"> })` and the broker delivers it into `bob`'s real mailbox, unconditionally trusting the envelope's own `from` field. The gateway is the only thing standing between "any JSON with `from: alice`" and "delivered as alice" — which is exactly why `verify_inbound` running BEFORE this call is the entire trust boundary (WIRE-01/TRUST-02).

For D-03 (outbound drain), the gateway's `bob`-backing connection (on A) can issue `BusMessage::Await{timeout_ms, task: None}` or `BusMessage::Inbox{since, include_terminal}` on the same connection to pull whatever local senders (`alice`) addressed to `bob`. **Claude's Discretion note from CONTEXT.md ("await-style blocking vs poll loop") is answerable now**: `Await` blocks server-side up to `timeout_ms` and wakes immediately on new mail (broker already implements wake-on-send via `waiting_clients_for_name`, `handle.rs:415-427`) — prefer `Await` in a loop (`loop { await(30_000ms); drain-and-forward }`) over polling `Inbox` on a timer, since `Await` gets push-latency for free from broker logic Phase 5/9-era code already exercises.

### 3. `famp-transport-http` reuse surface — the exact D-04 seam

`crates/famp-transport-http/src/server.rs`:
```rust
pub const INBOX_ROUTE: &str = "/famp/v0.5.1/inbox/{principal}";
pub type InboxRegistry = Mutex<HashMap<Principal, mpsc::Sender<TransportMessage>>>;
pub fn build_router(keyring: Arc<Keyring>, inboxes: Arc<InboxRegistry>) -> Router
```
`build_router` **always** mounts `FampSigVerifyLayer::new(keyring)` as an inner tower layer (outer `RequestBodyLimitLayer::new(1_048_576)` first, per D-C1 ordering comment) — there is no parameter to opt out. **D-04 therefore cannot be satisfied by calling `build_router()` unmodified**: the transport's own keyring-based verify would run BEFORE the gateway's `verify_inbound`, producing exactly the "two sources of trust truth" Phase 8 D-08 rejected.

Two concrete options for the planner (CONTEXT.md marks this "Claude's Discretion" — planner picks):
- **Option A (recommended): hand-assemble a parallel router in `famp-gateway`** reusing only `INBOX_ROUTE` (import the const) and `RequestBodyLimitLayer::new(ONE_MIB)` (re-declare or export the constant), with a handler taking `Path<String>` + `body: axum::body::Bytes` (same axum 0.8 extractor shape already used at `server.rs:69-74`: `async fn inbox_handler(Path(principal_str): Path<String>, State(state): State<ServerState>, Extension(envelope): Extension<Arc<AnySignedEnvelope>>, body: Bytes) -> Result<StatusCode, MiddlewareError>` — Phase 9's handler drops the `Extension` param entirely since there is no middleware stashing a pre-verified envelope) that calls the gateway's own `verify_inbound_any(&body, &gateway_keyring)` directly, then delivers via D-05.
- **Option B: export a `build_router_unverified(inboxes)` from `famp-transport-http`** that omits the `FampSigVerifyLayer` layer, keeping only the body-limit layer — smaller diff to `famp-gateway`, but touches the preserved transport crate (contradicts "wrap, do not rebuild" framing less than Option A, which touches nothing in `famp-transport-http`).

Either way, `famp-transport-http` must be added as a direct `famp-gateway` dependency (confirmed absent from the current `famp-gateway/Cargo.toml`, listed above).

`crates/famp-transport-http/src/transport.rs` — `HttpTransport` client-side send path is reusable as-is for D-03's egress:
```rust
pub fn new_client_only(trust_cert_path: Option<&Path>) -> Result<Self, HttpTransportError>
pub async fn add_peer(&self, principal: Principal, url: Url)   // D-02's runtime sink
fn send(&self, msg: TransportMessage) -> impl Future<Output = Result<(), Self::Error>>
```
`send` POSTs to `{base}/famp/v0.5.1/inbox/{recipient}` and expects HTTP 202 (`StatusCode::ACCEPTED`) — a non-202 becomes `HttpTransportError::ServerStatus { code, body }`. `TransportMessage { sender, recipient, bytes }` — `bytes` is exactly the already-signed, already-canonical-enough wire bytes (Phase 9's outbound loop produces these via `SignedEnvelope::encode()` shape, see §5 below).

`crates/famp-transport-http/src/middleware.rs` — `FampSigVerifyLayer` — read to understand what NOT to duplicate: it does peek-sender → keyring lookup → canonical pre-check → `AnySignedEnvelope::decode`. This is almost exactly what D-04's gateway-side ingress needs, EXCEPT it uses the *transport's* keyring, not the gateway's peers keyring — hence the need for a parallel (not reused) implementation per D-04.

### 4. `verify_inbound`'s real signature — a gap the planner must close

`crates/famp-gateway/src/verify.rs`:
```rust
pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason>
```
This is generic over **exactly one** body class `B`. Phase 8's unit tests only exercise `AckBody`. Phase 9's E2E cycle crosses the wire with **request → commit → deliver → ack**, i.e. 4 distinct classes (`RequestBody`, `CommitBody`, `DeliverBody`, `AckBody` — see `famp_envelope::body::{RequestBody, CommitBody, DeliverBody, AckBody, ControlBody, AuditLogBody}`, mirrored by `AnySignedEnvelope`'s 6-arm dispatch in `crates/famp-envelope/src/dispatch.rs:18-26`). **The ingress handler cannot know `B` in advance** — it must dispatch by the wire `class` field first.

**Recommended fix:** add a companion function in `famp-gateway/src/verify.rs`, e.g.
```rust
pub fn verify_inbound_any(bytes: &[u8], keyring: &Keyring) -> Result<AnySignedEnvelope, RejectReason>
```
mirroring `verify_inbound`'s existing two-pass flow (`peek_sender` → keyring lookup → decode) but calling `AnySignedEnvelope::decode(bytes, vk)` (`famp_envelope::dispatch::AnySignedEnvelope::decode`, which internally reads the `class` field and dispatches to the right typed `SignedEnvelope::<B>::decode_value`) instead of the single-class `SignedEnvelope::<B>::decode`. This is a **small, mechanical addition** — `AnySignedEnvelope::decode` already exists and does exactly the class-dispatch needed; `verify_inbound_any` just needs to route the same two D-07/D-08 gates (unpinned-key hard-reject before decode; `InvalidSignature` on decode failure) through it instead of through the single-class path. Keep `verify_inbound<B>` too — it stays useful for the existing unit tests and any future single-class caller.

### 5. Cross-host envelope field population + sign site (D-03 egress)

The federation fields (`from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry`) are `Option` fields on `WireEnvelope<B>` (`crates/famp-envelope/src/wire.rs:60-73`), all `skip_serializing_if = Option::is_none`. **Local-bus mailbox entries never carry a signature and never carry these fields** (`famp-envelope/src/bus.rs` docstring, BUS-11: "carries NO signature, ever"). The drained `serde_json::Value` the gateway reads back from `Await`/`Inbox` is therefore a **plain, unsigned, non-federation JSON object** — `{"famp":"0.5.2","id":...,"from":"agent:hostA/alice","to":"agent:hostA/bob","scope":...,"class":"request","authority":...,"ts":...,"body":{...}}`.

**Recommended sign site — mutate the raw `Value`, do not reconstruct a typed `UnsignedEnvelope<B>`:**
1. Drain returns `serde_json::Value` items (no typed decode needed — this is the byte-exact "content-transparent" path D-03 requires: `task_id`/`class`/`body` are never touched).
2. `value.as_object_mut()` — insert (only if not already federation-signed, per D-03) `"from_domain"`, `"to_domain"` (both derived via `famp::Principal::authority()` — `crates/famp-core/src/identity.rs:24-27`, e.g. `from.authority().to_string()`), `"sender_key_id"` (the gateway's own `key_id(&gateway_vk)` — `famp_crypto::key_id`, 16-char b64url truncation, already used identically in `famp peer export`), `"nonce"` (random — reuse whatever RNG source `FampSigningKey::generate()` uses, e.g. `uuid::Uuid::new_v4().to_string()` or raw random bytes b64url-encoded; no existing nonce-generation helper exists in-repo yet, this is new code), `"expiry"` (an RFC 3339 string strictly after `ts`, format-checked only per Phase 8 D-04 — e.g. `ts + 5min`).
3. Sign the whole mutated `Value` with the gateway's persisted signing key: `let signature = famp::sign_value(&gateway_sk, &value)?;` (`famp::sign_value` is the `famp` crate's re-export of `famp_crypto::sign_value`, already confirmed at `crates/famp/src/lib.rs:71-74`) — this canonicalizes internally (RFC 8785 JCS via `famp_canonical::canonicalize`), so the gateway need not pre-canonicalize.
4. `value["signature"] = Value::String(signature.to_b64url())`, `serde_json::to_vec(&value)` → wire bytes for `HttpTransport::send`.

This is deliberately the **same shape `SignedEnvelope::encode()` produces** (`crates/famp-envelope/src/envelope.rs:434-467` — build the view struct, `serde_json::to_value`, insert `signature`, `serde_json::to_vec` — NOT canonical output, just plain JSON) but skips the typed `WireEnvelopeRef` reconstruction entirely, which is not otherwise reachable from a raw drained `Value` without adding new public accessors to `BusEnvelope<B>` (currently only exposes `.body()` and `.class()`, per `crates/famp-envelope/src/bus.rs:36-46` — everything else is private on `WireEnvelope<B>`). **This Value-mutation approach needs zero new `famp-envelope` public API.**

**Signing key:** `~/.famp/gateway/identity.ed25519`, loaded via `crate::cli::peer::identity::load_or_generate(&gateway_identity_path(home))` (`crates/famp/src/cli/peer/identity.rs:14-22, 39-66`) — idempotent, generates-and-persists on first use, mode 0600. `famp-gateway`'s bin does not currently import `crate::cli::peer::identity` (it's in the `famp` crate's CLI module, not `famp-gateway`) — the planner should either (a) make this function/path reachable from `famp-gateway` by adding a `pub` re-export path off `famp::cli::peer::identity`, or (b) duplicate the tiny load-or-generate helper directly into `famp-gateway` (small, ~30 lines, no external deps needed beyond `famp_crypto::FampSigningKey` + `famp::cli::perms::write_secret`-equivalent). Confirm module visibility (`crates/famp/src/cli/mod.rs` — is `peer` a `pub mod`?) before committing to option (a).

**Keyring the receiving gateway reads:** `~/.famp/gateway/peers.keyring` (`gateway_peers_keyring_path`, same file), sole writer is `famp peer import` (`crates/famp/src/cli/peer/import.rs`), loaded via `Keyring::load_from_file(&path)` — the gateway's ingress handler needs to load this once at startup (or per-request; `Keyring::load_from_file` is cheap and there's no existing watch/reload mechanism, so **load once at gateway startup** is the simplest correct choice unless the planner wants hot-reload, which is out of scope here since peers are hand-imported before the gateway starts in the own-machines-first model).

### 6. task_id / FSM "advancement" — no persisted FSM, just mailbox content

**Correction to CONTEXT.md's "taskdir" framing:** there is no on-disk taskdir. `famp inspect tasks --id <task_id> [--json]` is a **read-only, on-the-fly aggregation** (`crates/famp-inspect-server/src/tasks.rs::inspect_tasks` / `inspect_tasks_by_id`) over `ctx.message_data.by_recipient` — i.e., the same mailbox JSONL files `Await`/`Inbox` already read. `task_id` is derived per-envelope by `famp_envelope::EnvelopeView::task_id()` (`crates/famp-envelope/src/view.rs:114-155`): resolution order is `causality.ref` → `body.details.task` → (`id` iff `body.event == "famp.send.new_task"`) → `None`. Per-envelope FSM state comes from `crates/famp-inspect-server/src/parse.rs::derive_fsm_state`, a pure match on `(class, body.details.mode, body.details.terminal, body.details.action)`:
```
("request", _, _, _)              -> "REQUESTED"
("commit", _, _, _)               -> "COMMITTED"
("deliver", "completed", true, _) -> "COMPLETED"
("deliver", "failed", true, _)    -> "FAILED"
("deliver", "cancelled", true, _) -> "CANCELLED"
```
**Implication for Phase 9's E2E test:** GW-03 passes the instant a `request` envelope with the right `causality`/`body.details` shape lands in B's `bob` mailbox, B's real local reply cycle (`commit`/`deliver`) lands in `bob`'s own outbound path and gets relayed back through the gateways into A's `alice` mailbox, and `famp inspect tasks --id <task_id> --json` on **both** sides shows the matching envelopes. Since D-03 is content-transparent (task_id/class/body untouched), this "just works" once the drain→sign→POST→verify→deliver pipeline round-trips any envelope class correctly — there is no separate FSM-wiring task needed beyond making the relay byte-preserving.

**What the executor needs to seed:** a normal local `famp send`/`famp reply` conversation on each side (the existing `request → commit → deliver → ack` local cycle already used elsewhere in the test suite, e.g. `crates/famp/tests/common/cycle_driver.rs` — reuse `drive_alice`/`drive_bob` shape if useful) addressed to the gateway-backed remote principal name.

### 7. Two-process loopback E2E harness (D-07) — concrete patterns to reuse

Two existing patterns to compose, neither of which is a full match alone:

**Pattern A — real subprocess brokers+gateways** (`crates/famp-gateway/tests/liveness.rs`, Phase 7): `Command::cargo_bin("famp")` / `Command::cargo_bin("famp-gateway")` spawned via `assert_cmd::cargo::CommandCargoExt`, wrapped in `ChildGuard` (`crates/famp-gateway/tests/common/child_guard.rs` and `crates/famp/tests/common/child_guard.rs` — **two separate copies today**, RAII kill+wait on drop), polled with a bounded deadline loop (`wait_for_broker_socket`) rather than a fixed `sleep()`. **Cross-package binary gotcha already documented**: `crates/famp-gateway` tests must `cargo build --quiet -p famp --bin famp` explicitly first (`ensure_famp_bin_built()`) because `CARGO_BIN_EXE_famp` is not propagated across package boundaries by Cargo — this bites any Phase 9 test spawning `famp` from a `famp-gateway`-crate test binary, or vice versa.

**Pattern B — in-process HTTPS transport with fixture certs** (`crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred`, pre-v0.9): loads TLS fixture certs from `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` (still present on disk — confirmed), builds two `axum`+`rustls` listeners via `tls_server::serve_std_listener(listener, router, Arc<ServerConfig>)`, drives the cycle via a shared `cycle_driver` helper. This test predates the gateway entirely (it calls `HttpTransport`/`build_router` directly, no UDS bus, no `famp-gateway` process) — **useful only for the TLS/cert-provisioning half of D-08**, not for the two-broker/two-gateway process topology D-07 actually wants.

**Recommended composition for Phase 9's gate:** two real broker subprocesses (Pattern A shape, distinct `--socket <path>` per broker — `famp broker --socket <sock>`), two real `famp-gateway` subprocesses (`famp-gateway --socket <sock> <principal-name>`) pointed at each broker, HTTPS loopback between the two gateways reusing the Pattern-B fixture certs, and two real local agents (any process capable of `famp send`/`famp inbox`/`famp reply` against each broker's socket) driving the actual request→commit→deliver→ack cycle. **Distinct isolation axes needed, both must vary per side**: `--socket <path>` (or `FAMP_BUS_SOCKET`) for broker/mailbox isolation (`bus_client::bus_dir()` anchors mailboxes/cursors off the socket's parent dir, NOT `FAMP_HOME`), AND `FAMP_HOME` for gateway identity/peer-keyring isolation (`~/.famp/gateway/identity.ed25519`, `~/.famp/gateway/peers.keyring` are keyed off `FAMP_HOME`, resolved via `crate::cli::home::resolve_famp_home()` — `$FAMP_HOME` verbatim else `$HOME/.famp`). A single `tempfile::tempdir()` per side used for both purposes (e.g. `tmpA/bus.sock` + `FAMP_HOME=tmpA`) is the simplest correct setup.

**`cargo nextest -p famp` hang gotcha** (memory: `project_nextest_list_hang.md`): nextest stalls in the test-binary `--list` phase for this crate. **Use plain `cargo test -p famp --test <name>` / `cargo test -p famp-gateway --test <name>`, not `cargo nextest`, for any new Phase 9 integration test binary.**

### 8. Pitfalls / landmines specific to this phase

- **No `serde(flatten)`, no `serde(tag)` on the envelope** — `crates/famp-envelope/src/wire.rs:7-8` warning; any new field added to support Phase 9 (there shouldn't be any — the federation fields already exist from Phase 8) must follow the plain-`Option`-with-`skip_serializing_if` pattern, never flatten/tag.
- **`just lint` (not plain `cargo clippy`) is the required gate for Rust changes** — per project memory (`feedback_executor_briefs_need_just_lint_for_rust.md`), `just lint` promotes nursery lints beyond plain clippy; a pedantic `expect_used`/`unwrap_used` in new gateway code will pass `cargo test` but block `git push` at the pre-push hook.
- **`.planning/` is gitignored** — per project memory, GSD executors for this phase should run non-isolated on `main` (not a worktree), consistent with `use_worktrees: false` already set in `.planning/config.json`.
- **`clippy::future_not_send`** — an existing, already-`#[allow]`'d pattern in this codebase (`crates/famp/src/cli/hook/{emit,codex_stop}.rs`) for futures holding `&mut dyn Write` across an `.await`. Phase 9's outbound drain loop or ingress handler is unlikely to hold a `Write` reference across await, but if any diagnostic/logging helper does, use the same `#[allow(clippy::future_not_send)]` + doc-comment convention rather than restructuring around it.
- **`just install` after any MCP tool-surface change** — not expected to be touched this phase (Phase 9 is gateway-process wiring, not MCP surface), but if any `famp_peers`/gateway-status MCP tool is added, `just install` is required before closing the PR (project CLAUDE.md convention).
- **Phase 7/8 VERIFICATION notes on what's NOT yet wired**: Phase 8's own canonical-refs section states outright: "the gateway has no transport/HTTP ingress yet — that's Phase 9" (`08-VERIFICATION.md`, referenced from `08-CONTEXT.md` line 177). Confirmed still true by direct source read: `famp-gateway`'s `Cargo.toml` has no `famp-transport-http` dependency, and `main.rs` only backs principals and parks on `ctrl_c()` — zero HTTP code exists in `famp-gateway` today. Phase 9 is starting this from a clean slate, not patching partial wiring.
- **One-signing-key-per-remote-principal-name limitation** (§ Summary above) — do not design the E2E test around multiple simultaneous remote principals per host; that's explicitly deferred to v1.1 per `crates/famp/src/cli/peer/mod.rs` docstring.
- **`GatewayRegistry::get` is immutable-only today** — any drain/send code needs a new `get_mut`; don't assume it already exists.
- **`resolve_op_identity` gates on connection registration, not envelope `from`** — this is a *feature* (it's what makes D-05 work without new bus code) but also means the gateway's `verify_inbound_any` call is the ENTIRE authorization boundary for "may this bytes-blob claim to be `alice`" — there is no secondary broker-side check. Any bug in the ingress handler that calls `Send` before (or without) `verify_inbound_any` succeeding is a silent trust bypass, not a loud failure.

## Common Pitfalls

### Pitfall 1: Calling `famp_transport_http::build_router()` unmodified for ingress
**What goes wrong:** The transport's own `FampSigVerifyLayer` runs first, verifying against the *transport's* keyring parameter (whatever `Arc<Keyring>` is passed to `build_router`), never invoking `famp_gateway::verify_inbound`/`verify_inbound_any` at all.
**Why it happens:** `build_router` looks like the obvious "just wrap it" entry point and its signature (`keyring: Arc<Keyring>, inboxes: Arc<InboxRegistry>`) makes it look pluggable.
**How to avoid:** Build a parallel, gateway-owned router (Option A in §3) or add an explicit unverified variant to `famp-transport-http` (Option B). Either way, `verify_inbound`/`verify_inbound_any` must be the ONLY signature check on the ingress path (D-04, D-08).
**Warning signs:** A gateway rejecting valid peer-signed envelopes because they're not ALSO pinned in whatever keyring got passed to `build_router`, or (worse) silently accepting envelopes verified against the wrong keyring.

### Pitfall 2: Treating `verify_inbound<B>` as ready for multi-class ingress
**What goes wrong:** Wiring the ingress handler directly to `verify_inbound::<RequestBody>` (or any single class) means every other class (commit/deliver/ack) fails to decode and the E2E cycle silently drops 3 of 4 message classes.
**Why it happens:** `verify_inbound` is the only function `famp-gateway::verify` exports today; its generic-over-`B` signature is easy to miss until you try to call it without a compile-time-known class.
**How to avoid:** Add `verify_inbound_any` (§4) before wiring the ingress handler; test all 4 real classes cross the wire in the E2E, not just one.
**Warning signs:** E2E test only exercises one message class end-to-end and passes — the other 3 classes are untested and may be silently broken.

### Pitfall 3: Re-signing via a reconstructed typed `UnsignedEnvelope<B>` instead of Value-mutation
**What goes wrong:** Attempting to decode the drained mailbox `Value` into `AnyBusEnvelope`, then extract fields to rebuild a typed `UnsignedEnvelope<B>` via its builder methods, discovers `BusEnvelope<B>` exposes only `.body()` and `.class()` — every other field (`id`, `from`, `to`, `ts`, `causality`, `authority`, `terminal_status`, `idempotency_key`, `extensions`) is private on the inner `WireEnvelope<B>`. This forces either a large new public-accessor surface on `famp-envelope`, or reaching for private-field workarounds.
**Why it happens:** The typed type-state API (`UnsignedEnvelope`/`SignedEnvelope`) looks like the "correct" high-level path, but it was designed for callers who are *constructing* an envelope from scratch, not *relaying* one whose fields are already fully populated as JSON.
**How to avoid:** Mutate the raw `serde_json::Value` in place (§5) — sign it with `famp::sign_value`, which accepts any `T: Serialize` including `&Value`. Zero new `famp-envelope` public API needed.
**Warning signs:** A plan task that proposes adding many new public getters to `BusEnvelope<B>`/`UnsignedEnvelope<B>` just to support the relay — that's a sign the Value-mutation path was missed.

### Pitfall 4: Assuming `famp inspect tasks` needs new gateway-side FSM code
**What goes wrong:** Building gateway-side task tracking/FSM state to "make GW-03 observable," duplicating logic that already exists purely as a read-side aggregation over mailbox content.
**Why it happens:** CONTEXT.md's own language ("taskdir," "advances its own task record") reads as if there's a stateful record to update.
**How to avoid:** Confirm via `crates/famp-inspect-server/src/tasks.rs` and `parse.rs` (§6) that `famp inspect tasks` is 100% derived from mailbox JSONL at query time. D-06 ("gateway is FSM-transparent") is correct and needs zero gateway code beyond byte-exact relay — do not add anything.
**Warning signs:** A plan task titled anything like "implement gateway task-state tracking" or "add FSM persistence to famp-gateway."

## Runtime State Inventory

Not applicable — Phase 9 is not a rename/refactor/migration phase. Skipped per the trigger condition in the agent instructions.

## Code Examples

### Existing axum 0.8 raw-body handler shape to mirror (D-04)
```rust
// Source: crates/famp-transport-http/src/server.rs:69-113 (existing inbox_handler)
async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(state): State<ServerState>,
    Extension(envelope): Extension<Arc<AnySignedEnvelope>>, // Phase 9's handler drops this —
                                                              // no upstream middleware stashes
                                                              // a pre-verified envelope
    body: Bytes,
) -> Result<StatusCode, MiddlewareError> { /* ... */ }
```

### Existing `ProxiedPrincipal::register` — the pattern any new send/drain method must match
```rust
// Source: crates/famp-gateway/src/principal.rs:24-57
pub async fn register(sock: &Path, name: String) -> Result<Self, GatewayError> {
    let mut client = BusClient::connect_no_spawn(sock, None).await.map_err(map_bus_client_err)?;
    let register = BusMessage::Register { name: name.clone(), pid: std::process::id(), cwd: None, listen: false };
    match client.send_recv(register).await.map_err(map_bus_client_err)? {
        BusReply::RegisterOk { .. } => Ok(Self { _client: client, name }),
        BusReply::Err { kind, message } => Err(GatewayError::RegisterFailed { kind, message }),
        other => Err(GatewayError::UnexpectedReply(format!("{other:?}"))),
    }
}
```
`map_bus_client_err` (private, same file) is the exact error-mapping function to reuse for any new `send_recv`-wrapping method on `ProxiedPrincipal`.

### Existing signing call shape to reuse for federation re-sign (D-03)
```rust
// Source: crates/famp-crypto/src/sign.rs:25-29, re-exported via famp::sign_value
pub fn sign_value<T: serde::Serialize + ?Sized>(
    signing_key: &FampSigningKey,
    value: &T,
) -> Result<FampSignature, CryptoError>
// Canonicalizes internally — pass &serde_json::Value directly, no pre-canonicalization needed.
```

## State of the Art

| Old Approach (pre-Phase-9) | Current Approach (Phase 9 target) | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `famp-gateway` backs principals and only parks on `ctrl_c()` | Gateway runs outbound drain loop(s) + inbound HTTP listener concurrently | This phase | First phase where `famp-gateway` does real transport work, not just liveness-proxying |
| `verify_inbound<B>` unit-tested in-process only (Phase 8) | `verify_inbound`/`verify_inbound_any` called from a live HTTP handler | This phase | First live exercise of the WIRE-01/TRUST-02 boundary against real network bytes |
| `famp-transport-http` preserved but unused by any live crate except deferred tests | `famp-gateway` becomes its first live consumer | This phase | Crate `Cargo.toml` dependency must be added; the crate exits "preserved, dormant" status |

**Deprecated/outdated:** none — this phase only activates dormant, already-correct primitives.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `verify_inbound_any` (new function name/shape) is the right fix for the multi-class ingress gap — not, e.g., changing `verify_inbound`'s signature | §4, Pitfall 2 | Low — this is an additive suggestion; planner/executor can choose a different function name or fold the logic into the axum handler directly without changing the underlying finding that a class-dispatch step is needed |
| A2 | Nonce generation has no existing in-repo helper and needs new code (`uuid::Uuid::new_v4()` or raw random bytes) | §5 | Low — if an existing nonce helper is found during planning that this research missed, use it instead; the format contract (`federation_format_ok()`, non-empty) is unaffected either way |
| A3 | Loading the peers keyring once at gateway startup (no hot-reload) is sufficient for Phase 9's own-machines-first scope | §5 | Low — if the E2E test needs to `peer import` AFTER gateway start, a restart-the-gateway step suffices for this phase; hot-reload would only matter for a long-running production gateway, out of scope here |
| A4 | `famp-gateway` can reach `crate::cli::peer::identity::load_or_generate` via a `pub` path from the `famp` crate, OR the planner duplicates the ~30-line helper | §5 | Low — either resolution works; the risk is only which one the planner picks, not whether the underlying key-persistence mechanism exists (it does, confirmed) |

## Open Questions

1. **Exact D-04 router construction (Option A vs Option B in §3)**
   - What we know: `build_router()` cannot be reused unmodified; both options are technically viable; CONTEXT.md explicitly defers this to the planner.
   - What's unclear: whether touching `famp-transport-http` (Option B, adding `build_router_unverified`) is preferred over a fully separate gateway-owned router (Option A) for maintainability.
   - Recommendation: Option A (fully separate, ~40-line router in `famp-gateway`) — it touches zero preserved-transport code, keeping the "wrap, do not rebuild" boundary crisp, at the cost of duplicating the `RequestBodyLimitLayer` + route-string wiring (small).

2. **Where the gateway signing-key load-or-generate call lives (A4 above)**
   - What we know: the function exists today at `crates/famp/src/cli/peer/identity.rs`, in the `famp` crate's `cli` module.
   - What's unclear: whether `pub mod peer` is visible outside `crate::cli` (would need checking `crates/famp/src/cli/mod.rs`'s visibility modifiers) for `famp-gateway` to import it directly, versus needing a small duplicate.
   - Recommendation: planner/executor greps `crates/famp/src/cli/mod.rs` for `pub mod peer` at implementation time (one `grep` call) and picks the reuse path if visible, else duplicates the ~30-line helper into `famp-gateway/src/identity.rs`.

## Environment Availability

Not applicable in the "external tool/service" sense — Phase 9 adds no new runtime dependency beyond the Rust toolchain already required for this entire project (Phase 0 prerequisite). All crates used (`axum` 0.8.8, `reqwest` 0.13.2, `rustls` 0.23.38) are already pinned in the workspace `Cargo.toml` and already compiled successfully as part of `famp-transport-http` since its preservation. No `Environment Availability` table is warranted.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (plain — NOT `cargo nextest -p famp`, see Pitfall/gotcha above) + `assert_cmd` for subprocess-driven integration tests |
| Config file | none — workspace `Cargo.toml` + per-crate `Cargo.toml` `[dev-dependencies]` |
| Quick run command | `cargo test -p famp-gateway --lib` (unit tests, e.g. new `verify_inbound_any` cases) |
| Full suite command | `cargo test -p famp-gateway --test <e2e_test_name>` (the D-07 two-process loopback gate) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GW-01 | A→B named-principal message reaches B's local mailbox | integration (2-process) | `cargo test -p famp-gateway --test e2e_cross_host_delivery` (new) | ❌ Wave 0 |
| GW-02 | B's reply within the same task reaches A's local mailbox | integration (2-process, same test) | same command | ❌ Wave 0 |
| GW-03 | Full request→commit→deliver→ack completes; `famp inspect tasks --id <id>` shows terminal state on both sides | integration (2-process, same test, asserts twice) | same command, plus `famp inspect tasks --id <task_id> --json` subprocess assertions inside the test | ❌ Wave 0 |
| — | `verify_inbound_any` rejects unsigned / unpinned-key bytes for all 4 classes (unit) | unit | `cargo test -p famp-gateway --lib verify` | ❌ Wave 0 (extends existing `verify.rs` tests) |
| — | `ProxiedPrincipal` new send/drain methods round-trip a `Send`/`Await` against a real broker | integration | `cargo test -p famp-gateway --test principal_send_drain` (new, or fold into existing `no_cross_talk.rs`/`liveness.rs`) | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p famp-gateway --lib` (fast unit-level checks on `verify_inbound_any`, federation-field-mutation helper, `ProxiedPrincipal` new methods)
- **Per wave merge:** `cargo test -p famp-gateway` (full crate, including the new subprocess-spawning E2E test — expect this to be slow, tens of seconds, per the Phase 7 `liveness.rs` precedent of spawning real broker+gateway processes)
- **Phase gate:** the new two-process loopback E2E green, plus `just lint` clean, before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (new) — covers GW-01/GW-02/GW-03, composing Pattern A (subprocess brokers+gateways, `crates/famp-gateway/tests/liveness.rs` shape) with the fixture certs from `crates/famp/tests/fixtures/cross_machine/`
- [ ] Extend `crates/famp-gateway/src/verify.rs` unit tests to cover `verify_inbound_any` across all 4 real message classes (request/commit/deliver/ack), not just `AckBody`
- [ ] New unit tests for the `ProxiedPrincipal` drain/send methods (mock or real broker) — likely lands in `crates/famp-gateway/src/principal.rs`'s existing `#[cfg(test)]` module or a new integration test
- [ ] Consider consolidating the two duplicate `ChildGuard` copies (`crates/famp-gateway/tests/common/child_guard.rs` and `crates/famp/tests/common/child_guard.rs`) if Phase 9's test needs to spawn from both crates in one binary — not required, but flagged since Phase 9 is the first phase to potentially need both `famp` and `famp-gateway` binaries spawned from the same test

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Ed25519 signature verification (`verify_inbound`/`verify_inbound_any`) is the sole cross-host authentication mechanism — no username/password, no session tokens on this path |
| V3 Session Management | no | The gateway's UDS connections to the local broker are the only "sessions," already covered by Phase 7's liveness design; nothing new this phase |
| V4 Access Control | yes | TRUST-02 (unpinned-key hard reject, no auto-pin) is the access-control gate; Phase 9 must not weaken it — `verify_inbound_any` must preserve the exact two-reason reject contract (`InvalidSignature` vs `UnpinnedKey`) from Phase 8 |
| V5 Input Validation | yes | `famp_canonical::from_slice_strict` (duplicate-key rejection) + `WireEnvelope`'s `deny_unknown_fields` already provide this; Phase 9 must not introduce a parse path that bypasses strict-parse (e.g. never call plain `serde_json::from_slice` on inbound HTTP bytes — always route through `peek_sender`/`AnySignedEnvelope::decode`, which use `from_slice_strict` internally) |
| V6 Cryptography | yes | Ed25519 via `famp_crypto` (`sign_value`/`verify_value`, `verify_strict` under the hood) — never hand-roll; Phase 9 reuses existing signing helpers exclusively (§5) |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Ingress bypasses `verify_inbound_any` and calls `Send` directly on attacker-controlled bytes | Spoofing / Tampering | Structural: the only path from HTTP body → local bus `Send` must go through `verify_inbound_any` first — no other code path should ever construct a `BusMessage::Send` from raw inbound bytes. Recommend a code-review checklist item / integration test that asserts an unsigned or wrong-key POST produces ZERO bus mailbox writes (mirrors Phase 8 D-08's existing unit-test pattern, extended to the live HTTP path) |
| Gateway re-signs with the wrong key (e.g. accidentally the receiving side's key) producing an envelope that verifies against the wrong peer entry | Spoofing | Keep the signing key strictly scoped to `~/.famp/gateway/identity.ed25519` read via `load_or_generate` at the OUTBOUND gateway only; never let the inbound handler sign anything |
| Replay of a captured cross-host envelope (nonce/expiry carried but NOT enforced this phase, per Phase 8 D-04) | Repudiation / Tampering (partial) | **Explicitly out of scope for Phase 9** (INGRESS-01 is v1.1) — do not add active replay-cache/expiry-rejection logic; document this limitation is inherited unchanged from Phase 8, not newly introduced |
| Body-size DoS on the new ingress route | Denial of Service | Reuse `RequestBodyLimitLayer::new(1_048_576)` (1 MiB, TRANS-07 §18) on whatever router construction Phase 9 builds (Option A/B in §3) — do not drop this layer when bypassing `FampSigVerifyLayer` |

## Sources

### Primary (HIGH confidence — direct source reads, this session)
- `crates/famp-gateway/src/{lib,main,registry,principal,verify,error}.rs` — full read, confirmed current gateway surface and the send/drain gap
- `crates/famp-transport-http/src/{server,transport,middleware}.rs` — full read, confirmed `build_router`/`INBOX_ROUTE`/`FampSigVerifyLayer`/`HttpTransport` shapes
- `crates/famp/src/bus_client/mod.rs` — full read, confirmed `BusClient::send_recv`/`connect_no_spawn` shapes
- `crates/famp-bus/src/proto.rs`, `crates/famp-bus/src/broker/handle.rs` (relevant sections) — confirmed `BusMessage`/`BusReply` variants and the `send()` handler's identity-resolution behavior
- `crates/famp-envelope/src/{envelope,bus,dispatch,peek,view,wire,lib}.rs` — full/partial reads, confirmed `SignedEnvelope`/`UnsignedEnvelope`/`AnySignedEnvelope`/`BusEnvelope`/`EnvelopeView` public surfaces
- `crates/famp-crypto/src/{sign,verify,keys}.rs` (grep + targeted reads) — confirmed `sign_value`/`verify_value`/`key_id` signatures
- `crates/famp/src/cli/peer/{mod,export,import,identity}.rs` — full reads, confirmed gateway keypair persistence path and peer export/import CLI shape
- `crates/famp/src/cli/home.rs` — confirmed `FAMP_HOME` resolution
- `crates/famp-inspect-server/src/{tasks,parse}.rs` — confirmed `famp inspect tasks` is a read-time mailbox aggregation, not a persisted FSM
- `crates/famp-gateway/tests/liveness.rs`, `crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred`, `crates/famp/tests/e2e_two_daemons_adversarial.rs`, `crates/famp/tests/common/child_guard.rs` — confirmed the two candidate E2E harness patterns
- `Cargo.toml` (workspace), `crates/famp-gateway/Cargo.toml` — confirmed pinned versions and the missing `famp-transport-http` dependency
- `.planning/phases/09-.../09-CONTEXT.md`, `.planning/phases/08-.../08-CONTEXT.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md` — locked decisions and requirement text

### Secondary (MEDIUM confidence)
- None — no external documentation lookups were needed; this phase is 100% in-repo composition of already-built, already-documented primitives.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies, every fact cited from files read this session
- Architecture: HIGH — every claim (gateway surface, bus protocol, envelope shapes, transport router) is a direct source citation with file path
- Pitfalls: HIGH — each pitfall traces to a specific, cited code gap (missing `get_mut`, single-class `verify_inbound`, private `BusEnvelope` fields, `build_router`'s unconditional middleware)

**Research date:** 2026-07-27
**Valid until:** 30 days (stable — this research describes existing, merged code; the only invalidation risk is if Phase 9 planning itself changes `famp-gateway`'s surface before this doc is consumed, which is expected and fine since the planner reads this immediately)
