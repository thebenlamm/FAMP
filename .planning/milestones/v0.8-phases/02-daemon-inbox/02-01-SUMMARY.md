---
phase: 02-daemon-inbox
plan: 01
subsystem: storage
tags: [tokio, jsonl, fsync, thiserror, inbox]

requires:
  - phase: 01-identity-cli-foundation
    provides: workspace layout, strict clippy lints, narrow error-enum pattern

provides:
  - famp-inbox crate (new workspace member, library-only)
  - Inbox::open creating 0600-mode JSONL file on unix
  - Inbox::append(&[u8]) with fsync-before-return durability receipt
  - read::read_all with tail-tolerance for truncated final line
  - Mid-file corruption surfaces as CorruptLine hard error
  - InboxError narrow 3-variant thiserror enum

affects: [02-02-daemon-listen, 02-03-shutdown, 03-conversation-cli]

tech-stack:
  added:
    - "tokio fs + sync features for async File + Mutex"
    - "tempfile 3 dev-dep for hermetic tests"
  patterns:
    - "Raw-bytes append API (&[u8]) preserves byte-exactness — no typed-decode-then-re-encode"
    - "Single Arc<tokio::sync::Mutex<File>> for concurrent-append serialization"
    - "Sync read_all (read path is cold — shutdown / cold-start / await)"
    - "Tail-tolerance = file-ended-with-newline? bit; partial final chunk swallowed"

key-files:
  created:
    - crates/famp-inbox/Cargo.toml
    - crates/famp-inbox/src/lib.rs
    - crates/famp-inbox/src/error.rs
    - crates/famp-inbox/src/append.rs
    - crates/famp-inbox/src/read.rs
    - crates/famp-inbox/tests/roundtrip.rs
    - crates/famp-inbox/tests/truncated_tail.rs
  modified:
    - Cargo.toml (added crates/famp-inbox to workspace members)

key-decisions:
  - "famp-inbox operates on raw &[u8], NOT typed SignedEnvelope — preserves byte-exactness (P3) and keeps famp-inbox decoupled from famp-envelope"
  - "Unix-only 0600 create step via std::os::unix::fs::OpenOptionsExt::mode, then reopen via tokio::fs::OpenOptions for the async writer handle"
  - "Append wraps its body in an inner async block + explicit drop(guard) to satisfy clippy::significant_drop_tightening while still holding the mutex across write_all + sync_data"
  - "Threat mitigation T-02-02 (file mode) covered by a unit test (open_creates_file_with_mode_0600) — plan originally scoped this into 'verify via integration test' but lifting it to a unit test inside append.rs is simpler and equally binding"

patterns-established:
  - "Narrow per-crate thiserror enum (InboxError: Io, CorruptLine, EmbeddedNewline)"
  - "read_all tail-tolerance rule: file_ends_with_newline? → discard; partial final chunk → swallow parse failure; non-terminal bad line → hard error"
  - "Integration tests write raw bytes via std::fs::write, independent of the append path, to verify the read-path contract in isolation"

requirements-completed: [INBOX-01, INBOX-02, INBOX-04, INBOX-05]

duration: ~20min
completed: 2026-04-14
---

# Phase 2 Plan 1: famp-inbox Summary

**Durable JSONL inbox library crate with fsync-before-return append and tail-tolerant read, delivered as an isolated workspace member ready for Plan 02-02's axum handler to drop in.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-04-14T20:13Z (approx)
- **Completed:** 2026-04-14T20:33Z
- **Tasks:** 2/2
- **Files created:** 7
- **Files modified:** 1

## Accomplishments

