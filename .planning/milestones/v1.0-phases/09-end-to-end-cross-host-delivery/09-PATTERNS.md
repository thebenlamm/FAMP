# Phase 9: End-to-End Cross-Host Delivery - Pattern Map

**Mapped:** 2026-07-27
**Files analyzed:** 8 (new/modified)
**Analogs found:** 8 / 8

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/famp-gateway/src/principal.rs` (ADD send/drain) | service (UDS client wrapper) | request-response | same file's existing `register()` | exact (same file, same idiom) |
| `crates/famp-gateway/src/registry.rs` (ADD `get_mut`) | service (demux table) | CRUD | same file's existing `get()` | exact |
| `crates/famp-gateway/src/verify.rs` (ADD `verify_inbound_any`) | utility (pure verify fn) | transform | same file's existing `verify_inbound<B>` + `famp-envelope` `AnySignedEnvelope::decode` dispatch | exact (mechanical multi-class wrapper) |
| new `crates/famp-gateway/src/ingress.rs` (or `http.rs`) — inbound axum router | route/middleware (HTTP ingress) | request-response | `crates/famp-transport-http/src/server.rs` (`build_router`, `inbox_handler`) | role-match (must NOT call `build_router` directly — hand-assemble, see D-04) |
| new `crates/famp-gateway/src/egress.rs` (or drain loop in `main.rs`) — outbound drain+sign+POST | service (event-driven relay) | event-driven | `crates/famp-transport-http/src/transport.rs` (`HttpTransport::send`/`new_client_only`) + `crates/famp/src/cli/peer/identity.rs` (`load_or_generate`) | role-match |
| `crates/famp-gateway/src/main.rs` (replace park-only loop) | config/bootstrap (bin entrypoint) | event-driven | same file's existing `main()` | exact (extend, don't replace shape) |
| `crates/famp-gateway/Cargo.toml` (ADD `famp-transport-http` dep) | config | — | `crates/famp-gateway/Cargo.toml` existing deps (`famp-envelope`, `famp-keyring` added in 08-03) | exact |
| `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (new) | test (2-process integration) | event-driven | `crates/famp-gateway/tests/liveness.rs` + `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` + `crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred` | role-match (compose two patterns) |

## Pattern Assignments

### `crates/famp-gateway/src/principal.rs` (service, request-response) — ADD send/drain

**Analog:** same file, `ProxiedPrincipal::register` (lines 34-57) + `map_bus_client_err` (lines 73-91)

**Struct field to un-privatize** (lines 15-22):
```rust
pub struct ProxiedPrincipal {
    _client: BusClient,   // rename to `client` (drop leading underscore) to allow &mut methods
    name: String,
}
```

