---
phase: 04-minimal-http-transport-cross-machine-example
reviewed: 2026-04-13T00:00:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - crates/famp-transport-http/Cargo.toml
  - crates/famp-transport-http/src/lib.rs
  - crates/famp-transport-http/src/error.rs
  - crates/famp-transport-http/src/middleware.rs
  - crates/famp-transport-http/src/server.rs
  - crates/famp-transport-http/src/transport.rs
  - crates/famp-transport-http/src/tls.rs
  - crates/famp-transport-http/src/tls_server.rs
  - crates/famp-envelope/src/peek.rs
  - crates/famp-envelope/src/lib.rs
  - crates/famp/src/runtime/peek.rs
  - crates/famp/examples/cross_machine_two_agents.rs
  - crates/famp/examples/_gen_fixture_certs.rs
  - crates/famp/tests/common/cycle_driver.rs
  - crates/famp/tests/common/mod.rs
  - crates/famp/tests/http_happy_path.rs
  - crates/famp/tests/cross_machine_happy_path.rs
  - crates/famp/tests/adversarial.rs
  - crates/famp/tests/adversarial/harness.rs
  - crates/famp/tests/adversarial/fixtures.rs
  - crates/famp/tests/adversarial/memory.rs
  - crates/famp/tests/adversarial/http.rs
  - crates/famp-transport-http/tests/middleware_layering.rs
  - .github/workflows/ci.yml
findings:
  critical: 0
  high: 0
  medium: 3
  low: 4
  info: 3
  total: 10
status: findings
---

# Phase 4: Code Review Report

**Reviewed:** 2026-04-13
**Depth:** standard
**Files Reviewed:** 24
**Status:** findings (no Critical/High; several Medium correctness/operability notes)

## Summary

The Phase 4 HTTP transport binding is well-structured and meets its security-critical
invariants: `FampSigVerifyLayer` runs before any handler dispatch; body limit is layered
outside the sig-verify layer (belt-and-braces cap inside too); canonicalization pre-check
mirrors `loop_fn.rs` byte-for-byte; `verify_strict` semantics are inherited from
`famp-crypto` via `AnySignedEnvelope::decode`; the keyring is the only source of trust and
unknown senders are rejected with 401; error variants do not leak key/secret material;
`#![forbid(unsafe_code)]` is set on the library; rustls-only TLS is enforced and the CI
workflow hard-fails if `openssl` or `native-tls` appear in the dep tree (D-F4 gate).

Findings below are operability, correctness-of-edge-cases, and consistency issues — none
rise to CRITICAL or HIGH for the v1 reference implementation.

## Medium

### MED-01: `tls.rs::load_pem_cert` silently returns an empty Vec for garbage PEM input

**File:** `crates/famp-transport-http/src/tls.rs:43-47` (and test at :129-137 documenting the behavior)

**Issue:** `rustls_pemfile::certs` returns an empty iterator on non-PEM input rather than
an error, and `load_pem_cert` propagates that as `Ok(vec![])`. The unit test explicitly
asserts this behavior. Downstream, `build_client_config(Some(&garbage_path))` would then
call `Verifier::new_with_extra_roots(vec![])` — which is indistinguishable from the
`None` (OS-roots-only) case. A user who typos their `--trust-cert` path or points it at a
non-PEM file will get a client that silently trusts only the OS root store, which in the
dev workflow (self-signed peer cert) means every TLS handshake will fail with an opaque
error far from the configuration mistake.

**Fix:** Return `TlsError::NoPrivateKey`-style typed error when `certs` yields zero
items, or add a distinct `TlsError::NoCertificatesInPem` variant and surface it from
`build_client_config` when `Some(path)` was supplied:

```rust
pub fn load_pem_cert(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut rd = BufReader::new(File::open(path)?);
    let out: Vec<_> = rustls_pemfile::certs(&mut rd).collect::<Result<_, _>>()?;
    if out.is_empty() {
        return Err(TlsError::NoCertificatesInPem);
    }
    Ok(out)
}
```

### MED-02: `middleware.rs` canonical pre-check does not short-circuit `Value`-based re-canonicalization of bodies with numeric precision quirks

**File:** `crates/famp-transport-http/src/middleware.rs:95-104`

**Issue:** The middleware parses with `from_slice_strict::<serde_json::Value>` then calls
`canonicalize(&parsed)` and compares to `bytes`. This mirrors `loop_fn.rs` which is the
intended contract, so behavior is consistent across transports — good. However, this
code path decodes the envelope TWICE: once as `Value` here, once as `AnySignedEnvelope`
via `decode(&bytes, &pinned)` at line 107. On a ~1 MB envelope this is a measurable hot
path, and any future divergence between `Value`-canonicalization and
`AnySignedEnvelope`-canonicalization would silently break CONF-07 distinguishability.

