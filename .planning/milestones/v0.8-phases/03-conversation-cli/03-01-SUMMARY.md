---
phase: 03-conversation-cli
plan: 01
subsystem: storage-primitives
tags: [famp-taskdir, famp-inbox, cursor, peer-entry, requirements-fix]

requires:
  - phase: 01-identity-cli-foundation
    provides: Phase 1 config.rs (Peers placeholder) + IdentityLayout
  - phase: 02-daemon-inbox
    provides: famp-inbox Inbox append/read (cursor added alongside in this plan)

provides:
  - famp-taskdir crate (TaskDir, TaskRecord, TaskDirError)
  - atomic write_atomic_file helper (tempfile + fsync + rename + chmod 0600)
  - famp_inbox::InboxCursor (sidecar byte-offset tracker)
  - InboxError::CursorParse variant
  - Extended PeerEntry { alias, endpoint, pubkey_b64, tls_fingerprint_sha256 }
  - Peers::{find, find_mut, try_add} helpers
  - Requirements traceability fix (INBOX-02/03/05 Phase 2 → Phase 3)

affects: [03-02-send, 03-03-await-inbox, 03-04-peer-add-lock, REQUIREMENTS.md]

tech-stack:
  added:
    - "toml 1.1 in famp-taskdir (matches existing famp workspace dep)"
    - "tempfile 3 promoted from dev-dep to runtime-dep in famp-inbox (cursor atomic write)"
  patterns:
    - "Same-directory NamedTempFile + sync_all + persist + chmod 0600 — mirrored in famp-taskdir/src/atomic.rs and famp-inbox/src/cursor.rs (deliberate duplication, ~30 lines, documented with MIRROR comments)"
    - "UUID validation at the public API boundary (TaskDir::path_for) before touching filesystem — defense against path injection via task_id"
    - "list() skip-unparseable-with-eprintln rather than hard-fail — one corrupted task file must not poison the iterator"
    - "tokio::task::spawn_blocking wrapper around sync tempfile/persist from an async API"

key-files:
  created:
    - crates/famp-taskdir/Cargo.toml
    - crates/famp-taskdir/src/lib.rs
    - crates/famp-taskdir/src/error.rs
    - crates/famp-taskdir/src/record.rs
    - crates/famp-taskdir/src/atomic.rs
    - crates/famp-taskdir/src/store.rs
    - crates/famp-taskdir/tests/roundtrip.rs
    - crates/famp-inbox/src/cursor.rs
    - crates/famp-inbox/tests/cursor_roundtrip.rs
    - .planning/milestones/v0.8-phases/03-conversation-cli/03-01-SUMMARY.md
  modified:
    - Cargo.toml
    - crates/famp-inbox/Cargo.toml
    - crates/famp-inbox/src/lib.rs
    - crates/famp-inbox/src/error.rs
    - crates/famp/src/cli/config.rs
    - .planning/REQUIREMENTS.md
    - .planning/milestones/v0.8-phases/02-daemon-inbox/02-01-PLAN.md
    - .planning/milestones/v0.8-phases/02-daemon-inbox/02-02-PLAN.md
    - .planning/milestones/v0.8-phases/02-daemon-inbox/02-02-SUMMARY.md

key-decisions:
  - "TaskRecord.state is a plain String (not famp_fsm::TaskState) per CONTEXT D-Cursor — keeps on-disk format stable across FSM refactors and avoids adding famp-fsm as a famp-taskdir dep"
  - "write_atomic_file is duplicated across famp-taskdir and famp-inbox rather than extracted to a shared famp-atomic crate — two ~30-line copies cost less than a new crate boundary and keep the two independent"
  - "InboxCursor lives inside famp-inbox (not a sibling crate) per CONTEXT D-Cursor — cursor offset is only meaningful against the companion jsonl file, coupling is inherent"
  - "Requirements labeling fix uses 02-VERIFICATION.md Option A (documentation-only): no code churn because Phase 2's ROADMAP success criteria (5/5) are all green. INBOX-02/03/05 move forward to Phase 3 where their implementations land alongside famp await"
  - "read() returns 0 on ErrorKind::NotFound — first-run case returns a numeric zero rather than a typed 'cursor-missing' error, matching the Unix convention for read-before-write sidecars"

requirements-completed: [INBOX-02, CLI-06]

duration: ~25min
completed: 2026-04-14
---

# Phase 3 Plan 01: Storage + Labeling Foundation Summary

**Ship the per-task TOML store, the inbox byte-cursor, the real PeerEntry schema, and fix the Phase-2 requirement mis-labeling — in one plan — so plans 03-02/03-03/03-04 consume stable primitives without touching the same files.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 3/3 (executed sequentially, one commit per task)
- **Files created:** 10
- **Files modified:** 9
- **Workspace tests:** 316 pass, 1 skipped (up from 298 — +18 new tests)