**Pattern to copy for the new send/drain method** (mirrors `register`'s `send_recv` + error-map shape, lines 34-57):
```rust
pub async fn register(sock: &Path, name: String) -> Result<Self, GatewayError> {
    let mut client = BusClient::connect_no_spawn(sock, None)
        .await
        .map_err(map_bus_client_err)?;
    let register = BusMessage::Register { name: name.clone(), pid: std::process::id(), cwd: None, listen: false };
    match client.send_recv(register).await.map_err(map_bus_client_err)? {
        BusReply::RegisterOk { .. } => Ok(Self { _client: client, name }),
        BusReply::Err { kind, message } => Err(GatewayError::RegisterFailed { kind, message }),
        other => Err(GatewayError::UnexpectedReply(format!("{other:?}"))),
    }
}
```
New method should follow exactly this shape:
```rust
pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, GatewayError> {
    self.client.send_recv(msg).await.map_err(map_bus_client_err)
}
```
Reuse `map_bus_client_err` (lines 73-91) unchanged — it already covers every `BusClientError` variant.

**Bus wire ops to send/receive via this method** (from `famp-bus/src/proto.rs:118-203, 205-273`, cited in RESEARCH §2):
```rust
BusMessage::Send { to: Target, envelope: serde_json::Value }
BusMessage::Await { timeout_ms: u64, task: Option<uuid::Uuid> }
BusReply::SendOk { task_id, delivered }
BusReply::AwaitOk { envelopes, mailbox, next_offset }
BusReply::AwaitTimeout {}
```

---

### `crates/famp-gateway/src/registry.rs` (CRUD) — ADD `get_mut`

**Analog:** same file, `get()` (lines 38-41)

```rust
#[must_use]
pub fn get(&self, name: &str) -> Option<&ProxiedPrincipal> {
    self.principals.get(name)
}
```
Mirror directly:
```rust
pub fn get_mut(&mut self, name: &str) -> Option<&mut ProxiedPrincipal> {
    self.principals.get_mut(name)
}
```

---

### `crates/famp-gateway/src/verify.rs` (transform) — ADD `verify_inbound_any`

**Analog:** same file, `verify_inbound<B>` (lines 36-45)

```rust
pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&from) else {
        return Err(RejectReason::UnpinnedKey { principal: from });
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}
```
New function keeps the identical two-gate shape (unpinned-key hard reject BEFORE decode; `InvalidSignature` on decode failure) but swaps the decode call for `famp_envelope::dispatch::AnySignedEnvelope::decode`, which internally reads `class` and dispatches to the right typed body:
```rust
pub fn verify_inbound_any(bytes: &[u8], keyring: &Keyring) -> Result<AnySignedEnvelope, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&from) else {
        return Err(RejectReason::UnpinnedKey { principal: from });
    };
    AnySignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}
```
Test module (lines 47-170) is the pattern for unit-testing every class — extend `signed_bytes` to build `RequestBody`/`CommitBody`/`DeliverBody` variants alongside the existing `AckBody`, reusing `strip_signature` (lines 74-78) for the invalid-signature case and an empty `Keyring::new()` (line 156) for the unpinned case.

---

### New ingress router — `crates/famp-gateway/src/ingress.rs` (route/middleware, request-response)

**Analog:** `crates/famp-transport-http/src/server.rs` — read fully, lines 1-113. **Do NOT call `build_router`** (it unconditionally mounts `FampSigVerifyLayer` against the transport's own keyring — D-04 forbids a second trust source).

**Router construction pattern to copy (structure only, not the layer stack)** (lines 27-51):
```rust
const ONE_MIB: usize = 1_048_576;
pub const INBOX_ROUTE: &str = "/famp/v0.5.1/inbox/{principal}";  // import this const from famp_transport_http::server instead of redeclaring the string, if pub; else re-declare identically

pub fn build_router(keyring: Arc<Keyring>, inboxes: Arc<InboxRegistry>) -> Router {
    let state = ServerState { inboxes };
    Router::new()
        .route(INBOX_ROUTE, post(inbox_handler))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(ONE_MIB))   // KEEP — D-04/security V DoS mitigation
                .map_request(|req: Request<_>| req.map(Body::new))
                .layer(FampSigVerifyLayer::new(keyring)),      // DROP — replaced by gateway's own verify_inbound_any inside the handler body
        )
}
```

**Handler signature to mirror (drop the `Extension` param — no upstream middleware pre-verifies)** (lines 69-113):
```rust
async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(state): State<ServerState>,
    Extension(envelope): Extension<Arc<AnySignedEnvelope>>,  // Phase 9: DELETE this param
    body: Bytes,
) -> Result<StatusCode, MiddlewareError> {
    let recipient = Principal::from_str(&principal_str).map_err(|_| MiddlewareError::BadPrincipal)?;
    let sender = envelope_sender(&envelope).clone();          // Phase 9: replace with verify_inbound_any(&body, &gateway_keyring) call
    // ... deliver to inbox channel ...
    Ok(StatusCode::ACCEPTED)
}
```
Phase 9's handler body: call `famp_gateway::verify::verify_inbound_any(&body, &gateway_keyring)` first; on `Err`, return an HTTP 4xx (map `RejectReason::InvalidSignature`/`UnpinnedKey` to distinct status codes per D-08's split-error contract) with **zero bus writes**; on `Ok(envelope)`, deliver via `GatewayRegistry::get_mut(sender_name)` → `ProxiedPrincipal::send_recv(BusMessage::Send{ to: Target::Agent{name: recipient}, envelope: <raw Value> })` (D-05 — deliver via the backed *sender* stand-in).

---

### New egress/drain loop — `crates/famp-gateway/src/egress.rs` (service, event-driven)

**Analog A — drain primitive:** `crates/famp-gateway/src/principal.rs`'s new `send_recv` (above) issuing `BusMessage::Await{ timeout_ms: 30_000, task: None }` in a loop (RESEARCH §2/§7 — prefer `Await` over polling `Inbox`, push-latency for free via broker's `waiting_clients_for_name`).

