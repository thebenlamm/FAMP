# Phase 11: Shipping-Client Remote Addressing + Setup Hardening - Pattern Map

**Mapped:** 2026-07-28
**Files analyzed:** 8 edit/create sites (5 code, 1 fixture set, 1 CI, 1 doc)
**Analogs found:** 8 / 8 (all in-tree; this phase is *wiring*, not building)

> Every new mechanic already exists and is tested elsewhere in the repo. The
> job is to copy the E2E's envelope shape into `famp send`, copy `home.rs`'s
> env→file resolution for own-domain, and copy the existing `#[error(...)]`
> `#[from]` chain style to un-swallow the transport error. Do not invent
> new patterns.

## File Classification

| Edit/Create Site | Role | Data Flow | Closest Analog | Match Quality |
|------------------|------|-----------|----------------|---------------|
| `crates/famp/src/cli/send/mod.rs::build_envelope_value` (~L413) | CLI / envelope-builder | transform (args→wire Value) | itself (local path L425-434) + `e2e_cross_host_delivery.rs` `build_request`/`unsigned_value` | exact (in-file + ground-truth) |
| own-domain config read (NEW fn, D-05) | config | request-response (env/file→String) | `crates/famp/src/cli/home.rs::resolve_famp_home` | exact (env→fallback idiom) |
| `crates/famp/src/cli/peer/export.rs::run_at` (L31-53) — derive/validate label authority | CLI / config-consumer | transform | itself (already parses `Principal`) | exact (in-file) |
| `crates/famp-transport-http/src/error.rs:63` + `egress.rs:211,273` (D-06) | error type + logging | error-propagation | `egress.rs::RelayError` (L168-179, `#[error]`/`#[from]`/`transparent`) | exact (in-file sibling) |
| shipping-surface e2e test (NEW, D-09) | test | event-driven (2-process) | `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` | exact (reuse `Side`/`ChildGuard`/poll harness) |
| `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` (D-08) | fixture (TLS certs) | file-I/O | RESEARCH cert recipe / `10-HUMAN-UAT.md` | role-match (recipe, no in-tree generator) |
| `.github/workflows/ci.yml` macOS leg (D-08) | CI config | batch | `ci.yml:104-118` `test` matrix job | exact (matrix already present) |
| `crates/famp/tests/gateway_setup_doc_accuracy.rs` (extend, D-07) | test (doc-accuracy) | request-response | itself (`--help` grep gate L23-70) | exact (in-file extend) |
| `docs/GATEWAY-SETUP.md` (D-07) | doc | — | §1-5 structure (L19,41,50,101,143) | doc-only |

## Pattern Assignments

### `crates/famp/src/cli/send/mod.rs::build_envelope_value` (CLI, transform) — PRIMARY EDIT (D-01/D-03/D-04)

**Analog A (in-file, the local path to keep verbatim on parse-failure):** L425-434
```rust
let from = format!("agent:local.bus/{identity}");
let to = match target {
    Target::Agent { name } => format!("agent:local.bus/{name}"),
    Target::Channel { name } => { /* channel-<stripped> */ }
};
```
The conditional (D-01/D-02): if `--to` parses as `Principal`, use it **verbatim** as `to` and stamp `from = agent:{own_domain}/{identity}`; else keep the two lines above. Class stays `audit_log` on the local branch, upgrades to typed `request` on the remote branch (D-04).

**Split-addressing seam (Pattern 1, D-01):** the bus `Target` is built at L228-245 (`Target::Agent { name: name.to_string() }`) — SEPARATE from the envelope `to`. On a remote send the bus `Target.name` must be the **leaf** (`bob`) while envelope `to` is the **full principal** (`agent:hostb.test/bob`). Do NOT set `Target::Agent{name:"agent:hostb.test/bob"}` (Pitfall 2 — routing miss). The `#`-guard at L250-257 stays.

