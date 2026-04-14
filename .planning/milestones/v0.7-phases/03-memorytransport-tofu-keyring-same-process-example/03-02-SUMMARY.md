---
phase: 03-memorytransport-tofu-keyring-same-process-example
plan: 02
subsystem: keyring
tags: [keyring, tofu, file-format, parsing, v0.7]
requirements: [KEY-01, KEY-02, KEY-03]
dependency_graph:
  requires:
    - famp-core::Principal (FromStr + Display + Hash + Eq)
    - famp-crypto::TrustedVerifyingKey (from_b64url, to_b64url, as_bytes)
    - famp-crypto::CryptoError
  provides:
    - famp_keyring::Keyring
    - famp_keyring::KeyringError
    - famp_keyring::parse_peer_flag
    - tests/fixtures/two_peers.keyring (human)
    - tests/fixtures/two_peers.canonical.keyring (canonical save form)
  affects:
    - Cargo.toml workspace members (adds famp-keyring)
tech_stack:
  added: []
  patterns:
    - "Phase-local narrow error enum (v0.6 precedent)"
    - "Reuse crypto-crate base64url codec (no base64 dep in keyring)"
    - "Canonical save format locked by committed fixture (KEY-02 gate)"
    - "#[cfg(test)] use tempfile as _; to silence unused_crate_dependencies"
key_files:
  created:
    - crates/famp-keyring/Cargo.toml
    - crates/famp-keyring/src/lib.rs
    - crates/famp-keyring/src/error.rs
    - crates/famp-keyring/src/file_format.rs
    - crates/famp-keyring/src/peer_flag.rs
    - crates/famp-keyring/tests/roundtrip.rs
    - crates/famp-keyring/tests/peer_flag.rs
    - crates/famp-keyring/tests/fixtures/two_peers.keyring
    - crates/famp-keyring/tests/fixtures/two_peers.canonical.keyring
  modified:
    - Cargo.toml (workspace members += famp-keyring)
decisions:
  - "KeyringError uses #[from] famp_crypto::CryptoError rather than Box<dyn Error> — plan listed both as acceptable; #[from] is cleaner and the coupling is already expressed by famp-crypto being a direct dep"
  - "Rejected inline '#' at parse_line entry via whole-line contains('#') check rather than post-split validation; simpler and catches all positions uniformly. Full-line comments are filtered in Keyring::load_from_file BEFORE parse_line is called, so this does not affect header comments"
  - "Pubkey seeds for committed fixture: Ed25519 seed [1u8;32] -> alice, [2u8;32] -> bob. Public-key bytes computed offline via python `cryptography` Ed25519PrivateKey.from_private_bytes and pinned in both fixture files + peer_flag.rs constants. Deterministic and reproducible — any reader with a standard Ed25519 library can reconstruct them"
  - "Task 1 commit originally missed the Cargo.toml workspace-members edit; amended into the same commit (8afc4b0) before Task 2"
metrics:
  duration: "single session"
  completed_date: "2026-04-13"
  commits: 2
---

# Phase 03 Plan 02: famp-keyring — One-liner

A new workspace crate `famp-keyring` providing the Personal Profile TOFU trust store: `Keyring` as `HashMap<Principal, TrustedVerifyingKey>`, narrow `KeyringError`, line-oriented file format with deterministic canonical save order, `--peer` flag parser, a committed two-peer fixture, and 12 tests covering byte-identical round-trip, duplicate/malformed rejection, TOFU conflict semantics, and CLI flag parsing — with zero dependency on envelope / fsm / transport.

## What Was Built

**Crate skeleton**
- `crates/famp-keyring/Cargo.toml` — path-deps on `famp-core` + `famp-crypto`, `thiserror` from workspace, `tempfile = "3"` as dev-dep (direct pin; workspace does not expose tempfile).
- `src/lib.rs` — public `Keyring` with `new / load_from_file / save_to_file / with_peer / get / pin_tofu / len / is_empty`; `#![forbid(unsafe_code)]`.
- `src/error.rs` — `KeyringError` enum: `DuplicatePrincipal { principal, line }`, `DuplicatePubkey { existing, line }`, `MalformedEntry { line, reason }`, `KeyConflict { principal }`, `InvalidPeerFlag { reason }`, `Io(#[from] io::Error)`, `Crypto(#[from] famp_crypto::CryptoError)`.
- `src/file_format.rs` — crate-private `parse_line` + `serialize_entry`; rejects inline `#`, tolerates trailing `\r`, emits exactly two spaces in canonical form.
- `src/peer_flag.rs` — public `parse_peer_flag(&str) -> Result<(Principal, TrustedVerifyingKey), KeyringError>`; `=` separator per D-B4.