**Analog B — HTTP client send:** `crates/famp-transport-http/src/transport.rs`, `HttpTransport::new_client_only` (lines 51-67) + `Transport::send` (lines 118+):
```rust
pub fn new_client_only(trust_cert_path: Option<&Path>) -> Result<Self, HttpTransportError> {
    let tls = build_client_config(trust_cert_path).map_err(|e| HttpTransportError::TlsConfig(format!("{e:?}")))?;
    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .timeout(Duration::from_secs(10))
        .http1_only()
        .build()
        .map_err(HttpTransportError::ReqwestFailed)?;
    // ...
}
```
`send` POSTs to `{base}/famp/v0.5.1/inbox/{recipient}` and expects HTTP 202 — reuse `HttpTransport` as-is for the outbound POST (D-03); do not reimplement the client.

**Analog C — signing key load-or-generate:** `crates/famp/src/cli/peer/identity.rs`, `load_or_generate` (lines 38-66) — reuse verbatim if `pub mod peer` is visible from `famp-gateway` (grep `crates/famp/src/cli/mod.rs` at implementation time), else duplicate the ~30-line function into `famp-gateway`:
```rust
pub fn load_or_generate(path: &Path) -> Result<FampSigningKey, CliError> {
    if path.exists() {
        let bytes = std::fs::read(path)...;
        return FampSigningKey::from_b64url(s.trim())...;
    }
    let key = FampSigningKey::generate();
    // create_dir_all(parent) then write_secret(path, key.to_b64url().as_bytes()) at mode 0600
    Ok(key)
}
```

**Sign-site pattern (Value-mutation, NOT typed reconstruction — RESEARCH §5/Pitfall 3):**
```rust
// value: serde_json::Value drained raw from BusReply::AwaitOk.envelopes
let obj = value.as_object_mut().unwrap();
obj.insert("from_domain".into(), from.authority().to_string().into());
obj.insert("to_domain".into(), to.authority().to_string().into());
obj.insert("sender_key_id".into(), famp_crypto::key_id(&gateway_vk).into());
obj.insert("nonce".into(), uuid::Uuid::new_v4().to_string().into());
obj.insert("expiry".into(), /* ts + 5min, RFC3339 */);
let signature = famp::sign_value(&gateway_sk, &value)?;   // famp_crypto::sign_value re-export, canonicalizes internally
obj.insert("signature".into(), signature.to_b64url().into());
let bytes = serde_json::to_vec(&value)?;   // -> HttpTransport::send
```

---

### `crates/famp-gateway/src/main.rs` (bootstrap, event-driven) — replace park loop

**Analog:** same file's existing `main()` (lines 67-99) — keep the arg-parse (`parse_args`) and `registry.back()` loop unchanged; replace only the final `tokio::signal::ctrl_c().await` park with `tokio::select!` over: the inbound axum server task (ingress router above), one outbound drain task per backed principal (egress loop above), and `ctrl_c()` for graceful shutdown. Same overall shape as `crates/famp-gateway/tests/liveness.rs`'s subprocess-driving pattern expects (a single long-running process that stays up until killed).

---

### `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (test, event-driven, 2-process)

**Analog A — subprocess + ChildGuard + poll-not-sleep:** `crates/famp-gateway/tests/liveness.rs`, full file pattern:
```rust
fn ensure_famp_bin_built() { /* cargo build --quiet -p famp --bin famp — REQUIRED, CARGO_BIN_EXE_famp doesn't cross package boundaries */ }

fn spawn_broker_subprocess(sock: &Path) -> ChildGuard {
    ChildGuard::new(Command::cargo_bin("famp").unwrap()
        .args(["broker", "--socket", sock.to_str().unwrap()])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().unwrap())
}

fn spawn_gateway_subprocess(sock: &Path, names: &[&str]) -> ChildGuard {
    ChildGuard::new(Command::cargo_bin("famp-gateway").unwrap()
        .arg("--socket").arg(sock).args(names)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().unwrap())
}

fn wait_for_broker_socket(sock: &Path, deadline: Duration) { /* poll UnixStream::connect, NEVER fixed sleep */ }
fn poll_until_all_live(sock: &Path, expect_live: &[&str], deadline: Duration) { /* poll famp inspect identities --json */ }
```
`ChildGuard` import: `#[path = "common/child_guard.rs"] mod child_guard;` — two copies exist (`crates/famp-gateway/tests/common/child_guard.rs`, `crates/famp/tests/common/child_guard.rs`); use the `famp-gateway` copy since the test lives in that crate.

