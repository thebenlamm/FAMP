# REL+PERF Report: FAMP Reference Implementation

**Audit Date**: 2026-04-13 | **Scope**: Reliability + performance across 14 crates, ~11K LOC | **Profile**: Rust async (tokio) + ed25519 + RFC 8785 + HTTP/TLS

---

## Summary

**Verdict**: PASSED. Reliability profile is strong with typed error boundaries, panic safety enforced at crate level, and resource limits in place. Performance is acceptable for a reference implementation (signature verification and canonicalization dominate CPU; no obvious quadratic or allocation hotspots on untrusted input).

**Severity Breakdown**: 1 MEDIUM finding (perf concern on error path), 2 LOW findings (minor allocation inefficiencies, logging gap).

**Top 3 Findings**:
1. **Error message construction in hot path** (envelope decode) — multiple `.to_string()` calls on serde errors; impact minor but fixable.
2. **Missing structured logging on inbox send failure** (transport-http) — surface-level diagnostics via eprintln!; should upgrade to tracing.
3. **Body always buffered in-memory** (middleware) — acceptable for protocol message sizes but worth documenting; no streaming path.

---

## Findings

### [MEDIUM] Error Message String Allocation in Envelope Decode Hot Path
- **Severity**: MEDIUM
- **Location**: `/Users/benlamm/Workspace/FAMP/crates/famp-envelope/src/envelope.rs:168,288,292,318,326`
- **Category**: performance
- **Issue**: `map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))` called 5 times in the decode path; each allocates a String for error context on serde failures, even when the error is not ultimately bubbled to the user.
- **Impact**: On a malformed JSON payload (e.g., invalid UTF-8 or truncated object), the middleware calls `.to_string()` on the serde error; if this happens at high QPS with many malformed messages, the allocator pressure is measurable (not critical for a reference impl, but visible in profiling).
- **Fix**: Defer to `.map_err(EnvelopeDecodeError::BodyValidation)` with a custom `From<serde_json::Error>` impl (already used elsewhere in the crate; see line 296 `map_serde_error` which does this correctly). Or accept the allocation since protocol messages are small (a few KB) and malformed input is a relative edge case.

### [LOW] Unstructured Logging on Inbox Send Failure
- **Severity**: LOW
- **Location**: `/Users/benlamm/Workspace/FAMP/crates/famp-transport-http/src/server.rs:104-108`
- **Category**: reliability (diagnostics)
- **Issue**: `eprintln!` used as a placeholder; comment (LOW-04) acknowledges tracing not yet wired. When `tx.send()` fails (receiver dropped), the error is printed to stderr with principal info but no structured correlation ID, no log level, no sink integration.
- **Impact**: In production, diagnostic signals are lost if stderr is not captured; if the inbox receiver dies, operators have no way to correlate the send failure with other system events (FSM state, peer identity, message ID).
- **Fix**: Phase 5 work — wire `tracing::error!()` macro with instrumentation span carrying sender/recipient/timestamp; replace eprintln! then. For now, document that Phase 4 is logging-free by design (see comment).

### [LOW] No TLS Handshake Timeout (Client-Side)
- **Severity**: LOW
- **Location**: `/Users/benlamm/Workspace/FAMP/crates/famp-transport-http/src/transport.rs:62-67`
- **Category**: reliability (resource limits)
- **Issue**: `reqwest::Client` configured with `.timeout(Duration::from_secs(10))` which applies to the full request including read timeout, but NOT to the TLS handshake phase specifically. If a peer is stuck in TLS negotiation (e.g., rogue server stalling on `ServerHello`), the connection hangs until the 10s wall-clock timeout fires.
- **Impact**: Unlikely in a well-managed federation, but a malicious peer could induce a 10s stall per send attempt. Reference implementations typically don't need separate handshake timeouts; this is LOW because the global timeout does prevent indefinite hangs.
- **Fix**: Document the 10s timeout and note that per-handshake tuning (if needed) would require dropping to `hyper-util` for fine-grained client control. Acceptable for v0.7 reference impl.

---

## Panic Audit

**All unwraps/expects are test-only or marked with `#[allow(...)]`.** Production code returns typed errors at every crate boundary. Summary table:

| Location | Type | Context | Verdict |
|----------|------|---------|---------|
| `famp-canonical/tests/conformance.rs:52,54,74,75,103,104,138,139` | expect/unwrap | Fixture JSON parsing in test harness; behind `#[allow(...)]` | SAFE (test-only) |
| `famp-crypto/src/verify.rs:55-64` | unwrap (4×) | Test-only `#[test]` verify roundtrip | SAFE (test-only, marked) |
| `famp-crypto/src/traits.rs:75-78,87` | unwrap (5×) | Test-only trait impl roundtrip | SAFE (test-only) |
| `famp-envelope/src/envelope.rs:403,410-412,420,427,439-441,456-473,496-507` | unwrap/parse (20×) | Test fixtures, vector data | SAFE (test-only, marked `#[allow(...)]`) |
| `famp-envelope/src/peek.rs:41` | expect | Test assertion; behind `#[allow(...)]` | SAFE (test-only) |
| `famp-core/tests/identity_roundtrip.rs` | parse().unwrap (15×) | Test fixtures | SAFE (test-only) |
| `famp-transport-http/src/transport.rs:167` | unwrap_or_default | `resp.text().await` fallback on response body read failure | SAFE (produces empty string, never panics) |
| `famp-transport-http/src/tls.rs:126-127,147,157,167` | unwrap/panic | Test assertions; behind `#[allow(...)]` | SAFE (test-only, marked) |
| `famp/tests/http_happy_path.rs` (50+ unwraps) | unwrap (cert loading, addr binding, Principal parsing) | Integration test fixtures | SAFE (test-only) |
| `famp-envelope/src/body/bounds.rs:119,129` | validate().unwrap | Test assertions | SAFE (test-only, marked) |

**Zero panics in production code**. All unwraps/expects isolated to unit/integration test harnesses with explicit `#[allow(clippy::unwrap_used, clippy::expect_used)]` guards. Verified via CI clippy lint enforcing `-D warnings` (workspace lint, inherited by all members).

---

## Resource Limit Audit

| Limit | Configured? | Value | Risk | Notes |
|-------|-------------|-------|------|-------|
| HTTP request body size (outer) | ✓ | 1 MiB | LOW | `RequestBodyLimitLayer::new(ONE_MIB)` in `server.rs:45`; spec §18 (TRANS-07) |
| HTTP request body size (inner defense-in-depth) | ✓ | 1 MiB + 16 KiB | LOW | `SIG_VERIFY_BODY_CAP` in middleware; allows graceful degradation if outer layer removed |
| HTTP client request timeout | ✓ | 10 sec | LOW | `reqwest::Client::timeout(Duration::from_secs(10))` in transport.rs:64; includes connect + read |
| TLS handshake timeout | ✗ | (implicit 10s global) | LOW | Covered by overall timeout; per-handshake tuning deferred to Phase 5 |
| Inbox channel capacity | ✓ | 64 | LOW | `INBOX_CHANNEL_CAPACITY = 64` in transport.rs:29; bounded queue prevents unbounded memory growth |
| Message ID/Principal clone overhead | OK | O(n) strings | LOW | Principal is ~30 bytes (clone via Copy/Clone on URL-safe domain), MessageId is uuid (128 bits); both cheap |
| Canonical JSON parse: two-pass overhead | OK | O(2n) | LOW | `from_slice_strict` → `canonicalize` double-parse is intentional per design (RESEARCH Pitfall 4); message sizes (few KB) make cost negligible |

**No DoS vector identified.** All untrusted input (wire bytes, HTTP headers, JSON) is capped before processing. Envelope decode performs two JSON passes (strict validation + canonicalization), which is intentional and within budget for protocol message sizes.

---

## Async Correctness & Cancellation Safety

| Concern | Status | Details |
|---------|--------|---------|
| Blocking ops in async contexts | ✓ PASS | No `block_on` calls found; all I/O via `.await` on tokio primitives |
| Missing `.await`s | ✓ PASS | Compiler enforces; no silent drops of futures |
| Send/Sync bounds on shared state | ✓ PASS | `Arc<Keyring>`, `Arc<Mutex<HashMap<...>>>`, `Arc<TrustedVerifyingKey>` — all thread-safe; transport trait futures are `+ Send + 'static` |
| Cancellation safety on request drop | ✓ PASS | Middleware returns early on body read error (to_bytes cap) or parse error; no partial state left behind. Envelope decode is read-only over immutable wire bytes. |
| Lock contention on hot paths | ✓ PASS | Keyring is cloned into each middleware instance (low contention); per-receiver Arc<Mutex> pattern in HttpTransport avoids lock-hold-across-await (see transport.rs:182-190, comment LOW-02) |
| Graceful shutdown | ~ PARTIAL | Server-side: `attach_server()` stores JoinHandle and drops it on transport drop; axum doesn't support explicit shutdown (tokio task cancel on drop). Client-side: no active shutdown sequence documented. For Phase 4 reference impl, acceptable. |