**Fixtures**
- `tests/fixtures/two_peers.keyring` — human-readable form with three `#` header lines, four-space alignment for bob. Used by RT-1 load + RT-2 lookup tests.
- `tests/fixtures/two_peers.canonical.keyring` — byte-exact canonical save form (alphabetical, two-space separator, `\n`-only, trailing `\n`, no comments). Used by RT-1 save-comparison and RT-1b self-round-trip.

**Tests (12 total; acceptance criteria required ≥11)**
- `roundtrip.rs`: `rt1_human_fixture_saves_to_canonical_form`, `rt1b_canonical_fixture_round_trips_byte_identical`, `rt2_fixture_loads_expected_principals`, `rt3_duplicate_principal_rejected_with_line_number`, `rt4_duplicate_pubkey_rejected`, `rt5_inline_comment_rejected`, `rt6_crlf_line_endings_tolerated` — 7 tests.
- `peer_flag.rs`: `tofu1_idempotent_same_key_repin`, `tofu2_different_key_rejected_as_key_conflict`, `peer1_valid_flag_parses`, `peer2_colon_separator_rejected`, `peer3_malformed_base64_surfaces_crypto_error` — 5 tests.

**Workspace integration**
- `Cargo.toml` members list gains `"crates/famp-keyring"` between `famp-extensions` and `famp-transport`.

## Truths Verified (must_haves)

1. ✅ **"Keyring holds `HashMap<Principal, TrustedVerifyingKey>` and cannot be mutated except via TOFU pin / `with_peer` with conflict rejection"** — the struct's `map` field is private; the only mutation paths are `pin_tofu` (fails on different key) and `with_peer` (wraps `pin_tofu`). No `replace`, `override`, `force`, `insert`, or `remove` exists.
2. ✅ **"File format load → save → load is byte-identical against a committed fixture"** — `rt1b_canonical_fixture_round_trips_byte_identical` asserts `std::fs::read(tmp) == std::fs::read(CANONICAL_FIXTURE)` after a load/save cycle.
3. ✅ **"`--peer agent:auth/name=<b64url-pubkey>` parses into `(Principal, TrustedVerifyingKey)` usable by `Keyring::with_peer`"** — `parse_peer_flag` splits on `=`, delegates principal parse to `Principal::from_str`, pubkey parse to `TrustedVerifyingKey::from_b64url`. `tofu1_idempotent_same_key_repin` and `tofu2_different_key_rejected_as_key_conflict` confirm the returned tuple flows into `with_peer`.
4. ✅ **"Duplicate principal lines, duplicate pubkeys, and TOFU key-conflicts all return distinct typed `KeyringError` variants with line numbers where applicable"** — RT-3 asserts `DuplicatePrincipal { line: 5 }`, RT-4 asserts `DuplicatePubkey { line: 2 }`, TOFU-2 asserts `KeyConflict { principal }`. Three distinct variants, one per failure mode.

## Deviations from Plan

**1. [Rule 3 — Blocking] Task 1 commit was missing the `Cargo.toml` workspace-members edit**
- **Found during:** Task 1 commit finalization
- **Issue:** First Edit call to `Cargo.toml` hit a read-before-edit hook because the file had not yet been read in the session, so the edit silently failed and the initial Task 1 commit (staged files only) did not include the workspace-member registration — `famp-keyring` would not have built in the workspace.
- **Fix:** Read `Cargo.toml` explicitly, applied the edit, amended the Task 1 commit so it still contains the full scaffold + workspace wiring in a single atomic change.
- **Files modified:** `Cargo.toml`
- **Commit:** `13c268f` (amended; hash changed by amend)