- New `famp-inbox` workspace crate compiling under the strict pedantic-deny lint set, zero `unsafe_code`.
- Durability contract locked in source and test: `write_all(bytes)` → `write_all(b"\n")` → `sync_data()` → `Ok(())`. Plan 02-02 can safely return HTTP 200 on observing `Ok`.
- Concurrent-append serialization verified by a 16-task multi-threaded tokio test: every one of 16 distinct 900-byte payloads lands as a parseable JSONL line with no interleaving.
- 0600 file mode verified by test — closes threat T-02-02 (information disclosure to other local users).
- Read-path boundary locked: truncated/garbage tail silently skipped, mid-file corruption returns `InboxError::CorruptLine { line_no: 2 }` with the exact offending index.
- Embedded-newline guard rejects malformed bytes **before** touching the file, so a buggy caller can never split one logical envelope across two JSONL lines.
- `cargo tree -i openssl` still empty (E2E-03 guard holds).
- Workspace nextest: **292/292 green** (253 v0.7 baseline + 31 Phase 1 + 8 new famp-inbox tests).

## Task Commits

1. **Task 1: Scaffold famp-inbox crate, error enum, Inbox::open + append with fsync** — `b7ca9bb` (feat)
   - New workspace member, module wiring, InboxError enum, Inbox::open/append, read::read_all, 4 unit tests (roundtrip, embedded_newline_rejected, concurrent_appends_serialize, open_creates_file_with_mode_0600).
2. **Task 2: Integration tests — truncated tail tolerated, mid-file corruption rejected** — `071b781` (test)
   - `tests/roundtrip.rs`: 1 test (drop-and-reopen roundtrip).
   - `tests/truncated_tail.rs`: 3 tests (missing-newline tail, garbage tail, mid-file hard error).

## Files Created/Modified

- `Cargo.toml` — added `crates/famp-inbox` to `[workspace] members`.
- `crates/famp-inbox/Cargo.toml` — new crate manifest (tokio fs+sync+io-util+rt, serde_json, thiserror; dev: tempfile, tokio macros+rt-multi-thread).
- `crates/famp-inbox/src/lib.rs` — `#![forbid(unsafe_code)]`, module wiring, re-exports `Inbox` + `InboxError`.
- `crates/famp-inbox/src/error.rs` — three-variant `InboxError` enum (`Io { path, source }`, `CorruptLine { line_no, source }`, `EmbeddedNewline`).
- `crates/famp-inbox/src/append.rs` — `Inbox` struct, `open`, `append` (fsync-before-return), `path` accessor, 4 inline unit tests.
- `crates/famp-inbox/src/read.rs` — sync `read_all` with tail-tolerance rule.
- `crates/famp-inbox/tests/roundtrip.rs` — drop-and-reopen integration smoke test.
- `crates/famp-inbox/tests/truncated_tail.rs` — three read-path boundary tests.

## Decisions Made

- **Raw bytes over typed envelope (plan-mandated):** The append API takes `&[u8]`, not `&SignedEnvelope`. This preserves the byte-exact bytes the daemon has already verified on the wire; no typed decode-then-re-encode round-trip can introduce canonicalization drift. famp-inbox therefore has zero dependency on famp-envelope, and Plans 02-02/02-03 stay trivially reusable for any future body schema without churning this crate.
- **`clippy::significant_drop_tightening` accommodation:** The lint fires on `let mut guard = self.file.lock().await;` held across three `.await` points. Rather than `#[allow]`, I wrapped the body in an inner `async { ... }` block, captured the result, and explicitly `drop(guard)` before returning. The mutex is still held across the full write+fsync sequence (which is the correctness requirement), but the lifetime is now explicit to the lint.
- **Mode-0600 test in `append.rs` not `truncated_tail.rs`:** Plan threat model T-02-02 asked for a file-mode assertion in the Task 1 unit suite. I added `open_creates_file_with_mode_0600` gated on `#[cfg(unix)]`, matching the `unix`-only plan surface.

## Deviations from Plan

### Rule 2 — Missing Critical (security): 0600 mode unit test