## Accomplishments

- **Task 1 — Requirement labeling fixed**: REQUIREMENTS.md traceability rows for INBOX-02/03/05 re-mapped from Phase 2 to Phase 3. Plan 02-01 frontmatter no longer claims INBOX-02 or INBOX-05 (actual delivery was INBOX-01 + INBOX-04). Plan 02-02 frontmatter no longer claims INBOX-03 and carries an explicit DAEMON-03 ↔ DAEMON-04 → test mapping comment. SUMMARY 02-02 gained a `## Re-Verification Note (2026-04-14)` section.
- **Task 2 — famp-taskdir crate**: New workspace member under `crates/famp-taskdir/`. Public surface `TaskDir::{open, read, create, update, list}` + `TaskRecord::new_requested` + `TaskDirError` enum. Atomic writes via same-directory `NamedTempFile` + `sync_all` + `persist` + Unix chmod 0600. UUID validation happens at `path_for()` before any filesystem touch (InvalidUuid variant rejects e.g. `"../etc/passwd"`). `list()` logs and skips unparseable files via eprintln.
- **Task 3 — InboxCursor + PeerEntry schema**: `famp_inbox::InboxCursor::{at, read, advance}` added alongside `Inbox`. Wire format is a single ASCII decimal line followed by `\n`; atomic 0600 replace; first-run (missing file) returns 0; malformed content returns `InboxError::CursorParse`. `PeerEntry` promoted from the Phase 1 zero-field placeholder to `{ alias, endpoint, pubkey_b64, tls_fingerprint_sha256: Option<String> }`; `Peers::{find, find_mut, try_add}` helpers added; `deny_unknown_fields` at both struct and entry level; zero-byte `peers.toml` backward-compat preserved (Phase 1 `peers_empty_file_loads_empty` still green).
- **Verification**: `cargo nextest run --workspace` → 316/316 passed, 1 skipped. `cargo clippy --workspace --all-targets -- -D warnings` → 0 warnings. `cargo tree -i openssl` → empty.

## Task Commits

1. **docs(03-01): fix cross-phase requirement labeling** — `2ba3f62`
2. **feat(03-01): add famp-taskdir crate with atomic TOML task records** — `19ef973`
3. **feat(03-01): add InboxCursor + extend PeerEntry schema** — `40a9f28`

## Test Coverage Added

### famp-taskdir (9 new tests)
- `create_then_read_returns_same_record`
- `create_rejects_duplicate` — AlreadyExists on second create
- `create_rejects_invalid_uuid` — InvalidUuid on `"not-a-uuid"`
- `read_missing_returns_not_found` — NotFound on absent task_id
- `update_mutates_in_place` — closure-based mutation round-trip
- `list_returns_all_records` — 3 records created, 3 listed
- `list_skips_unparseable` — garbage.toml in the dir is logged and skipped
- `update_round_trip_byte_stable` — read → write-back unchanged → byte-identical file
- `open_is_idempotent` — double open() is fine

### famp-inbox (5 new cursor tests)
- `read_returns_zero_when_missing`
- `advance_then_read_roundtrip` — 42 → 99
- `advance_creates_0600_file` — Unix mode check
- `read_garbage_returns_cursor_parse_error` — CursorParse variant
- `advance_is_atomic_across_concurrent_writers` — 8 concurrent tasks, final offset is one of them (no tearing)

### famp (4 new peer tests in cli::config::tests)
- `peers_roundtrip_single_entry`
- `peers_try_add_rejects_duplicate_alias`
- `peers_rejects_unknown_fields_on_entry`
- `peers_find_returns_none_for_unknown_alias`

## Decisions Made

- **No famp-atomic shared crate** — `write_atomic_file` is mirrored in `famp-taskdir/src/atomic.rs` and inlined in `famp-inbox/src/cursor.rs`. Both are ~30 lines. The shared-crate boundary would cost more than the duplication. Mirrored files carry `MIRROR:` comments so future edits stay in sync.
- **TaskRecord.state as String** — the file format must survive future FSM refactors. FSM-to-string mapping lives in the consumer (`crates/famp/src/cli/`, plan 03-02), not in `famp-taskdir`. This keeps `famp-taskdir` zero-dep on runtime crates.
- **Cursor read() returns 0 on NotFound** — first-run convention. Callers don't need to special-case missing-cursor vs offset-zero.
- **Requirements fix is documentation-only** — Phase 2's ROADMAP success criteria (5/5) are all green per `02-VERIFICATION.md`. The mis-labeling was about `requirements_addressed` frontmatter claims vs REQUIREMENTS.md wording, not about missing functionality. Option A (re-map labels) costs nothing; Option B (retrofit cursor/lock/await-poll into Phase 2) would have duplicated Phase 3 work.

## Deviations from Plan

### Rule 1 — Bug: TaskDir::read original UTF-8 error path was unreachable