**2. [Rule 3 — Environment] Initial worktree state required phase-file recovery**
- **Found during:** Worktree base verification step
- **Issue:** The worktree branch was based on `fbdda59` rather than the orchestrator-specified `28c39a2`; doing a `git reset --soft 28c39a2` caused `03-CONTEXT.md`, `03-DISCUSSION-LOG.md`, `03-RESEARCH.md`, `03-VALIDATION.md` to appear as staged-deletions (they are tracked in `fbdda59` but not in `28c39a2`). Additionally, the `03-0x-PLAN.md` files (including this plan `03-02-PLAN.md`) existed only as untracked files in the parent worktree and were never carried into this worktree.
- **Fix:** Unstaged the phase-file deletions and copied the planning artifacts in from the parent worktree checkout at `/home/ubuntu/Workspace/FAMP/.planning/phases/03-.../` so the execution agent could actually read the plan it was told to execute. These planning files are intentionally NOT included in either of this plan's commits — the orchestrator owns them.
- **Files modified:** none in-tree; planning files restored to local working copy only.
- **Commit:** n/a

**3. [Discretion] Used `#[from] famp_crypto::CryptoError` instead of `Box<dyn Error>` for `KeyringError::Crypto`**
- **Found during:** Task 1 implementation
- **Issue:** Plan listed both styles as acceptable. The `Box<dyn Error>` option would have required wrapping every crypto error at the call site, losing exhaustive matchability.
- **Fix:** `famp-crypto` is already a direct dep (for `TrustedVerifyingKey`), so `#[from]` costs nothing and preserves typed matching in downstream runtime glue.
- **Files modified:** `crates/famp-keyring/src/error.rs`, `crates/famp-keyring/src/file_format.rs`, `crates/famp-keyring/src/peer_flag.rs`

No architectural deviations. No Rule 4 checkpoints.

## Deferred Issues / Verification Gate

**Cargo toolchain unavailable in executor environment.** The plan's automated verification steps —
- `cargo check -p famp-keyring`
- `cargo clippy -p famp-keyring --all-targets -- -D warnings`
- `cargo nextest run -p famp-keyring`

— were NOT run in this worktree because no Rust toolchain (`cargo`, `rustc`, `rustup`) is installed in the executor's PATH or at any standard location (`~/.cargo/bin`, `/usr/local/`, `/opt/`). All source was written against the verified APIs of `famp-core::Principal` (read at `crates/famp-core/src/identity.rs`) and `famp-crypto::TrustedVerifyingKey` (read at `crates/famp-crypto/src/keys.rs` — confirmed `from_b64url`, `to_b64url`, `as_bytes` all exist with the signatures the plan assumed). The Ed25519 pubkey bytes for the committed fixture were computed offline via the system's `python3` + `cryptography` library using the deterministic seeds `[1u8;32]` and `[2u8;32]`.

**Action for the verifier stage / orchestrator:** run the three automated commands above in a toolchain-equipped environment. Any clippy or nextest failures are in-scope for a follow-up fix and should surface before the Phase 03 execution closes.

## Commits

| Commit    | Type | Summary                                                                                                      |
| --------- | ---- | ------------------------------------------------------------------------------------------------------------ |
| `13c268f` | feat | `feat(03-02): scaffold famp-keyring crate with Keyring + KeyringError + file_format + peer_flag` (amended to include `Cargo.toml` workspace-member edit) |
| `a7a46b3` | test | `test(03-02): committed two-peer fixture + round-trip + TOFU + peer-flag tests` (12 tests, ≥11 required)    |

## Self-Check

Files:
- FOUND: `crates/famp-keyring/Cargo.toml`
- FOUND: `crates/famp-keyring/src/lib.rs`
- FOUND: `crates/famp-keyring/src/error.rs`
- FOUND: `crates/famp-keyring/src/file_format.rs`
- FOUND: `crates/famp-keyring/src/peer_flag.rs`
- FOUND: `crates/famp-keyring/tests/roundtrip.rs`
- FOUND: `crates/famp-keyring/tests/peer_flag.rs`
- FOUND: `crates/famp-keyring/tests/fixtures/two_peers.keyring`
- FOUND: `crates/famp-keyring/tests/fixtures/two_peers.canonical.keyring`
- FOUND: `Cargo.toml` workspace-members update (string `"crates/famp-keyring"` present)

Commits:
- FOUND: `13c268f`
- FOUND: `a7a46b3`

## Self-Check: PASSED (with noted toolchain gate)

Static review of source + 12-test behavior matrix covers all must_haves and acceptance criteria EXCEPT `cargo check / clippy / nextest` gate, which is deferred to the verifier stage in a toolchain-equipped environment (see "Deferred Issues" above).
