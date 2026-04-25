---
phase: quick-260425-lny
plan: 01
subsystem: famp/cli/send
tags: [bug-fix, tdd, fsm, try_update, b2-class, sentinel-discriminator]
key-decisions:
  - "Mirror await_cmd post-ho8 pattern verbatim: try_update + match arms, log-and-continue on closure Err"
  - "Sentinel-discriminator (TOML comment) is the discriminating proof, not byte equality or mtime"
key-files:
  created:
    - crates/famp/tests/send_terminal_advance_error_surfaces.rs
  modified:
    - crates/famp/src/cli/send/mod.rs
metrics:
  duration: ~25min
  completed: 2026-04-25
  tasks: 2
  files: 2
---

# Quick Task 260425-lny: Fix B2-class FSM Error Suppression at send/mod.rs:~514

## What and Why

Closed the same B2-class "error swallowing + spurious write" anti-pattern on
the send side that quick-260425-ho8/kbx closed on the await side.

`send/mod.rs`'s `SendMode::DeliverTerminal` arm of `persist_post_send` used
`let _ = fsm_glue::advance_terminal(&mut r)` inside a `tasks.update(...)`
closure. Two symptoms:

1. **Errors swallowed** — if `advance_terminal` returned `Err(IllegalTransition)`
   (e.g., on-disk record in REQUESTED instead of COMMITTED), the error was
   silently discarded.
2. **Spurious write** — `tasks.update` rewrote the file regardless, updating
   `last_send_at` even though the FSM advance failed.

Fix: replaced the whole block with `tasks.try_update(task_id, |mut r| { r.last_send_at = ...; advance_terminal(&mut r).map(|_| r) })` + explicit `match` over `TryUpdateError::Closure(_)` / `TryUpdateError::Store(_)`. On closure `Err`, `try_update` performs NO disk write. The NotFound create-on-demand arm is preserved verbatim. Structural identity to `await_cmd/mod.rs:173-198` confirmed.

## Diff Summary

| File | Change |
|------|--------|
| `crates/famp/src/cli/send/mod.rs` | `+21 / -6` — `TryUpdateError` import added; `SendMode::DeliverTerminal` block rewritten |
| `crates/famp/tests/send_terminal_advance_error_surfaces.rs` | `+251 / 0` — new sentinel-discriminator integration test |

## Commits

| Atom | Hash | Description |
|------|------|-------------|
| RED test | `22eacd3` | `test(quick-260425-lny): add RED sentinel-discriminator for send terminal-advance error surfacing` |
| GREEN fix | `238e397` | `fix(quick-260425-lny): replace tasks.update with try_update in SendMode::DeliverTerminal` |

Two atomic commits as specified. Pre-commit hooks passed on both (hooks do not
run the full test suite per-commit — CI gate handles that).

## Stash-Pop Sanity Check

### RED (fix reverted via `git stash push`)

```
Nextest run ID 4f362065 with nextest profile: default
    Starting 1 test across 1 binary
        FAIL [   0.149s] (1/1) famp::send_terminal_advance_error_surfaces terminal_send_when_record_in_requested_does_not_rewrite_task_file
  stderr:
    thread 'terminal_send_when_record_in_requested_does_not_rewrite_task_file' panicked at crates/famp/tests/send_terminal_advance_error_surfaces.rs:197:5:
    sentinel was clobbered: a spurious write occurred during SendMode::DeliverTerminal persist when advance_terminal returned Err (record was in REQUESTED state). Bytes pre/post:
    ---PRE---
    task_id = "019dc62d-27bd-74f3-ae5a-83677a434110"
    state = "REQUESTED"
    ...
    # TEST_SENTINEL_DO_NOT_REWRITE

    ---POST---
    task_id = "019dc62d-27bd-74f3-ae5a-83677a434110"
    state = "REQUESTED"
    ...
    [sentinel GONE — last_send_at updated, comment dropped by toml::to_string]

     Summary [   0.153s] 1 test run: 0 passed, 1 failed, 0 skipped
```

**Conclusion:** sentinel clobbered by spurious `tasks.update` call. Bug confirmed.

### GREEN (fix restored via `git stash pop`)

```
Nextest run ID 5119f513 with nextest profile: default
    Starting 1 test across 1 binary
        PASS [   0.124s] (1/1) famp::send_terminal_advance_error_surfaces terminal_send_when_record_in_requested_does_not_rewrite_task_file

     Summary [   0.124s] 1 test run: 1 passed, 0 skipped
```

**Conclusion:** sentinel survives — `try_update` correctly skips write on closure `Err`.

## Structural Mirror Confirmed

`send/mod.rs` DeliverTerminal block (post-lny) vs `await_cmd/mod.rs` commit-receipt block (post-ho8):

```rust
// send/mod.rs — DeliverTerminal arm
match tasks.try_update(task_id, |mut r| {
    r.last_send_at = Some(now_s.clone());       // extra mutation (send-specific)
    fsm_glue::advance_terminal(&mut r).map(|_| r)
}) {
    Ok(_) => {}
    Err(TryUpdateError::Store(famp_taskdir::TaskDirError::NotFound { .. })) => {
        // create-on-demand (send-specific NotFound body)
    }
    Err(TryUpdateError::Closure(e)) => {
        eprintln!("famp send: advance_terminal failed for task {task_id}: {e}");
    }
    Err(TryUpdateError::Store(e)) => {
        eprintln!("famp send: failed to persist terminal-advance for task {task_id}: {e}");
    }
}

// await_cmd/mod.rs — commit-receipt branch (lines 173-198)
match tasks.try_update(task_id_str, |mut record| {
    advance_committed(&mut record).map(|_| record)
}) {
    Ok(_) | Err(TryUpdateError::Store(TaskDirError::NotFound { .. })) => {}
    Err(TryUpdateError::Closure(e)) => {
        eprintln!("famp await: advance_committed failed for task {task_id_str}: {e}");
    }
    Err(TryUpdateError::Store(e)) => {
        eprintln!("famp await: failed to persist commit-advance for task {task_id_str}: {e}");
    }
}
```

Shape is structurally identical. Differences are intentional:
- `last_send_at` mutation (send-specific field not present on await side)
- `NotFound` arm body (send-specific create-on-demand logic, vs silent skip on await side)
- Operation name and eprintln label (`send`/`advance_terminal` vs `await`/`advance_committed`)

## Workspace Results

```
cargo nextest run --workspace
Summary [12.123s] 397 tests run: 397 passed, 2 skipped

cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile — 0 warnings
```

397/397 green (+1 from 396 baseline). Clippy clean.

## Out of Scope (Explicitly Not Addressed)

- `try_update` rustdoc or implementation — not touched (quick-260425-lg7 just landed)
- `await_cmd/mod.rs` — not touched (already fixed in quick-260425-ho8)
- `cargo fmt` on unrelated files — not run
- Pre-existing `CliError::Envelope` Display masking `IllegalTransition` string content — not addressed
- Other `let _ =` patterns in the codebase — not audited

## Deviations from Plan

None — plan executed exactly as written. Two atomic commits (RED test + GREEN
fix) produced successfully without pre-commit hook interference.

## Self-Check: PASSED

- `crates/famp/tests/send_terminal_advance_error_surfaces.rs` exists: FOUND
- `crates/famp/src/cli/send/mod.rs` modified: FOUND
- RED commit `22eacd3`: FOUND (`git log --oneline -5`)
- GREEN commit `238e397`: FOUND (`git log --oneline -5`)
- 397/397 workspace tests green: CONFIRMED
- Clippy clean: CONFIRMED