**Fix:** Not urgent — keep the double-decode for Phase 4 as it is load-bearing for the
adversarial matrix. Add a doc comment explicitly stating that the `Value` path must stay
byte-identical to the typed decode path, and add a property test (or a fixed snapshot)
that canonicalizes each envelope variant via both paths and asserts equality. Without
that invariant pinned, a serde-layer refactor could desynchronize them undetectably.

### MED-03: `HttpTransport::send` hand-rolls URL construction with manual percent-encoding

**File:** `crates/famp-transport-http/src/transport.rs:139-149`

**Issue:** Percent-encoding is done by `str::replace(':', "%3A").replace('/', "%2F")`.
This is correct for the specific `agent:local/name` shape produced by `Principal::to_string`
TODAY, but it will silently corrupt any principal containing `%`, `+`, `?`, `#`, or
non-ASCII characters if the `Principal` grammar is ever widened. The comment even
acknowledges this: "Space/control chars already excluded by Principal's parser; we only
need to escape `:` and `/`." That's a parser-coupling assumption that lives far from the
parser.

**Fix:** Use `percent_encoding::utf8_percent_encode` with the `PATH_SEGMENT` encode set,
or use `Url::path_segments_mut().push(&recipient_str)` which does RFC-correct segment
encoding. Either is ~5 lines and removes the coupling. Example:

```rust
use url::Url;
let mut inbox_url = base.clone();
inbox_url.set_path(""); // clear any trailing path
{
    let mut segs = inbox_url
        .path_segments_mut()
        .map_err(|()| HttpTransportError::InvalidUrl(url::ParseError::RelativeUrlWithoutBase))?;
    segs.extend(&["famp", "v0.5.1", "inbox"]);
    segs.push(&msg.recipient.to_string());
}
```

## Low

### LOW-01: `http_happy_path.rs` uses `tokio::time::sleep(300ms)` as a "settle" — flaky on slow CI

**File:** `crates/famp/tests/http_happy_path.rs:117`

**Issue:** 300 ms "let both servers accept" sleep. On cold CI runners this occasionally
won't be enough, and on fast runners it's wasted time. This is a known Phase 4
observation — not a blocker — but it's the kind of flake that surfaces at the worst time.

**Fix:** Replace with a connection-probe loop: spin a `TcpStream::connect` retry against
each bound addr with a tight backoff and a 2 s ceiling. Or use `tokio::task::yield_now`
in a bounded loop until both `local_addr()` sockets respond to a TLS ClientHello. Keep
the 300 ms as the hard cap, not the expected wait.

### LOW-02: `transport.rs::recv` holds the outer `Mutex<HashMap>` across `rx.recv().await`

**File:** `crates/famp-transport-http/src/transport.rs:177-193`

