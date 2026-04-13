---
phase: 03-memorytransport-tofu-keyring-same-process-example
verified: 2026-04-13T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 03: MemoryTransport + TOFU Keyring + Same-Process Example — Verification Report

**Phase Goal:** A single developer runs `request → commit → deliver → ack` end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on MemoryTransport.

**Verified:** 2026-04-13
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | famp-transport exposes a Transport trait (async send + incoming stream); MemoryTransport is in-process, no network, no TLS. | VERIFIED | `crates/famp-transport/src/lib.rs` defines `pub trait Transport` with native AFIT `send`/`recv` returning `impl Future<…> + Send`. `crates/famp-transport/src/memory.rs` implements `MemoryTransport` over `tokio::sync::mpsc::unbounded_channel` — no network, no TLS, no `serde`/`reqwest`/`rustls` deps in `Cargo.toml`. 5 unit tests green (`memory::tests::happy_path_roundtrip`, `fifo_ordering`, `unknown_recipient_returns_typed_error`, `cross_principal_isolation`, `send_raw_for_test_is_gated_and_works`). |
| 2 | TOFU keyring is a local-file `HashMap<Principal, VerifyingKey>`; principal = raw 32-byte Ed25519 pubkey; loadable from file OR `--peer` flags. | VERIFIED | `crates/famp-keyring/src/lib.rs` defines `Keyring { map: HashMap<Principal, TrustedVerifyingKey> }` with `load_from_file` / `save_to_file` / `with_peer` / `pin_tofu`. `crates/famp-keyring/src/peer_flag.rs` parses `agent:<auth>/<name>=<b64url-pubkey>`. `peer_flag::peer1_valid_flag_parses` and `tofu1_idempotent_same_key_repin` / `tofu2_different_key_rejected_as_key_conflict` green. |
| 3 | `cargo run --example personal_two_agents` exits 0 and prints a typed conversation trace `request → commit → deliver → ack` over MemoryTransport (CONF-03). | VERIFIED | Live execution: `cargo run -p famp --example personal_two_agents` exit 0; stdout contains the four ordered lines `[1] alice -> bob: Request`, `[2] bob -> alice: Commit`, `[3] bob -> alice: Deliver`, `[4] alice -> bob: Ack`, then `OK: personal_two_agents complete`. Subprocess test `famp::example_happy_path::personal_two_agents_exits_zero_with_expected_trace` PASS. |
| 4 | CONF-05 (unsigned) / CONF-06 (wrong-key) / CONF-07 (canonical divergence) each fail closed with a distinct typed `RuntimeError` when injected into MemoryTransport; no panics, no silent drops. | VERIFIED | `crates/famp/tests/adversarial.rs`: `conf_05_unsigned_message_rejected` asserts `Decode(MissingSignature)`; `conf_06_wrong_key_signature_rejected` asserts `Decode(SignatureInvalid)`; `conf_07_canonical_divergence_rejected` asserts `CanonicalDivergence`. All three PASS in nextest. Pre-decode canonical re-check in `crates/famp/src/runtime/loop_fn.rs` runs before `AnySignedEnvelope::decode`, ensuring CONF-06 vs CONF-07 are textually distinct variants. Fixture `tests/fixtures/conf-07-canonical-divergence.json` committed. |
| 5 | Keyring file format round-trip tested (load → save → load byte-identical) and committed as fixture. | VERIFIED | Two fixtures committed: `crates/famp-keyring/tests/fixtures/two_peers.keyring` (human form) and `two_peers.canonical.keyring` (canonical save form). `roundtrip::rt1b_canonical_fixture_round_trips_byte_identical` asserts `std::fs::read(saved) == std::fs::read(canonical_fixture)` after a load/save cycle. PASS. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-transport/src/lib.rs` | Transport trait + TransportMessage | VERIFIED | `pub trait Transport` + `pub struct TransportMessage` present; native AFIT, no async-trait. |
| `crates/famp-transport/src/memory.rs` | MemoryTransport impl + test-util gate | VERIFIED | `impl Transport for MemoryTransport`; `#[cfg(feature = "test-util")] pub async fn send_raw_for_test`; mpsc-based hub. |
| `crates/famp-transport/src/error.rs` | MemoryTransportError narrow enum | VERIFIED | `UnknownRecipient`, `InboxClosed` variants. |
| `crates/famp-keyring/src/lib.rs` | Keyring + load/save/with_peer/pin_tofu | VERIFIED | All listed methods present; map field private. |
| `crates/famp-keyring/src/error.rs` | KeyringError narrow enum | VERIFIED | `DuplicatePrincipal`, `DuplicatePubkey`, `MalformedEntry`, `KeyConflict`, `InvalidPeerFlag`, `Io`, `Crypto` variants. |
| `crates/famp-keyring/src/file_format.rs` | Line parser + serializer | VERIFIED | `parse_line`, `serialize_entry` present; rejects inline `#`, tolerates `\r`. |
| `crates/famp-keyring/src/peer_flag.rs` | `parse_peer_flag` | VERIFIED | `=` separator parser; tests confirm. |
| `crates/famp-keyring/tests/fixtures/two_peers.keyring` | Two-peer fixture (alice+bob) | VERIFIED | Both files committed (human + canonical forms). |
| `crates/famp/src/runtime/error.rs` | RuntimeError with distinct adversarial variants | VERIFIED | `UnknownSender`, `Decode`, `CanonicalDivergence`, `RecipientMismatch`, `Transport`, `Keyring`, `Fsm`. |
| `crates/famp/src/runtime/peek.rs` | peek_sender helper | VERIFIED | Uses `famp_canonical::from_slice_strict`. |
| `crates/famp/src/runtime/adapter.rs` | fsm_input_from_envelope | VERIFIED | Returns `None` for Ack and Request (Rule-1 fix); drives transitions for Commit/Deliver/Control. |
| `crates/famp/src/runtime/loop_fn.rs` | process_one_message 6-step pipeline | VERIFIED | Sequence: peek → keyring lookup → canonical pre-check → decode → recipient cross-check → FSM step. Canonical check textually precedes `AnySignedEnvelope::decode`. Zero `unwrap`/`expect` in `src/`. |
| `crates/famp/examples/personal_two_agents.rs` | Single-binary happy-path | VERIFIED | Spawns alice + bob tasks, runs full request/commit/deliver/ack; live run exits 0 with trace. |
| `crates/famp/tests/example_happy_path.rs` | Subprocess happy-path test | VERIFIED | Invokes example via `env!("CARGO")`, asserts trace + exit code. PASS. |
| `crates/famp/tests/adversarial.rs` | CONF-05/06/07 tests | VERIFIED | All three tests PASS asserting distinct variants; uses `send_raw_for_test`; compile-time `_require_test_util` probe. |
| `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` | CONF-07 fixture | VERIFIED | Committed; pretty-printed envelope with valid Ed25519 sig over canonical form. |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `loop_fn.rs::process_one_message` | `famp_envelope::AnySignedEnvelope::decode` | two-phase decode after peek + keyring lookup | WIRED |
| `loop_fn.rs::process_one_message` | `famp_canonical::canonicalize` | pre-decode canonical divergence check, runs textually before decode | WIRED |
| `loop_fn.rs::process_one_message` | `famp_fsm::TaskFsm::step` | via adapter.rs `fsm_input_from_envelope` | WIRED |
| `personal_two_agents.rs` | `process_one_message` | per-agent spawned task drives runtime | WIRED |
| `tests/adversarial.rs` | `MemoryTransport::send_raw_for_test` | test-util feature in `[dev-dependencies]` only | WIRED |
| `crates/famp/Cargo.toml` `[dev-dependencies]` | `famp-transport` `features=["test-util"]` | compile-time isolation | WIRED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---------|---------|--------|--------|
| Full workspace test suite green | `cargo nextest run --workspace` | 226/226 passed | PASS |
| Workspace clippy clean (-D warnings) | `cargo clippy --workspace --all-targets -- -D warnings` | clean exit | PASS |
| Example binary runs and prints expected trace | `cargo run -p famp --example personal_two_agents` | exit 0; 4-line trace + `OK: personal_two_agents complete` | PASS |
| CONF adversarial tests asserting distinct variants | `cargo nextest run -p famp --test adversarial` | 3/3 PASS | PASS |
| Keyring round-trip byte-identical | `cargo nextest run -p famp-keyring --test roundtrip` | 7/7 PASS incl. `rt1b_canonical_fixture_round_trips_byte_identical` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TRANS-01 | 03-01 | Transport trait (async send + incoming stream) | SATISFIED | `crates/famp-transport/src/lib.rs` `pub trait Transport` |
| TRANS-02 | 03-01 | MemoryTransport in-process impl | SATISFIED | `crates/famp-transport/src/memory.rs` impl + 5 unit tests |
| KEY-01 | 03-02 | `HashMap<Principal, TrustedVerifyingKey>` keyring | SATISFIED | `crates/famp-keyring/src/lib.rs`; REQUIREMENTS.md wording updated per D-A1 |
| KEY-02 | 03-02 | File format round-trip tested + committed | SATISFIED | Two committed fixtures + `rt1b_canonical_fixture_round_trips_byte_identical` |
| KEY-03 | 03-02 | `--peer` CLI bootstrap path | SATISFIED | `peer_flag.rs::parse_peer_flag` + 3 peer-flag tests |
| EX-01 | 03-04 | `personal_two_agents.rs` happy path exits 0 | SATISFIED | Live run + subprocess test PASS |
| CONF-03 | 03-03 | Happy path two-node MemoryTransport | SATISFIED | Example + subprocess test PASS |
| CONF-05 | 03-04 | Unsigned message rejected | SATISFIED | `conf_05_unsigned_message_rejected` PASS asserting `Decode(MissingSignature)` |
| CONF-06 | 03-04 | Wrong-key signature rejected | SATISFIED | `conf_06_wrong_key_signature_rejected` PASS asserting `Decode(SignatureInvalid)` |
| CONF-07 | 03-04 | Canonicalization divergence detected | SATISFIED | `conf_07_canonical_divergence_rejected` PASS asserting `CanonicalDivergence` (distinct from CONF-06; pre-decode) |