**Analog B (ground-truth typed-request shape, D-03) — `e2e_cross_host_delivery.rs:406-436`:**
```rust
fn unsigned_value<B: BodySchema>(env: UnsignedEnvelope<B>) -> serde_json::Value {
    let dummy_sk = FampSigningKey::from_bytes([42u8; 32]);
    let bytes = env.sign(&dummy_sk).expect(..).encode().expect(..);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect(..);
    value.as_object_mut().expect(..).remove("signature");  // BUS-11: unsigned on bus
    value
}
fn build_request(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = RequestBody {
        scope: serde_json::json!({"task": "..."}),
        bounds: two_key_bounds(),                    // ≥2 of 8 fields set
        natural_language_summary: Some("ping".to_string()),
    };
    let env = UnsignedEnvelope::<RequestBody>::new(
        task_id, from.clone(), to.clone(), AuthorityScope::Advisory, ts(), body);
    unsigned_value(env)
}
```
The fixed `famp send` remote branch MUST emit exactly this shape. This is the sanctioned sign-then-strip (no "encode unsigned" accessor exists). Reply causality (`causality.rel/ref`) is already injected at L494-507 — mirror `.with_causality(..)` for the typed path.

**Existing test scaffold to extend:** `#[cfg(test)] mod tests` at L512+ (`SendArgs { .. }` literal at L519-528; add `--to agent:d/n` remote cases + own-domain resolution + missing-domain error). Note `SendArgs` currently has NO domain field — planner adds `--domain` per D-05.

---

### own-domain config read (NEW, config, request-response) — D-05 / ADDR-03

**Analog:** `crates/famp/src/cli/home.rs::resolve_famp_home` (L11-22) — the ONLY established env→file resolution idiom in the CLI:
```rust
pub fn resolve_famp_home() -> Result<PathBuf, CliError> {
    let path: PathBuf = if let Some(v) = std::env::var_os("FAMP_HOME") {
        PathBuf::from(v)
    } else {
        let home = std::env::var_os("HOME").ok_or(CliError::HomeNotSet)?;
        PathBuf::from(home).join(".famp")
    };
    if !path.is_absolute() { return Err(CliError::HomeNotAbsolute { path }); }
    Ok(path)
}
```
Copy this shape for own-domain: precedence `--domain` flag → `FAMP_OWN_DOMAIN` env → `$FAMP_HOME/own-domain` file → actionable error. Note the module doc's rule: **env read lives in ONE place**; every other call site takes the resolved value / `&Path` to dodge the `std::env::set_var` parallel-test race. Validate the result with `str::parse::<Principal>()` authority rules (see `identity.rs:212 validate_authority` — DNS labels, ≤253B, ASCII) BEFORE stamping `from`. Test idiom: all env cases in ONE `#[test] fn` for serial execution (home.rs L32-65).

**Coupling to preserve (D-05, load-bearing):** `peer export`'s label authority and `famp send`'s `from` authority MUST read the SAME source. See `export.rs:36` — it already `.parse::<Principal>()`s `--as`; the fix either derives its authority from own-domain or validates `--as`'s authority == own-domain and rejects mismatch. This closes the `from == pinned-label` invariant (RESEARCH D-05).

---

### transport error chain un-swallow (D-06, error-propagation) — SEQUENCE FIRST

**Swallow site 1 — `crates/famp-transport-http/src/error.rs:63`:**
```rust
#[error("reqwest failure")]                 // ← discards #[source] in Display
ReqwestFailed(#[source] reqwest::Error),
```
**Swallow site 2 — `crates/famp-gateway/src/egress.rs:211`:**
```rust
.map_err(|e| RelayError::Transport(e.to_string()))  // ← e.to_string() drops .source() chain
```
The log at **egress.rs:273** then prints only the top `Display`:
```rust
eprintln!("famp-gateway: egress[{name}]: failed to relay envelope: {e}");
```

