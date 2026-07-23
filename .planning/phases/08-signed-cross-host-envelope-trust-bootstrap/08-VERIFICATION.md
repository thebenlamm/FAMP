---
phase: 08-signed-cross-host-envelope-trust-bootstrap
verified: 2026-07-23T00:00:00Z
status: passed
score: 4/4 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 8: Signed Cross-Host Envelope + Trust Bootstrap Verification Report

**Phase Goal:** Every envelope crossing the gateway boundary between two machines is Ed25519-signed under INV-10 and carries the forward-compatible fields v1.1/v2.0 need without a wire break, and two machines Ben controls establish mutual key trust via out-of-band export/import with TOFU pinning — with no implicit trust for unpinned keys. Reuses famp-crypto + famp-canonical as-is; extends famp-envelope and wires famp-keyring into the gateway cross-host path.

**Verified:** 2026-07-23
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | WIRE-01: unsigned or signature-invalid cross-host envelope rejected before touching the local bus | ✓ VERIFIED | `crates/famp-gateway/src/verify.rs::verify_inbound` returns `Err(RejectReason::InvalidSignature)` on unsigned/tampered bytes; independently re-ran `rejects_unsigned` + `rejects_bad_signature` — both assert `keyring.len()` unchanged (no state mutation) and pass (2/2 green, confirmed live, not from SUMMARY). No bus/registry call exists on the reject path (function takes no bus handle at all — pure `(bytes, &Keyring) -> Result`). |
| 2 | WIRE-02: extended envelope carries sender/receiver domain + key_id, nonce, expiry, capability/approval omitted-when-empty, round-trips JCS byte-exact, and local-bus envelope stays byte-identical | ✓ VERIFIED | `crates/famp-envelope/src/wire.rs` lines 58-74: all 7 fields (`from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry`, `capability`, `approval`) present as plain `Option` + `skip_serializing_if = "Option::is_none"` — no `serde(flatten)`/`serde(tag)`. Independently re-ran `federation_fields_roundtrip`, `local_bus_byte_identical`, `federation_format_well_formed` (all pass) plus the crate's full doctest suite (6/6 pass, including the updated `compile_fail` doctest). Lockstep verified across `WireEnvelope`, `UnsignedEnvelope`, `WireEnvelopeRef` (both construction sites) via source read. |
| 3 | TRUST-01: `famp peer export` → import round-trip pins the key via TOFU; no key material crosses FAMP | ✓ VERIFIED | `crates/famp/src/cli/peer/{export,import,identity}.rs` read in full: `export::run_at` only ever writes the *public* key + `key_id` to a writer; `import::run_at` only reads a local file/stdin, never a network call. Independently re-ran `crates/famp/tests/peer_roundtrip.rs` (3/3 pass): `export_import_pins_the_exact_key`, `conflicting_repin_fails_closed` (fails closed, original pin survives), `never_imported_principal_is_absent`. `load_or_generate` idempotency independently re-verified (2/2 unit tests pass in `identity.rs`). |
| 4 | TRUST-02: envelope signed by an unpinned key rejected with no state created / no implicit trust | ✓ VERIFIED | `verify_inbound`'s `keyring.get(&from) == None` path returns `Err(RejectReason::UnpinnedKey{principal})` with **no auto-pin call anywhere on that path** (source-read confirmed — the `None` arm returns immediately, never calls `pin_tofu`). Independently re-ran `rejects_unpinned_key` — asserts `keyring.len()` unchanged. `key_id` is never used as a trust-decision input anywhere in `famp-gateway` or `famp-keyring` (grep confirms zero references) — the full 32-byte pinned pubkey via `Keyring::get` is the sole trust anchor, per the D-03/T-08-05 prohibition. |

**Score:** 4/4 truths verified (0 present, behavior-unverified)