**Issue:** The comment explicitly acknowledges this ("the single-receiver-per-principal
contention model means this is uncontended in practice") and `#[allow(clippy::significant_drop_tightening)]`
suppresses the clippy warning. Factually correct for the current single-receiver model,
but the lock is also what `register()` takes for WRITE on every new principal
registration. If a caller registers a new principal while another task is parked in
`recv`, `register` will block indefinitely. This is not a deadlock in the strict sense
(recv wakes on message delivery), but it is a liveness bug in any scenario where
principals are added after a recv is already parked.

**Fix:** Move `receivers: Mutex<HashMap<Principal, mpsc::Receiver<...>>>` to
`HashMap<Principal, Mutex<mpsc::Receiver<...>>>` wrapped in an outer `RwLock` or an
`arc-swap`, so `recv` takes a read-lock on the map, acquires the per-receiver mutex, then
drops the map lock before awaiting. Or take the `Receiver` out of the map by-value on
first use and keep it in a per-principal slot.

### LOW-03: `middleware.rs::call` uses `to_bytes(body, ONE_MIB)` with the same cap as the outer layer — not actually belt-and-braces

**File:** `crates/famp-transport-http/src/middleware.rs:78-81`

**Issue:** The outer `RequestBodyLimitLayer::new(ONE_MIB)` caps at 1 MiB, and the inner
`to_bytes(body, ONE_MIB)` also caps at 1 MiB. A request body sized between 1 MiB and
whatever `hyper`'s internal allocations allow will be refused by the outer layer, which
returns a generic `tower_http` response (not `MiddlewareError::BodyTooLarge`). The inner
check is unreachable given correct outer-layer ordering — it protects only against a
future refactor that drops the outer layer.

**Fix:** Either (a) document this as an ordering assertion with a compile-fail test that
fails if the outer layer is removed, or (b) widen the inner cap to e.g.
`ONE_MIB.saturating_add(16 * 1024)` so the inner check is the redundant one AND produces
the uniform typed error. Option (b) matches the "belt-and-braces" wording.

### LOW-04: `server.rs::inbox_handler` returns `MiddlewareError::Internal` on inbox send failure (lossy diagnostics)

**File:** `crates/famp-transport-http/src/server.rs:89-95`

**Issue:** When `tx.send(TransportMessage { ... })` fails (channel closed because the
receiver was dropped), the handler maps the error to `MiddlewareError::Internal` and logs
nothing. For a reference protocol implementation, this is diagnosable only by sampling
the 500 response slug — a debugger has to re-read the source to know what the slug means.

**Fix:** Add a `tracing::error!` (or `eprintln!` if tracing is not yet wired in Phase 4)
with the sender+recipient on this branch. Consider a distinct `MiddlewareError::InboxGone`
slug for operability, still mapped to 500 per D-C6.

## Info

### INF-01: `lib.rs` has stale `use _ as _` silencers for `famp_crypto` and `serde_json`

**File:** `crates/famp-transport-http/src/lib.rs:7-8`

**Issue:** Comment says "Silencers for dependencies still pending wiring after Plan 04-03.
As each later plan lands, remove the matching `use _ as _;` line." `serde_json` is in
fact used transitively through `famp-canonical`/`middleware.rs`, and `famp_crypto` is
used transitively through `famp-envelope::decode`. The silencers are likely there because
the direct `[dependencies]` entry exists but no top-level `use` names them — i.e. they
are library-level workspace hygiene placeholders, not pending wiring.

**Fix:** Update the comment to reflect reality, or drop the direct dep entries if they
really are unused at the source level. No functional impact.

### INF-02: `error.rs::HttpTransportError::TlsConfig(String)` loses typed info

**File:** `crates/famp-transport-http/src/error.rs:71-72` and `transport.rs:58-59`

**Issue:** `HttpTransportError::TlsConfig(String)` is populated via `format!("{e:?}")` on
a `TlsError`. This discards the typed enum so downstream callers cannot match on
`NoPrivateKey` vs `Rustls(_)` vs `Io(_)`. Not a bug, but goes against the
`thiserror`-in-libs convention that motivated adopting `thiserror` in the first place.

**Fix:** Store `#[source] TlsError` instead of `String`:

```rust
#[error("tls config error")]
TlsConfig(#[from] crate::tls::TlsError),
```

### INF-03: `examples/_gen_fixture_certs.rs` uses `env!("CARGO_MANIFEST_DIR")` + hardcoded `tests/fixtures/cross_machine` — will write to repo on `cargo run`

**File:** `crates/famp/examples/_gen_fixture_certs.rs:33-42`

**Issue:** Running this example overwrites the committed fixture certs under
`crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}`. This is the intended
workflow per the module docstring, but it means `cargo run --example _gen_fixture_certs`
is a destructive action on the working tree with no confirmation. The leading `_` helps,
but a stray tab-completion could still hit this. Consider a `REGENERATE=1` env-gate
guard.

**Fix:** Optional. Add:

```rust
if std::env::var("REGENERATE").ok().as_deref() != Some("1") {
    eprintln!("Refusing to overwrite committed fixtures. Re-run with REGENERATE=1.");
    std::process::exit(1);
}
```

---

## Security invariants verified (no findings)

- **INV-10 / TRANS-09 SC#2:** Middleware rejects unsigned / wrong-key / canonical-divergence
  bytes BEFORE handler dispatch; verified both by the `middleware_layering.rs` integration
  test (AtomicBool sentinel) and by the `adversarial/http.rs` mpsc-sentinel test. The
  handler's only visible side-effect is an `mpsc::Sender::send`, and both tests confirm
  that send never happens on adversarial input.
- **verify_strict, not verify:** `AnySignedEnvelope::decode` is the only verification path;
  it is inherited unchanged from Phase 2 and uses `ed25519_dalek::VerifyingKey::verify_strict`
  per the tech stack contract. Middleware does not call `verify` directly.
- **TOFU keyring as single trust source:** `FampSigVerifyService::call` consults only
  `self.keyring.get(&sender)`; there is no implicit-trust fallback. `UnknownSender` is a
  hard 401.
- **Two-phase decode shape:** `peek_sender` uses `from_slice_strict` (duplicate-key
  rejecting) before the keyring lookup, ensuring the sender field cannot be JSON-smuggled.
- **Body limit layering:** `RequestBodyLimitLayer` is outermost (first to run), sig-verify
  second, handler last. Confirmed in `server.rs::build_router` and tested in
  `middleware_layering.rs::body_over_1mb_does_not_enter_handler`.
- **Error disclosure:** `MiddlewareError::into_response` emits a fixed-slug JSON body; no
  key material, no stack traces, no arbitrary request data. `HttpTransportError`
  variants include `Principal` (public identifier) and HTTP status codes only.
- **TLS purity:** `rustls 0.23` only, `aws-lc-rs` as the installed crypto provider, no
  `openssl` / `native-tls`. CI workflow hard-fails via `cargo tree -i openssl` and
  `cargo tree -i native-tls` gates.
- **`#![forbid(unsafe_code)]`** set at the crate root of `famp-transport-http` and
  `famp-envelope`.
- **`thiserror` in libs, `anyhow` nowhere:** Verified across all reviewed files.

---

_Reviewed: 2026-04-13_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