- **Found during:** Task 1 (while wiring the `#[cfg(unix)]` branch of `Inbox::open`).
- **Issue:** Plan §tasks lists 3 unit tests but the `<threat_model>` row T-02-02 explicitly requires a file-mode test "add to Task 1 unit test." The test count in `<done>` (3/3) conflicts with the threat model's "add to Task 1 unit test" directive.
- **Fix:** Added a 4th unit test, `open_creates_file_with_mode_0600`, verifying `metadata.permissions().mode() & 0o777 == 0o600` after `Inbox::open`. Gated on `#[cfg(unix)]`.
- **Files modified:** `crates/famp-inbox/src/append.rs`.
- **Verification:** Test passes. Final famp-inbox test count is 4 unit + 4 integration = 8 (plan said 7).
- **Committed in:** `b7ca9bb` (Task 1 commit).

### Rule 3 — Blocking (lint): `clippy::significant_drop_tightening` in `Inbox::append`

- **Found during:** Task 1 verify step.
- **Issue:** Workspace lints set `clippy::nursery = warn` plus `-D warnings` in CI — the `significant_drop_tightening` nursery lint rejected holding the mutex guard across three `.await` points even though that is the correctness contract.
- **Fix:** Refactored the body of `append` to a block-expression pattern: `let result = async { … }.await; drop(guard); result`. The guard is explicitly dropped after the inner block instead of at function scope end. Same locking semantics, lint happy.
- **Files modified:** `crates/famp-inbox/src/append.rs`.
- **Verification:** `cargo clippy -p famp-inbox --all-targets -- -D warnings` exits 0.
- **Committed in:** `b7ca9bb` (Task 1 commit, pre-commit fix).

---

**Total deviations:** 2 auto-fixed (1 Rule-2 security, 1 Rule-3 blocking).
**Impact on plan:** Neither changes public API, crate layout, or test intent. The extra mode test raises the bar slightly above plan baseline; the clippy fix is cosmetic-to-the-contract.

## Issues Encountered

None beyond the two deviations documented above. Unit tests and integration tests passed on first run after the clippy refactor.

## Verification Artifacts

- `cargo nextest run -p famp-inbox` → **8/8 passed** (0.08s)
- `cargo clippy -p famp-inbox --all-targets -- -D warnings` → 0 warnings
- `cargo nextest run --workspace` → **292/292 passed, 1 skipped** (baseline intact)
- `cargo tree -i openssl` → empty (E2E-03 guard)
- `grep -n 'sync_data' crates/famp-inbox/src/append.rs` → 3 hits (docstring + module header + call site)
- `grep -n 'mode(0o600)' crates/famp-inbox/src/append.rs` → 2 hits (create call + test assertion)

## Threat Flags

None. No new network surface, auth path, or schema change introduced beyond the inbox file trust boundary already enumerated in the plan's `<threat_model>`.

## Next Phase Readiness

- **Plan 02-02 (`famp listen` daemon)** can import `famp-inbox` and call `inbox.append(bytes).await` from inside the existing `famp-transport-http` dispatch closure. The fsync-before-return contract is the HTTP 200 durability receipt Plan 02-02 needs.
- **Plan 02-03 (graceful shutdown + durability test)** can rely on `read_all` tail-tolerance for the post-SIGKILL re-read step.
- **Phase 3 (`famp inbox`, `famp await`)** can reuse `read_all` directly — it already returns `Vec<serde_json::Value>` which is the right shape for list/filter/print pipelines.

## Self-Check: PASSED

- `crates/famp-inbox/Cargo.toml` — FOUND
- `crates/famp-inbox/src/lib.rs` — FOUND
- `crates/famp-inbox/src/error.rs` — FOUND
- `crates/famp-inbox/src/append.rs` — FOUND
- `crates/famp-inbox/src/read.rs` — FOUND
- `crates/famp-inbox/tests/roundtrip.rs` — FOUND
- `crates/famp-inbox/tests/truncated_tail.rs` — FOUND
- `Cargo.toml` workspace member — FOUND (grep `famp-inbox` → hit)
- Commit `b7ca9bb` — FOUND in `git log`
- Commit `071b781` — FOUND in `git log`

---
*Phase: 02-daemon-inbox*
*Plan: 01*
*Completed: 2026-04-14*