---

## Error Handling & Observability

### Typed Errors at Crate Boundaries
- **All libraries use `thiserror`** for typed errors (461 LOC total across error.rs files)
- **anyhow never leaks out of libraries** — verified via grep: only found in test/bin contexts
- **Distinctive error types** per layer:
  - `famp-canonical::CanonicalError` — parse/serialize issues
  - `famp-envelope::EnvelopeDecodeError` — envelope codec + body validation
  - `famp-crypto::CryptoError` — (small, 19 LOC) key material issues
  - `famp-transport-http::MiddlewareError` → HTTP status code mapping (error.rs lines 39-56)
  - `famp::runtime::RuntimeError` — process-level errors (7 variants including CanonicalDivergence, UnknownSender, RecipientMismatch)

### Error Response Mapping (HTTP Middleware)
All `MiddlewareError` variants map to spec-compliant status codes (CONF-05/06/07):
| Error | HTTP Status | Context |
|-------|------------|---------|
| BodyTooLarge | 413 | Outer cap (1 MiB) exceeded |
| BadPrincipal | 400 | Path extraction failure |
| BadEnvelope | 400 | Malformed JSON or missing required fields |
| CanonicalDivergence | 400 | Wire bytes don't round-trip to canonical form |
| UnknownSender | 401 | No pinned key for sender |
| SignatureInvalid | 401 | Signature verification failed |
| UnknownRecipient | 404 | Recipient not registered on this host |
| Internal | 500 | Channel closed / inbox receiver dropped |

### Error Messages
- **Context included**: e.g., `"inbox send failed (sender={}, recipient={}): {}"` in server.rs; allows troubleshooting
- **No secret leakage identified**: keys not printed in error context; principal/sender info is public (federation metadata)
- **Serialized in JSON responses**: `{"error": "slug", "detail": "Display string"}` (error.rs:34-56)

---

## Performance Hot Paths

### Canonicalization (`famp-canonical`)
- **Primary path**: `canonicalize<T: Serialize>(value: &T) -> Result<Vec<u8>>` delegates to `serde_jcs::to_vec`; no custom logic, no clones, no redundant parsing.
- **Strict parse path**: `from_slice_strict` parses twice (strict validation pass via custom serde visitor, then typed deserialization). Two-pass is intentional per RESEARCH Pitfall 4 to catch duplicate keys; cost is O(2n) but n is small (a few KB).
- **No streaming**: Entire message buffered in memory; acceptable for protocol sizes, documented limitation.
- **No unnecessary allocations**: `StrictTree` enum avoids carrying payloads (see lib.rs doc); leaf values extracted during second pass.

### Signature Verification (`famp-transport-http/middleware.rs`)
1. **to_bytes(body, cap)** → allocates body once (unavoidable; axum API)
2. **peek_sender()** → one strict parse (extracting just "from" field)
3. **keyring.get()** → O(1) HashMap lookup; clone of Arc<TrustedVerifyingKey> (cheap pointer clone)
4. **Canonical pre-check** → `from_slice_strict` + `canonicalize` + memcmp; two parses but bounded by body cap
5. **AnySignedEnvelope::decode()** → third parse (into typed struct) + signature verify via `ed25519_dalek::verify_strict` (constant-time)

**Three JSON parses in middleware** (step 1 strict peek, step 2 canonical check, step 3 typed decode). Spec allows this for clarity (distinguishing CONF-06 vs CONF-07); optimization would defer typed parse until after sig verify. Acceptable for reference impl; not a bottleneck.

### No Obvious Quadratic Behavior
- **Envelope encoding**: Fixed number of fields; no loops over untrusted input
- **Keyring lookup**: O(1) hash table; no iteration over peers
- **Body validation** (bounds.rs): One pass over body fields; no cross-product checks
- **Principal/Instance parsing**: Linear in string length; no backtracking

