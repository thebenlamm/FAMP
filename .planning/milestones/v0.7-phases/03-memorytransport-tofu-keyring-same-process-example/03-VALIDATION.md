---
phase: 3
slug: memorytransport-tofu-keyring-same-process-example
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-13
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` + `proptest` + `#[tokio::test]` |
| **Config file** | `Cargo.toml` workspace (nextest defaults) |
| **Quick run command** | `cargo nextest run -p famp-transport -p famp-keyring -p famp --all-features` |
| **Full suite command** | `cargo nextest run --workspace --all-features && cargo run --example personal_two_agents -p famp` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run the task's `<automated>` verify command (scoped to touched crate)
- **After every plan wave:** Run full-suite command
- **Before `/gsd-verify-work`:** Full suite must be green, example binary exits 0
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Plan | Task | Requirements | Verify Command | Key Assertion |
|------|------|--------------|----------------|---------------|
| 03-01 | 1 — Transport trait + error skeleton | TRANS-01 | `cargo check -p famp-transport --all-features && cargo clippy -p famp-transport --all-features --all-targets -- -D warnings` | `pub trait Transport` + `MemoryTransportError` compile clean; zero envelope/keyring deps |
| 03-01 | 2 — MemoryTransport impl + test-util + 5 unit tests | TRANS-02 | `cargo nextest run -p famp-transport --all-features` | 5+ tests green; FIFO, UnknownRecipient, cross-principal isolation, test-util gate |
| 03-02 | 1 — famp-keyring scaffold + Keyring API + file format + peer_flag | KEY-01, KEY-03 | `cargo check -p famp-keyring && cargo clippy -p famp-keyring --all-targets -- -D warnings` | Keyring + KeyringError + parse_peer_flag compile; zero envelope/transport deps |
| 03-02 | 2 — Fixture + round-trip + TOFU + peer-flag tests | KEY-02, KEY-03 | `cargo nextest run -p famp-keyring` | ≥11 tests green; `load → save` byte-identical on canonical fixture; KeyConflict on TOFU collision |
| 03-03 | 1 — famp Cargo.toml deps + runtime scaffold + RuntimeError + peek + adapter | (composition) | `cargo check -p famp && cargo clippy -p famp --all-targets -- -D warnings` | runtime module compiles; test-util in dev-deps only |
| 03-03 | 2 — process_one_message + canonical pre-check + 7 unit tests | (composition) | `cargo nextest run -p famp --test runtime_unit` | 7+ tests green; canonical pre-check ordering verified; no unwrap outside tests |
| 03-04 | 1 — Example binary + subprocess test + REQUIREMENTS.md fix | EX-01, CONF-03 | `cargo run --example personal_two_agents -p famp && cargo nextest run -p famp --test example_happy_path` | Exit 0, ordered request/commit/deliver/ack trace; REQUIREMENTS.md KEY-01 updated |
| 03-04 | 2 — CONF-05/06/07 adversarial tests + fixture | CONF-05, CONF-06, CONF-07 | `cargo nextest run -p famp --test adversarial` | 3 tests green, each asserting a DISTINCT RuntimeError variant |

### Coverage Matrix

| Requirement | Plan/Task | Covered By |
|-------------|-----------|------------|
| TRANS-01 | 03-01 / Task 1 | `pub trait Transport` + `TransportMessage` compile |
| TRANS-02 | 03-01 / Task 2 | `MemoryTransport` impl + FIFO/UnknownRecipient tests |
| KEY-01 | 03-02 / Task 1 + 03-04 / Task 1 (REQUIREMENTS.md fix) | `Keyring { HashMap<Principal, TrustedVerifyingKey> }` + D-A1 wording update |
| KEY-02 | 03-02 / Task 2 | Round-trip byte-identical on canonical fixture |
| KEY-03 | 03-02 / Task 1+2 | `parse_peer_flag` + PEER-1/2/3 tests |
| EX-01 | 03-04 / Task 1 | `personal_two_agents` example + subprocess test |
| CONF-03 | 03-04 / Task 1 | Happy-path integration test asserts 4-message ordered trace |
| CONF-05 | 03-04 / Task 2 | `conf_05_unsigned_message_rejected` asserts `Decode(MissingSignature)` |
| CONF-06 | 03-04 / Task 2 | `conf_06_wrong_key_signature_rejected` asserts `Decode(SignatureInvalid)` |
| CONF-07 | 03-04 / Task 2 | `conf_07_canonical_divergence_rejected` asserts `CanonicalDivergence` (distinct variant) |

---

## Wave 0 Requirements

- [x] `crates/famp-transport/Cargo.toml` — existing Phase 0 stub (Plan 03-01 Task 1 fills in)
- [ ] `crates/famp-keyring/Cargo.toml` — NEW crate scaffolded in Plan 03-02 Task 1
- [ ] `crates/famp/src/runtime/` — NEW module in Plan 03-03 Task 1
- [ ] `crates/famp/examples/personal_two_agents.rs` — NEW in Plan 03-04 Task 1
- [ ] `crates/famp/tests/runtime_unit.rs` — NEW in Plan 03-03 Task 2
- [ ] `crates/famp/tests/adversarial.rs` — NEW in Plan 03-04 Task 2
- [ ] `crates/famp/tests/example_happy_path.rs` — NEW in Plan 03-04 Task 1
- [ ] `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` — committed fixture in Plan 03-04 Task 2
- [ ] `crates/famp-keyring/tests/fixtures/two_peers.keyring` + `two_peers.canonical.keyring` — Plan 03-02 Task 2

Wave 0 scaffolding is distributed across the first task of each plan (rather than a separate pre-wave) because each plan's Task 1 is a scaffolding step and Task 2 is the behavior/test step — the test scaffold is created alongside its behavior in the same plan.

---

## Manual-Only Verifications

*All phase behaviors have automated verification — example binary self-checks ordering and exit code; adversarial tests assert distinct typed variants; round-trip fixture compared byte-wise.*

None.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: every task has automated verify (no 3-consecutive-task gap)
- [x] Wave 0 covered per-plan (scaffold in Task 1, tests in Task 2)
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** ready for execution