All four truths are behavior-dependent (state-mutation / no-implicit-trust invariants) and each has a passing, independently re-run test that exercises the actual invariant — not just symbol presence. Full test suites for the touched crates were also re-run in this session and match the claimed counts exactly: `famp-envelope` 36/36, `famp-crypto` 14/14, `famp-gateway` 4/4, `famp` lib 263/263, `peer_roundtrip` 3/3, `cli_help_invariant` 1/1, `famp-envelope` doctests 6/6. `just lint` (workspace clippy `-D warnings`) exits 0.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-envelope/src/wire.rs` | `WireEnvelope<B>` +7 omit-when-empty fields | ✓ VERIFIED | Confirmed lines 58-74; no flatten/tag |
| `crates/famp-envelope/src/envelope.rs` | 7 fields in lockstep across `UnsignedEnvelope`, `WireEnvelopeRef`, both construction sites, builders, `federation_format_ok()` | ✓ VERIFIED | All sites confirmed via grep + read; 3 new tests present and passing |
| `crates/famp-crypto/src/keys.rs` | `FampSigningKey::generate()` (OsRng), `key_id()` (16-char b64url) | ✓ VERIFIED | Both present, tested, re-exported from `lib.rs` |
| `crates/famp-gateway/src/verify.rs` | `verify_inbound<B>(bytes, &Keyring) -> Result<SignedEnvelope<B>, RejectReason>` | ✓ VERIFIED | Pure function, two-pass `peek_sender` → `keyring.get` → `SignedEnvelope::decode`; no raw `ed25519_dalek::VerifyingKey` construction (grep confirms 0) |
| `crates/famp-gateway/src/error.rs` | `RejectReason` enum, exactly 2 variants | ✓ VERIFIED | `InvalidSignature`, `UnpinnedKey{principal}` — distinct, documented |
| `crates/famp/src/cli/peer/{mod,identity,export,import}.rs` | Peer CLI tree + keypair persistence | ✓ VERIFIED | All 4 files present, wired into `Commands::Peer`, full round-trip test passing |
| `crates/famp/tests/peer_roundtrip.rs` | Single-machine export→import→pin test | ✓ VERIFIED | 3/3 tests pass, in-process, no subprocess |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `famp peer export` (identity.rs) | `~/.famp/gateway/identity.ed25519` | `load_or_generate` generate-once-persist | ✓ WIRED | `load_or_generate_is_idempotent` + `load_or_generate_persists_to_disk` tests pass |
| `famp peer import` | `~/.famp/gateway/peers.keyring` | `Keyring::pin_tofu` + `save_to_file` | ✓ WIRED | Same path helper (`gateway_peers_keyring_path`) used by both import and the round-trip test's assertion read |
| `verify_inbound` | `famp-keyring::Keyring` | `keyring.get(&from)` | ✓ WIRED | This is the *same* keyring type `famp peer import` writes to (Plan 03/04 share the D-06 single-source-of-truth file) — confirmed by shared `gateway_peers_keyring_path` helper and `famp_keyring::Keyring` type used on both sides. Phase 9 is the only remaining gap: wiring the live HTTP handler to load this file and call `verify_inbound` at the actual network boundary — explicitly out of scope for Phase 8 per CONTEXT.md. |
| `famp-envelope` federation fields | `famp-gateway::verify_inbound` | `SignedEnvelope::decode` (routes through `verify_strict`) | ✓ WIRED | `verify.rs` imports and calls `famp::SignedEnvelope::decode`, which uses the envelope's extended wire shape |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|--------------|--------|----------|
| WIRE-01 | 08-03 | Unsigned/invalid envelope rejected before local bus | ✓ SATISFIED | `verify_inbound` reject tests, re-run and green |
| WIRE-02 | 08-01, 08-02 | Extended envelope, forward-compatible, byte-exact round-trip | ✓ SATISFIED | Field lockstep + round-trip/byte-identity tests, re-run and green |
| TRUST-01 | 08-02, 08-04 | Export/import TOFU pin round-trip, no key material crosses FAMP | ✓ SATISFIED | `peer_roundtrip.rs`, re-run and green |
| TRUST-02 | 08-03 | Unpinned key rejected, no implicit trust | ✓ SATISFIED | `rejects_unpinned_key`, re-run and green |

All 4 requirement IDs declared in phase 8 PLAN frontmatters match REQUIREMENTS.md exactly (lines 84-87: WIRE-01, WIRE-02, TRUST-01, TRUST-02 all marked `Complete`/`Phase 8`). **No orphaned requirements** — REQUIREMENTS.md maps no additional IDs to Phase 8 beyond these four.

### Scope-Fence Checks (violations would be findings)

| Check | Status | Evidence |
|-------|--------|----------|
| Nonce/expiry NOT actively enforced | ✓ CONFIRMED | `federation_format_ok()` is format-validate-only (empty-nonce / expiry≤ts flagged; a **past-but-parseable** expiry is explicitly NOT rejected — `past_but_after_ts` test asserts `true`). Zero call sites of `federation_format_ok()` outside its own test module — it is not wired into `verify_inbound` or any enforcement path. No replay-cache code exists in production paths (grep confirms all "replay" hits are unrelated mailbox-replay-protection comments, not nonce/anti-replay). |
| Capability/approval reserved, omit-when-empty, interpreted by nothing | ✓ CONFIRMED | Only pass-through construction sites touch `.capability`/`.approval` (struct literals copying the `Option` value); no `match`/pattern-read of contents anywhere in non-test code (grep confirms). |
| No live two-machine HTTP transport built | ✓ CONFIRMED | `verify_inbound` signature is `(bytes: &[u8], keyring: &Keyring) -> Result<...>` — pure, no socket/handle parameter. `famp-gateway/Cargo.toml` has no HTTP/transport crate (no axum/reqwest/hyper) added this phase. |
| `_deferred_v1/` NOT reactivated | ✓ CONFIRMED | `git log` on `crates/famp/tests/_deferred_v1/` shows only the pre-Phase-8 freeze commits (`91da87d`, `bf33ccb`); no Phase 8 commit touches that directory; no Phase 8 SUMMARY references it. |

### FED-01 Invariant Change (D-05, flagged for confirmation)

`crates/famp/tests/cli_help_invariant.rs`'s `famp_help_omits_deleted_federation_verbs` test was modified by plan 08-04 to permit the `famp peer` verb (previously asserted absent since the Phase 4 CLI purge). **Confirmed intentional and decision-backed**: CONTEXT.md D-05 explicitly locks reintroducing `famp peer export/import` (a different shape — Ed25519 TOFU against `famp-keyring` — than the deleted TOML `peers.toml` `add`/`import` surface). The test file's own header comment documents this rationale in full, and the modified test still asserts the OLD `peer add` subcommand and `init`/`setup`/`listen` verbs stay gone while only the NEW `export`/`import` shape is advertised. Independently re-ran: 1/1 pass. This is a planned, documented change, not a silent regression.

### Anti-Patterns Found

None. Grepped all 17 files modified across the 4 plans for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|not yet implemented|coming soon` — zero matches.