**Analog (the CORRECT pattern, same file, `RelayError` L168-179):** it already uses `#[error(transparent)]` + `#[from]` to preserve inner errors:
```rust
#[error(transparent)]
Sign(#[from] EgressError),
#[error("failed to serialize signed envelope: {0}")]
Encode(#[from] serde_json::Error),
```
Fix: give `ReqwestFailed`/`TlsConfig` a Display that includes the source (`#[error("reqwest failure: {0}")]`), and/or change the egress capture to `RelayError::Transport(format!("{e:?}"))` or walk `.source()`. Add a `famp-transport-http` unit test asserting the Display contains the underlying reason (OBS-01, Wave 0). This must land as **Wave 0/plan-1** — it is the force-multiplier on every subsequent two-machine debug (Pitfall 4).

---

### shipping-surface e2e test (NEW, D-09) — TEST-03

**Analog (reuse wholesale):** `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`.

- **Domain constants** (L86-92): `ALICE = "agent:hosta.test/alice"`, `BOB = "agent:hostb.test/bob"`, `ALICE_DOMAIN/BOB_DOMAIN`. The new test asserts the FIXED `famp send` produces these — instead of the raw-bus `send_bus_envelope` helper.
- **Bus-send seam** (L519-543 `send_bus_envelope`): sends to `Target::Agent { name: to_name }` (bare `"bob"`) while the envelope carries `to = agent:hostb.test/bob` — this is the split-addressing the new test verifies `famp send` now does itself.
- **Harness:** `ChildGuard` (RAII kill+wait — `common/child_guard.rs`, imported L82), `connect_no_spawn` (L526), `poll_inbox_contains` (L553, bounded poll — never a fixed sleep).
- **NEGATIVE test (D-09):** drive a `local.bus`-authority `from` through the federated path → assert a typed error surfaces (ingress `UnpinnedKey` / `unknown_sender`), NOT a silent drop. Trust-lookup keys off `from` (`verify.rs:41,62`).

Throwaway artifacts to "delete" (`wire_proof_inject.rs`, `probe_tls.rs`) are **NOT git-tracked** (RESEARCH verified) — confirm with `git ls-files | grep` and skip; focus on ADDING the shipping test. Must keep `e2e_cross_host_delivery.rs` green.

---

### TLS fixtures regen (D-08) — `crates/famp/tests/fixtures/cross_machine/*`

**Current state (verified):** ECDSA P-256, self-signed, **no EKU, no explicit basicConstraints** — Linux-conditionally-green only. README (L3) still claims "Ed25519" and references `cargo run --example _gen_fixture_certs` which is **gone**.

**Analog / recipe (RESEARCH + `10-HUMAN-UAT.md` canonical recipe):**
```bash
openssl req -x509 -newkey rsa:2048 -nodes -days 800 \
  -keyout <host>.key -out <host>.crt -subj "/CN=<host>" \
  -addext "subjectAltName=IP:127.0.0.1,DNS:localhost" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
  -addext "extendedKeyUsage=serverAuth"
```
Use `127.0.0.1`/`localhost` SANs (E2E binds loopback). CA:FALSE + serverAuth satisfies BOTH Apple SecTrust (macOS) and webpki (Linux) — Pitfall 3. Update the stale README (generator + "Ed25519" claim both wrong). Alt: small `rcgen` example setting `IsCa::NoCa` + `ExtendedKeyUsagePurpose::ServerAuth`.

---

### CI macOS leg (D-08) — `.github/workflows/ci.yml`