Phase 9's test needs a `famp-gateway --socket <sock> --peer <domain>=<url> --identity-home <FAMP_HOME> <principal>` invocation per side plus TWO isolation axes per side (RESEARCH §7): `--socket <path>` for bus/mailbox isolation AND `FAMP_HOME` env var for gateway identity/peers-keyring isolation — a single `tempfile::tempdir()` per side serves both (`tmpA/bus.sock` + `FAMP_HOME=tmpA`).

**Analog B — TLS fixture certs + rustls server:** `crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred` — loads certs from `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` (confirmed present on disk), builds axum+rustls listeners via `tls_server::serve_std_listener(listener, router, Arc<ServerConfig>)`. Reuse only the cert-loading/rustls-server-config half; the process topology (real UDS brokers + real gateway subprocesses) comes from Analog A, not this deferred test (which predates the gateway).

**Assertion pattern — `famp inspect tasks --id <task_id> --json`:** same shape as `live_identity_names` in `liveness.rs` (spawn `famp inspect identities --json`, parse into a typed reply struct) — swap for `famp inspect tasks --id <task_id> --json`, poll with a deadline (never fixed sleep) until both sides show the matching terminal `(class, mode, terminal)` combination per `crates/famp-inspect-server/src/parse.rs::derive_fsm_state` (RESEARCH §6): `("deliver","completed",true) -> "COMPLETED"` etc.

**Test-run command:** `cargo test -p famp-gateway --test e2e_cross_host_delivery` — plain `cargo test`, NOT `cargo nextest -p famp` (memory: nextest hangs on this workspace's `--list` phase for the `famp` crate; `famp-gateway`'s own test binaries are unaffected but stay consistent).

---

## Shared Patterns

### Bus-client error mapping
**Source:** `crates/famp-gateway/src/principal.rs:73-91` (`map_bus_client_err`)
**Apply to:** any new `ProxiedPrincipal` method issuing `send_recv` — reuse this fn unchanged rather than writing a new match arm.

### ChildGuard RAII for spawned test subprocesses
**Source:** `crates/famp-gateway/tests/common/child_guard.rs` (imported via `#[path = ...] mod child_guard;`)
**Apply to:** every broker/gateway child in `e2e_cross_host_delivery.rs` — memory: `project_test_child_guard_convention.md`, mandatory or leaked tmp-socket brokers respawn.

### Poll-with-deadline, never fixed sleep
**Source:** `crates/famp-gateway/tests/liveness.rs` (`wait_for_broker_socket`, `poll_until_all_live`)
**Apply to:** every readiness/convergence check in the new E2E test (socket-up, identities-live, task-terminal-state).

### Value-mutation signing (not typed reconstruction)
**Source:** RESEARCH §5, Pitfall 3 — `famp::sign_value` accepts `&serde_json::Value` directly, canonicalizes internally.
**Apply to:** the egress drain/sign path — avoids needing new public accessors on `BusEnvelope<B>`/`WireEnvelope<B>` (currently private beyond `.body()`/`.class()`).

### Single verify authority (no double-verify)
**Source:** Phase 8 D-07/D-08, `crates/famp-gateway/src/verify.rs`
**Apply to:** the ingress handler — `verify_inbound_any` against the gateway's own `~/.famp/gateway/peers.keyring` is the ONLY signature check; never route through `FampSigVerifyLayer`/`build_router`.

## No Analog Found

None — every new/modified file for Phase 9 has a same-repo analog (either an existing preserved crate to wrap, or the very file being extended).

## Metadata

**Analog search scope:** `crates/famp-gateway/`, `crates/famp-transport-http/`, `crates/famp/src/cli/peer/`, `crates/famp-gateway/tests/`, `crates/famp/tests/_deferred_v1/`, `crates/famp/tests/fixtures/cross_machine/`
**Files scanned:** `principal.rs`, `registry.rs`, `verify.rs`, `main.rs`, `error.rs` (famp-gateway); `server.rs`, `transport.rs`, `middleware.rs` (famp-transport-http); `identity.rs` (famp cli peer); `liveness.rs`, `child_guard.rs` (famp-gateway tests); `e2e_two_daemons.rs.deferred` (famp tests)
**Pattern extraction date:** 2026-07-27