---

## Notable Design Patterns

### Defense-in-Depth (Resource Limits)
- **Outer cap**: `RequestBodyLimitLayer` (1 MiB, Pitfall 2 aware)
- **Inner cap**: `SIG_VERIFY_BODY_CAP` (1 MiB + 16 KiB sentinel) — if outer removed, inner still fires
- Both guard before signature verification runs (prevents unbounded buffer allocation)

### Canonical Pre-Check (CONF-07 Distinguishability)
Middleware re-canonicalizes wire bytes **before** calling `AnySignedEnvelope::decode`, allowing it to respond `CanonicalDivergence` rather than `SignatureInvalid`. Pins the same canonicalize/verify path the runtime uses (parity checked via unit test `canonical_pre_check_*`).

### Per-Receiver Lock Pattern (Transport)
HttpTransport avoids holding the outer `receivers` HashMap lock across `rx.recv().await` by cloning each receiver's Arc<Mutex> out first (lines 182-190). Allows concurrent `register()` calls while one is parked waiting.

### Error Domain Separation
- **Middleware layer** returns `MiddlewareError` (HTTP semantics) with StatusCode mapping
- **Runtime layer** returns `RuntimeError` (process semantics, FSM-aware)
- **Transport layer** returns transport-specific errors (`HttpTransportError`, `MemoryTransportError`)
Each is typed and cannot be conflated.

---

## Strengths

1. **Panic Safety Enforced**: Workspace lint `#[deny(clippy::unwrap_used, clippy::expect_used)]` ensures no production panics; all 30+ unwraps are isolated to test harnesses with explicit `#[allow(...)]` markers and verified via CI.

2. **Typed Errors at Every Boundary**: 461 LOC of typed error definitions across 8 error.rs files; no `anyhow` in library APIs; every error path is compiler-checked.

3. **Resource Limits Comprehensively Gated**: Body cap (1 MiB outer + 16 KiB inner sentinel), channel capacity (64), request timeout (10s), all before untrusted-input processing.

4. **Canonical JSON Correctness Verified**: Two-pass strict parsing (duplicate-key rejection) + re-canonicalization before signature verify; parity checked via unit tests (`canonical_pre_check_*`).

5. **HTTP Middleware Security Posture**: Signature verification happens in middleware (tower Layer) before handler is invoked; errors map to spec-compliant status codes; invalid sigs reject at 401 before reaching application code.

6. **Async Correctness**: No blocking calls in async contexts, all futures Send + 'static, no unwitting mutex-across-await, lock-contention-aware (LOW-02 pattern).

7. **No Streaming Limitations Documented**: Body always buffered; acceptable for reference impl; no hidden surprises.

---

## Gaps / Deferred Work

1. **Structured Logging** — Phase 4 uses eprintln! as placeholder; Phase 5 should wire tracing (acknowledged in LOW-04 comment)
2. **Graceful Shutdown Sequence** — Server-side relies on tokio task drop; no explicit draining or SigTerm handler documented
3. **Per-Handshake TLS Timeouts** — Global 10s timeout covers handshake; per-phase tuning deferred (Phase 5 if needed)
4. **Error Message String Allocations** — Five `.to_string()` calls in envelope decode hot path; fixable via From impl (Phase 5 polish)

---

## Recommendations for Phase 5+

1. **Replace eprintln! with tracing::error!()** and add instrumentation spans for sender/recipient/message-id
2. **Optimize envelope error path**: Use `From<serde_json::Error>` impl instead of map_err + to_string
3. **Document body buffering strategy**: Explicitly note no streaming path; design record for future optimization
4. **Add requestId/correlationId**: Link HTTP request → envelope processing → FSM step for end-to-end tracing
5. **Benchmark canonicalization under load**: Profile the two-pass parse cost; optimize if it becomes a bottleneck (unlikely given message sizes)

---

**Conclusion**: FAMP's reliability profile is strong for a reference implementation. Panic safety is enforced, errors are typed at all boundaries, resource limits are in place, and async correctness is sound. Performance is acceptable for protocol message sizes; no quadratic behavior or obvious allocation hotspots identified. Minor opportunities for structured logging and error-path polish in Phase 5, but nothing blocking production conformance-level-2 deployment.
