---
phase: 03-memorytransport-tofu-keyring-same-process-example
plan: 04
subsystem: famp-runtime-example
tags: [example, adversarial, conf-matrix, integration, phase-finish-line]
requires:
  - 03-01-SUMMARY.md  # MemoryTransport + send_raw_for_test
  - 03-02-SUMMARY.md  # famp-keyring + TrustedVerifyingKey binding
  - 03-03-SUMMARY.md  # runtime/process_one_message + canonical pre-check
provides:
  - crates/famp/examples/personal_two_agents.rs
  - crates/famp/tests/example_happy_path.rs
  - crates/famp/tests/adversarial.rs
  - crates/famp/tests/fixtures/conf-07-canonical-divergence.json
affects:
  - Cargo.toml
  - crates/famp/Cargo.toml
  - crates/famp/src/lib.rs
  - crates/famp/src/runtime/adapter.rs
  - crates/famp/tests/runtime_unit.rs
  - .planning/REQUIREMENTS.md
tech-stack:
  added:
    - rand = "0.8" (workspace dep, std_rng feature) — rand_core 0.6 compatible with ed25519-dalek 2.2
  patterns:
    - ed25519-dalek rand_core feature enables OsRng::generate()-driven keypair creation in example binaries
    - Subprocess integration test invokes `cargo run --example` via CARGO env var to gate example regressions in CI
    - Pre-generated + regenerate-if-missing fixture pattern for deterministic adversarial bytes
key-files:
  created:
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/tests/example_happy_path.rs
    - crates/famp/tests/adversarial.rs
    - crates/famp/tests/fixtures/conf-07-canonical-divergence.json
  modified:
    - Cargo.toml (workspace rand dep + ed25519-dalek rand_core feature)
    - crates/famp/Cargo.toml (direct ed25519-dalek + rand deps)
    - crates/famp/src/lib.rs (silence unused_crate_dependencies for example/test-only deps)
    - crates/famp/src/runtime/adapter.rs (Rule-1 fix: Request class now returns None for FSM input)
    - crates/famp/tests/runtime_unit.rs (silence unused_crate_dependencies for ed25519_dalek + rand)
    - .planning/REQUIREMENTS.md (KEY-01 wording update per D-A1)
decisions:
  - Rule-1 deviation on fsm_input_from_envelope — Request class returns None
  - Generate CONF-07 fixture deterministically from seed [1u8; 32], pretty-print for byte divergence
  - Subprocess happy-path test over in-process-spawn to catch cargo/link regressions that unit tests miss
metrics:
  duration: ~45m
  completed: 2026-04-13
  tasks: 2
  tests_added: 4  # 1 happy-path subprocess + 3 CONF adversarial
  tests_passing_workspace: 226
requirements_closed: [EX-01, CONF-05, CONF-06, CONF-07]
---

# Phase 03 Plan 04: Personal Two Agents Example + CONF Adversarial Matrix Summary

Ship EX-01 (single-binary happy path) and the three CONF-0x adversarial integration tests that gate the full runtime pipeline end-to-end over `MemoryTransport`. This plan is the phase's finish line — all four TRANS/KEY/CONF requirements that remained unsatisfied after Plans 01-03 close here.

## What Shipped

**`crates/famp/examples/personal_two_agents.rs`** — Single-binary happy-path driver. Two tokio tasks (`agent:local/alice`, `agent:local/bob`) exchange a full `request → commit → deliver → ack` cycle over an in-process `MemoryTransport`, with pre-pinned per-agent keyrings (not TOFU — Personal Profile v0.7 bootstraps via the `with_peer` builder). Every message is RFC-8785 canonicalized, Ed25519-signed, and verified through `process_one_message`. Prints an ordered typed 4-line trace and exits 0.

**`crates/famp/tests/example_happy_path.rs`** — Subprocess integration test. Invokes the example via `Command::new(env!("CARGO")).args(["run", "--quiet", "-p", "famp", "--example", "personal_two_agents"])` and asserts exit-code 0 plus expected trace lines. Catches cargo/link regressions that unit tests cannot.

**`crates/famp/tests/adversarial.rs`** — Three CONF adversarial integration tests using `MemoryTransport::send_raw_for_test` (gated behind the `test-util` feature, enabled only in `[dev-dependencies]` of the `famp` umbrella crate). Each test asserts a DISTINCT `RuntimeError` variant — the load-bearing D-D8 guarantee that CONF-06 and CONF-07 never collapse into the same error:

| CONF case | Injection                               | Runtime error                                       |
| --------- | ---------------------------------------- | --------------------------------------------------- |
| CONF-05   | Unsigned envelope (signature stripped)   | `Decode(EnvelopeDecodeError::MissingSignature)`     |
| CONF-06   | Wrong-key signature                      | `Decode(EnvelopeDecodeError::SignatureInvalid)`     |
| CONF-07   | Canonical divergence (pretty-printed)    | `CanonicalDivergence` (pre-decode check)            |

**`crates/famp/tests/fixtures/conf-07-canonical-divergence.json`** — Pre-generated CONF-07 fixture. Deterministic construction from seed `[1u8; 32]` + a valid signed request envelope, re-serialized with `serde_json::to_vec_pretty` so the wire bytes contain whitespace that the canonical form does not. The signature remains valid (verifier canonicalizes internally), so the test proves the pre-decode canonical re-check fires **before** signature verification runs.

**Compile-time feature gate** — Private `_require_test_util` probe in `adversarial.rs` ensures `famp-transport`'s `test-util` feature stays wired into `[dev-dependencies]`. A future drive-by that removes the feature would fail to compile rather than silently skip the adversarial matrix.