All 10 declared requirement IDs satisfied. No orphaned requirements (REQUIREMENTS.md does not list any additional Phase 3 IDs unclaimed by plans).

### Anti-Patterns Found

None. Per Plan 03-03 summary `rg "unwrap\(\)|\.expect\("` against `crates/famp/src/runtime/` returns zero matches (test-only allowances are scoped behind `#![allow(clippy::unwrap_used, clippy::expect_used)]`). Workspace clippy with `-D warnings` is clean — pedantic + all lints denied at workspace level — providing a stronger gate than ad-hoc grep.

### Human Verification Required

None. All success criteria are programmatically verifiable: the example binary exits 0, the subprocess integration test asserts the trace, the adversarial matrix asserts distinct typed variants via `matches!`, and the round-trip test compares bytes. No visual/UX/external-service surface in this phase.

### Gaps Summary

No gaps. Phase 3 closes its full requirements set (TRANS-01/02, KEY-01/02/03, EX-01, CONF-03/05/06/07). Implementation matches plan intent; the two minor planned deviations (Rule-1 fix to `fsm_input_from_envelope` Request handling, and the Wave 1 clippy lock-scope tightening in `MemoryTransport::send`) are documented in summaries and do not change the goal contract. The phase goal — "single developer runs request→commit→deliver→ack end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on MemoryTransport" — is fully achieved.

---

_Verified: 2026-04-13_
_Verifier: Claude (gsd-verifier)_