- **Found during:** First clippy run on famp-taskdir.
- **Issue:** The initial `store::read` attempted to construct a `TaskDirError::TomlParse` from a synthesized toml error via `unwrap()` on a dummy parse call. This tripped `clippy::unwrap_used` (project deny level).
- **Fix:** Non-UTF-8 bodies now surface as `TaskDirError::Io { source: io::Error::new(InvalidData, …) }` — cleaner and avoids the dummy parse.
- **Files modified:** `crates/famp-taskdir/src/store.rs`.
- **Commit:** squashed into `19ef973`.

### Rule 3 — Blocking: clippy doc-markdown + redundant-clone fixes

- **Found during:** Initial clippy runs on famp-taskdir and famp-inbox.
- **Issues fixed inline before commit:**
  1. `clippy::doc_markdown` on `UUIDv7` in `TaskRecord.task_id` doc — wrapped in backticks.
  2. `clippy::doc_markdown` on `write_atomic_file` in `cursor.rs` module comment — wrapped in backticks.
  3. `clippy::redundant_clone` on `path.clone()` in `store::read` Io-error arm (path is dropped after the branch) — removed.
  4. `unused_crate_dependencies` on famp-taskdir test crate (serde/thiserror/toml/uuid pulled transitively but not referenced in tests) — added `use ... as _;` silencers.
  5. Same silencers for `serde_json as _; thiserror as _;` in famp-inbox cursor test.
- **Impact:** None on plan scope. All inline before the Task 2 / Task 3 commits.

### Note: `state_from_fsm` free function deferred

Plan action Task 2 step 5 suggested adding `state_from_fsm(fsm::TaskState) -> &'static str` to `record.rs`, but also explicitly forbade adding `famp-fsm` as a dependency. The two constraints are incompatible, and the plan itself flagged that the mapping function should live in the consumer (`crates/famp/src/cli/`, plan 03-02). Left out of this plan accordingly — the plan text already says "Just leave the `state` field as a `String`."

---

**Total deviations:** 1 Rule-1 bug (unreachable unwrap path), 1 Rule-3 lint batch (5 items). No Rule-4 architectural changes.

## Verification Artifacts

- `cargo nextest run --workspace` → **316 passed, 1 skipped** (was 298, +18 new)
- `cargo clippy --workspace --all-targets -- -D warnings` → **0 warnings**
- `cargo tree -i openssl` → `did not match any packages` (openssl gate holds)
- `grep "INBOX-02 | Phase 3" .planning/REQUIREMENTS.md` → hit
- `grep "INBOX-03 | Phase 3" .planning/REQUIREMENTS.md` → hit
- `grep "INBOX-05 | Phase 3" .planning/REQUIREMENTS.md` → hit
- `grep "INBOX-03" .planning/milestones/v0.8-phases/02-daemon-inbox/02-02-PLAN.md` → no matches
- `grep "INBOX-0[25]" .planning/milestones/v0.8-phases/02-daemon-inbox/02-01-PLAN.md` → no matches

## Threat Flags

None. No new network endpoints, auth paths, or trust boundaries introduced. The `tls_fingerprint_sha256` field is an Option consumed by future Phase 3 TOFU logic (plan 03-02/03); it is a storage slot only in this plan. UUID validation at the `path_for()` boundary is a Rule-2 mitigation for path injection (already in plan).

## Next Plan Readiness

- **Plan 03-02 (famp send)** can now consume `famp_taskdir::TaskDir::create` for new-task records, `Peers::find` for peer lookup, and the `tls_fingerprint_sha256` slot for TOFU pinning.
- **Plan 03-03 (famp await + famp inbox)** can consume `famp_inbox::InboxCursor::{read, advance}` for cursor semantics.
- **Plan 03-04 (peer add + lock + E2E)** can consume `Peers::try_add` for duplicate rejection and extend the cursor with the INBOX-05 advisory lock.

## Self-Check: PASSED

- `crates/famp-taskdir/src/lib.rs` — FOUND
- `crates/famp-taskdir/src/store.rs` — FOUND
- `crates/famp-taskdir/src/record.rs` — FOUND
- `crates/famp-taskdir/src/error.rs` — FOUND
- `crates/famp-taskdir/src/atomic.rs` — FOUND
- `crates/famp-taskdir/tests/roundtrip.rs` — FOUND
- `crates/famp-inbox/src/cursor.rs` — FOUND
- `crates/famp-inbox/tests/cursor_roundtrip.rs` — FOUND
- Commit `2ba3f62` — FOUND in git log
- Commit `19ef973` — FOUND in git log
- Commit `40a9f28` — FOUND in git log
- `grep -l "INBOX-02 | Phase 3" .planning/REQUIREMENTS.md` — FOUND

---
*Phase: 03-conversation-cli*
*Plan: 01*
*Completed: 2026-04-14*