## Happy-path trace (verified)

```
[1] agent:local/alice -> agent:local/bob: Request
[2] agent:local/bob -> agent:local/alice: Commit
[3] agent:local/bob -> agent:local/alice: Deliver
[4] agent:local/alice -> agent:local/bob: Ack
OK: personal_two_agents complete
```

Alice's FSM: `Requested → Committed` (via Commit) `→ Completed` (via Deliver+TerminalStatus::Completed). Bob's FSM: `Requested` (initial; Request is wire-only per Rule-1 fix below) — no further transitions since Ack is wire-only per D-D4.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `fsm_input_from_envelope` Request class routing**

- **Found during:** Task 1, first attempt to run the example. Bob receives the request via `process_one_message`, which called `task_fsm.step((Request, None))` against a fresh `TaskFsm::new()` (initial state `Requested`). The FSM engine has no `(Requested, Request, None)` arm and correctly returned `IllegalTransition`, failing the example.
- **Root cause:** The adapter's original contract documented that only `Ack` is wire-only, but the 5-state FSM design has no pre-`Requested` state. The initial `Requested` state IS the creation event; there is no legitimate FSM transition that consumes a `Request` message class. The adapter was therefore producing an input that the engine was guaranteed to reject.
- **Fix:** `crates/famp/src/runtime/adapter.rs` — `fsm_input_from_envelope` now returns `None` for `AnySignedEnvelope::Request(_)` in addition to `AnySignedEnvelope::Ack(_)`. The match collapses both variants into one `None` arm with an updated doc comment explaining the rationale. `Commit`, `Deliver`, and `Control` continue to drive transitions.
- **Impact:** `process_one_message` still verifies signature, canonical form, sender, and recipient for request envelopes — only the FSM step is skipped. The existing `runtime_unit::unit4_fsm_input_from_envelope_ack_is_none` test still passes (Ack path unchanged), and no existing test exercised the Request path, so this is a compatibility-preserving correction.
- **Files modified:** `crates/famp/src/runtime/adapter.rs`
- **Commit:** `00d3e3d`

**2. [Rule 3 - Blocking] `unused_crate_dependencies` warnings from new deps**

- **Found during:** Task 1 clippy gate.
- **Issue:** `ed25519-dalek` and `rand` added to `crates/famp/Cargo.toml` as direct deps for the example binary — but the library compile unit (`famp/src/lib.rs`) does not reference them, triggering the workspace's `unused_crate_dependencies = "warn"` lint, which becomes deny under `-D warnings`. Similarly `runtime_unit.rs` tripped the same lint for the test compile unit.
- **Fix:** Added `use ed25519_dalek as _;` / `use rand as _;` to `famp/src/lib.rs` and `famp/tests/runtime_unit.rs` (pattern established earlier in phase 01/02 for tokio, thiserror, etc.).
- **Files modified:** `crates/famp/src/lib.rs`, `crates/famp/tests/runtime_unit.rs`
- **Commit:** `00d3e3d`

**3. [Rule 3 - Blocking] clippy pedantic hits in the example + adversarial test**

- **Issue:** `too_many_lines` (example main > 100 lines), `similar_names` (`bob_vk` vs `bob_sk`, `alice_vk` vs `alice_sk`), and `doc_markdown` (`ALICE_SECRET` in a doc-comment without backticks).
- **Fix:** Targeted module-level `#![allow]` attributes on the example binary and `adversarial.rs` test — both are leaf test/example compile units where the pedantic cost outweighs the clippy benefit.
- **Commits:** `00d3e3d`, `2fa43c3`

All deviations above were `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace` clean after fix.

## Verification

- `cargo run -p famp --example personal_two_agents` — exit 0, prints 4-line ordered trace, prints `OK: personal_two_agents complete`
- `cargo nextest run -p famp --test example_happy_path` — 1/1 passed
- `cargo nextest run -p famp --test adversarial` — 3/3 passed
- `cargo nextest run --workspace` — 226/226 passed
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo check -p famp --all-targets` — clean

## Requirements Closed

| Requirement | Evidence                                                                                                                                         |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| EX-01       | `crates/famp/examples/personal_two_agents.rs` + `example_happy_path.rs` — subprocess test asserts exit 0 + trace order                           |
| CONF-05     | `conf_05_unsigned_message_rejected` — asserts `Decode(MissingSignature)` distinct variant                                                        |
| CONF-06     | `conf_06_wrong_key_signature_rejected` — asserts `Decode(SignatureInvalid)` distinct variant                                                     |
| CONF-07     | `conf_07_canonical_divergence_rejected` — asserts `CanonicalDivergence` distinct variant (fires before signature verification runs, distinct from CONF-06) |

KEY-01 wording reconciled with D-A1 (`TrustedVerifyingKey` + pinned Ed25519 public key + binding-is-the-keyring-not-type-equality) in the same commit as Task 1.

## Known Stubs

None. The example fully exercises the runtime pipeline; no hardcoded empty values, no placeholder bodies, no missing data wiring.

## Self-Check: PASSED

- `crates/famp/examples/personal_two_agents.rs` — FOUND
- `crates/famp/tests/example_happy_path.rs` — FOUND
- `crates/famp/tests/adversarial.rs` — FOUND
- `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` — FOUND
- Commit `00d3e3d` (Task 1) — FOUND
- Commit `2fa43c3` (Task 2) — FOUND
- REQUIREMENTS.md KEY-01 contains `TrustedVerifyingKey` and `pinned Ed25519 public key` — verified