### Behavioral Spot-Checks / Test Re-Runs (independently executed this session, not taken from SUMMARY)

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| WIRE-02 round-trip + byte-identity + format-well-formed | `cargo test -p famp-envelope --lib federation`, `local_bus_byte_identical` | 3/3 pass | ✓ PASS |
| famp-envelope full suite | `cargo test -p famp-envelope --lib` | 36/36 pass | ✓ PASS |
| famp-envelope doctests (incl. updated compile_fail) | `cargo test -p famp-envelope --doc` | 6/6 pass | ✓ PASS |
| famp-crypto generate()/key_id() | `cargo test -p famp-crypto --lib generate`, `key_id` | 2/2, 1/1 pass | ✓ PASS |
| famp-crypto full suite | `cargo test -p famp-crypto --lib` | 14/14 pass | ✓ PASS |
| WIRE-01/TRUST-02 verify_inbound | `cargo test -p famp-gateway --lib verify` | 4/4 pass | ✓ PASS |
| TRUST-01 round-trip | `cargo test -p famp --test peer_roundtrip` | 3/3 pass | ✓ PASS |
| FED-01 invariant (D-05 update) | `cargo test -p famp --test cli_help_invariant` | 1/1 pass | ✓ PASS |
| famp lib full suite | `cargo test -p famp --lib` | 263/263 pass | ✓ PASS |
| Workspace lint | `just lint` | clean, exit 0 | ✓ PASS |

### Probe Execution

Not applicable — no `scripts/*/tests/probe-*.sh` files exist in this repo and none were declared by the phase's plans.

### Human Verification Required

None. This phase is pure Rust library/CLI code with no UI, no visual output, no external service integration, and no real-time behavior — every observable truth is deterministically testable and was independently re-verified against passing tests in this session.

### Gaps Summary

No gaps. All 4 must-have truths are verified against actual source code and independently re-run passing tests (not SUMMARY.md claims). All scope-fence constraints hold: nonce/expiry are carried+signed but not enforced, capability/approval are reserved and untouched, no live HTTP transport was built, and `_deferred_v1/` remains frozen. The one documented deviation (FED-01 invariant update) is intentional, decision-backed (D-05), and does not represent a regression. Phase 9 remains the correct home for wiring `verify_inbound` to a live two-machine HTTP transport — that gap is explicitly out of scope here, not a Phase 8 failure.

---

_Verified: 2026-07-23_
_Verifier: Claude (gsd-verifier)_