**Analog (already present):** the `test` job L104-118:
```yaml
test:
  name: test (${{ matrix.os }})
  runs-on: ${{ matrix.os }}
  strategy:
    fail-fast: false
    matrix:
      os: [ubuntu-latest, macos-latest]
  steps:
    - run: cargo nextest run --workspace --profile ci
```
`macos-latest` already exists → exercises the Apple SecTrust path via `rustls-platform-verifier`. **First D-08 verification step (Open Q2):** confirm `e2e_cross_host_delivery` actually RUNS + is green on the macOS leg with current fixtures (it arguably should FAIL per finding #5) — falsification-with-a-control: current fixtures fail on macOS, regenerated pass. No new job likely needed; confirm before adding one.

---

### doc-accuracy gate extension (D-07) — `crates/famp/tests/gateway_setup_doc_accuracy.rs`

**Analog (extend in-file):** current gate is `--help` flag-grep only (L23-70): runs `famp peer export --help`, asserts stdout `contains("--as")`. Flag-grep **cannot** catch semantic inversion, pin-label direction, ordering, or cert policy (D-07 requirement). Add assertions that parse `GATEWAY-SETUP.md` (path helper L19-21) and check: §4 backs the REMOTE principal, §3 pins under the sender AGENT principal (not `/gateway`), pin-before-launch/no-hot-reload stated, CA:FALSE+serverAuth cert recipe present, macOS firewall pre-auth documented.

---

### `docs/GATEWAY-SETUP.md` (D-07) — doc-only

**Structure (sections to correct):** §1 Prerequisites (L19), §2 Gateway identity (L41), §3 Out-of-band key exchange (L50) — fix pin label to sender agent principal, §4 Start each gateway (L101) — back the remote principal + move "ready" after keyring load + duplicate-pubkey warning, §5 Connect/verify (L143). Replace "self-signed is fine" with the CA:FALSE+serverAuth recipe (above). Add macOS `socketfilterfw` pre-auth. All 8 findings in `10-HUMAN-UAT.md`.

## Shared Patterns

### Principal parse + authority validation
**Source:** `famp_core::Principal` (`str::parse::<Principal>()`), `identity.rs:212 validate_authority`, `identity.rs:245 validate_name_or_instance_id`
**Apply to:** `--to` parse (D-01), own-domain validation (D-05), `peer export --as` (already). Do NOT hand-roll regex — charset/DNS-label rules are strict + tested. Note: `--as` on `famp send` becomes `Hello{bind_as}`, charset `[A-Za-z0-9._-]+` (rejects `:` `/`) — this is WHY own-domain needs its own source.

### Sign-then-strip (BUS-11 unsigned typed wire Value)
**Source:** `e2e_cross_host_delivery.rs:406` `unsigned_value`; also `egress.rs plain_request_value`, `famp/tests/common/cycle_driver.rs`
**Apply to:** every remote (`request`) send in `build_envelope_value`. Sign with `FampSigningKey::from_bytes([42u8;32])`, `.encode()`, parse to Value, `remove("signature")`. Local crypto is throwaway; gateway re-signs at egress.

### thiserror `#[error]` / `#[from]` / `transparent` chain
**Source:** `egress.rs:168-179 RelayError`
**Apply to:** the D-06 fix in `transport-http/src/error.rs` — preserve `#[source]` in the Display string; capture with `{e:?}` at the log site, never `.to_string()`.

### env→file resolution in ONE place
**Source:** `home.rs:11 resolve_famp_home` + module doc (L4-6)
**Apply to:** own-domain resolver. Single env-reading fn; callees take resolved value; all env test cases in one serial `#[test]`.

### Test harness: ChildGuard + bounded poll (no fixed sleeps)
**Source:** `e2e_cross_host_delivery.rs` (`ChildGuard` import L82, `poll_inbox_contains` L553, `connect_no_spawn` L526)
**Apply to:** the new shipping-surface e2e (D-09). Per Memory: tests spawning `famp register`/broker children MUST use ChildGuard or they leak.

## No Analog Found

None. Every site has a strong in-tree analog. The only site without a code
analog is the TLS fixture regen (no in-tree generator survives — the
`_gen_fixture_certs` example was deleted), but the RESEARCH/UAT cert **recipe**
is a concrete role-match.

## Metadata

**Analog search scope:** `crates/famp/src/cli/{send,peer,home,identity}`, `crates/famp-gateway/{src/egress.rs,tests/e2e_cross_host_delivery.rs}`, `crates/famp-transport-http/src/error.rs`, `crates/famp-core/src/identity.rs`, `crates/famp/tests/`, `.github/workflows/ci.yml`, `docs/GATEWAY-SETUP.md`
**Files scanned:** 12 (all code-traced, HIGH confidence)
**Pattern extraction date:** 2026-07-28
