# Filter Terminal Tasks from `famp_inbox list` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide every inbox entry whose task is terminal in `famp-taskdir` from `famp_inbox list` and `famp inbox list` output, by default. Preserve full-history access via a new `include_terminal` opt-in flag. Leave `famp_await` unfiltered as the canonical originator-completion signal.

**Architecture:** Single-crate change inside `crates/famp`. `run_list` in `cli/inbox/list.rs` gains a `include_terminal: bool` parameter; when `false`, it opens `famp-taskdir` and filters out entries whose `task_id` maps to a record with `terminal == true`. Missing taskdir records fail-open (entry is surfaced); parse/IO errors fail-closed (entry hidden + `eprintln!` warning). Per-call HashMap cache avoids redundant TOML reads. The CLI subcommand and MCP tool thread the flag through with default `false`.

**Tech Stack:** Rust, `clap` derive, `serde_json`, `famp-taskdir`, `famp-inbox`, existing MCP stdio harness.

**Spec:** `docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`

---

## File Structure

**Modify:**
- `crates/famp/src/cli/inbox/list.rs` — filter logic + `extract_task_id` helper
- `crates/famp/src/cli/inbox/mod.rs` — add `--include-terminal` flag to `InboxListArgs`, thread through `run_list` call
- `crates/famp/src/cli/mcp/tools/inbox.rs` — parse `include_terminal` field, reject non-bool, thread through
- `crates/famp/src/cli/await_cmd/mod.rs` — rustdoc only (canonical-completion-signal note)

**Create (tests):**
- `crates/famp/tests/inbox_list_filters_terminal.rs` — unit tests for filter, fail-open, fail-closed, cache

**Extend (tests):**
- `crates/famp/tests/inbox_list_respects_cursor.rs` — update existing `run_list` calls to pass new param (pure call-site update; expectations unchanged)
- `crates/famp/tests/mcp_stdio_tool_calls.rs` — add filtering round-trip tests
- `crates/famp/tests/mcp_malformed_input.rs` — reject non-bool `include_terminal`
- `crates/famp/tests/e2e_two_daemons.rs` — post-completion `list` visibility assertions

**No changes:**
- `crates/famp/Cargo.toml` — `famp-taskdir` already a dep; no new crates
- `crates/famp-taskdir/*` — API already sufficient
- `crates/famp-inbox/*` — not touched
- Daemon (`cli/listen/*`) — not touched
- `cli/await_cmd/mod.rs` logic — rustdoc only

---

## Task 1: Factor out `extract_task_id` helper with exhaustive `MessageClass` test

**Pure refactor.** Locks down the task-id extraction logic before the filter starts depending on it. Current inline match in `run_list` (hardcoded `"request"` string against every other class) gets hoisted into a named helper and covered by a test that enumerates every `famp_core::MessageClass` variant, so future new classes either route correctly or fail the build.

**Files:**
- Modify: `crates/famp/src/cli/inbox/list.rs`
- Test: `crates/famp/tests/inbox_list_filters_terminal.rs` (new, created here; Task 2 extends it)

### Step 1: Write the failing test

- [ ] **Step 1.1: Create the test file**

Create `crates/famp/tests/inbox_list_filters_terminal.rs` with:

```rust
//! Tests for `famp_inbox` filter semantics (spec 2026-04-20).
//!
//! Task 1 covers the `extract_task_id` helper; Tasks 2-4 extend with
//! filter, fail-open/fail-closed, cache, and MCP round-trips.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::cli::inbox::list::extract_task_id_for_test;
use famp_core::MessageClass;
use serde_json::json;

/// Every `MessageClass` variant must either yield a non-empty `task_id`
/// or be explicitly handled. A new variant that lands its id outside
/// the currently-understood envelope shape fails this test.
#[test]
fn extract_task_id_covers_every_message_class() {
    let cases: &[(MessageClass, &str)] = &[
        (MessageClass::Request, "01913000-0000-7000-8000-00000000000a"),
        (MessageClass::Commit, "01913000-0000-7000-8000-00000000000b"),
        (MessageClass::Deliver, "01913000-0000-7000-8000-00000000000c"),
        (MessageClass::Ack, "01913000-0000-7000-8000-00000000000d"),
        (MessageClass::Control, "01913000-0000-7000-8000-00000000000e"),
    ];

    for (class, expected_tid) in cases {
        let value = match class {
            // `request`: envelope's own `id` IS the task_id.
            MessageClass::Request => json!({
                "id": expected_tid,
                "class": class.to_string(),
            }),
            // Every other class: task_id lives in `causality.ref`.
            _ => json!({
                "id": "01913000-0000-7000-8000-0000000000ff",
                "class": class.to_string(),
                "causality": { "ref": expected_tid },
            }),
        };
        let extracted = extract_task_id_for_test(&value);
        assert_eq!(
            extracted,
            *expected_tid,
            "class={class} extracted={extracted:?} expected={expected_tid:?}",
        );
    }
}

// Silencers — match the convention in inbox_list_respects_cursor.rs.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_taskdir as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use humantime as _;
use rand as _;
use rcgen as _;
use reqwest as _;
use rustls as _;
use serde as _;
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tempfile as _;
use tokio as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
```

- [ ] **Step 1.2: Run the test to verify it fails**

```
cargo test -p famp --test inbox_list_filters_terminal
```

Expected: compile error — `extract_task_id_for_test` does not exist. That counts as "failing" for refactor-test-first: the test pins the API we're about to create.

### Step 2: Implement the helper

- [ ] **Step 2.1: Refactor `list.rs` to expose the extractor**

Edit `crates/famp/src/cli/inbox/list.rs`. Replace the inline class match inside `run_list` with a call to a new private helper, and export a `*_for_test` wrapper gated on `cfg(test)` OR make the helper `pub(crate)` with a re-export. Use the `pub fn` + doc-hidden pattern so the integration test can reach it without exposing internals.

Full replacement for the file body (preserve the existing module doc comment at top):

```rust
//! `famp inbox list` — non-blocking dump via `read_from`.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::cli::error::CliError;
use crate::cli::paths;

/// Derive the `task_id` a given inbox entry refers to.
///
/// - `class == "request"`: envelope's `id` field IS the task_id.
/// - Any other class: `causality.ref` carries the task_id.
///
/// Exhaustively covered by `tests/inbox_list_filters_terminal.rs` —
/// adding a new `MessageClass` variant without updating this function
/// will fail that test.
fn extract_task_id(value: &Value) -> &str {
    let class = value.get("class").and_then(Value::as_str).unwrap_or("");
    match class {
        "request" => value.get("id").and_then(Value::as_str).unwrap_or(""),
        _ => value
            .get("causality")
            .and_then(|c| c.get("ref"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    }
}

/// Test-only re-export so integration tests can call `extract_task_id`
/// without widening the module's public surface.
#[doc(hidden)]
pub fn extract_task_id_for_test(value: &Value) -> &str {
    extract_task_id(value)
}

/// Read every inbox entry at or past `since` and write one JSON line
/// per entry to `out`, in the same locked shape as `famp await`.
/// Does NOT advance the cursor.
pub fn run_list(home: &Path, since: Option<u64>, out: &mut dyn Write) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries =
        famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0)).map_err(CliError::Inbox)?;

    for (value, end_offset) in entries {
        let task_id = extract_task_id(&value);
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let body = value.get("body").cloned().unwrap_or(Value::Null);
        let shaped = json!({
            "offset": end_offset,
            "task_id": task_id,
            "from": from,
            "class": class,
            "body": body,
        });
        let line = serde_json::to_string(&shaped).unwrap_or_default();
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: inbox_path.clone(),
            source: e,
        })?;
    }
    Ok(())
}
```

- [ ] **Step 2.2: Run the new test**

```
cargo test -p famp --test inbox_list_filters_terminal extract_task_id_covers_every_message_class
```

Expected: PASS.

- [ ] **Step 2.3: Run the existing cursor test to confirm no regression**

```
cargo test -p famp --test inbox_list_respects_cursor
```

Expected: PASS. (Pure refactor — output shape and cursor semantics unchanged.)

- [ ] **Step 2.4: Run full workspace tests**

```
cargo nextest run
```

Expected: all tests pass. If any other crate imports `run_list` directly, they should continue to compile; this task has not changed the signature.

### Step 3: Commit

- [ ] **Step 3.1: Commit the refactor**

```bash
git add crates/famp/src/cli/inbox/list.rs crates/famp/tests/inbox_list_filters_terminal.rs
git commit -m "$(cat <<'EOF'
refactor(inbox): factor task_id extraction into testable helper

Hoists the inline class-based task_id extraction out of run_list into
a private extract_task_id() + a doc-hidden pub extract_task_id_for_test
wrapper. Adds an exhaustive test over every famp_core::MessageClass
variant so a new class that routes its task_id through a different
envelope field fails the build instead of silently bypassing the
filter landing in Task 2.

Pure refactor. run_list output and cursor semantics unchanged.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Task 2: Add `include_terminal` parameter to `run_list` + filter logic

**Red-green-refactor.** Parameter added, filter implemented with taskdir lookup + per-call HashMap cache + fail-open on missing record + fail-closed with `eprintln!` on parse/IO error.

**Files:**
- Modify: `crates/famp/src/cli/inbox/list.rs`
- Modify: `crates/famp/src/cli/inbox/mod.rs` (update the single call site — Task 3 will add the flag)
- Modify: `crates/famp/tests/inbox_list_respects_cursor.rs` (update two `run_list(&home, ...)` call sites — pass `true` for include_terminal so behavior matches pre-filter tests with no taskdir records seeded; verifies the flag works as a pass-through)
- Test: `crates/famp/tests/inbox_list_filters_terminal.rs` (extend with filter + fail-open + fail-closed + cache tests)

### Step 1: Write the failing filter test

- [ ] **Step 1.1: Add a helper for building inbox fixtures**

Append to `crates/famp/tests/inbox_list_filters_terminal.rs`, below the existing test:

```rust
use famp_taskdir::{TaskDir, TaskRecord};
use std::path::Path;

fn write_inbox(home: &Path, lines: &[serde_json::Value]) {
    let mut body = Vec::<u8>::new();
    for line in lines {
        body.extend_from_slice(serde_json::to_string(line).unwrap().as_bytes());
        body.push(b'\n');
    }
    std::fs::write(home.join("inbox.jsonl"), body).unwrap();
}

fn seed_taskdir(home: &Path, task_id: &str, peer: &str, terminal: bool) {
    let tasks = home.join("tasks");
    let dir = TaskDir::open(&tasks).unwrap();
    let mut rec = TaskRecord::new_requested(
        task_id.to_string(),
        peer.to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    );
    if terminal {
        rec.state = "COMPLETED".to_string();
        rec.terminal = true;
    }
    dir.create(&rec).unwrap();
}

const TID_ACTIVE: &str = "01913000-0000-7000-8000-0000000000a1";
const TID_DONE: &str = "01913000-0000-7000-8000-0000000000a2";

fn fixture_entries() -> [serde_json::Value; 4] {
    [
        json!({
            "id": TID_ACTIVE,
            "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE },
            "body": { "text": "active-request" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000b1",
            "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE },
            "body": { "text": "active-deliver" },
        }),
        json!({
            "id": TID_DONE,
            "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "done-request" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000b2",
            "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "done-deliver" },
        }),
    ]
}

#[test]
fn list_hides_entries_for_terminal_tasks_by_default() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, /* include_terminal */ false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "only active task entries visible: {text}");
    for l in &lines {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["task_id"].as_str().unwrap(), TID_ACTIVE);
    }
}

#[test]
fn list_include_terminal_returns_all_entries() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, /* include_terminal */ true, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "all four entries returned with override");
}

#[test]
fn list_fail_open_on_missing_taskdir_record() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    // Seed inbox but NO taskdir records at all.
    write_inbox(&home, &fixture_entries());

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "missing taskdir records fail-open (surface entry): {text}"
    );
}

#[test]
fn list_fail_closed_on_corrupt_taskdir_record() {
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    write_inbox(&home, &fixture_entries());
    // Seed one valid (active) and one *corrupt* record for TID_DONE.
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    let corrupt_path = home.join("tasks").join(format!("{TID_DONE}.toml"));
    std::fs::write(&corrupt_path, "this is not valid toml ===").unwrap();

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "corrupt record → fail-closed → entry hidden: {text}"
    );
    for l in &lines {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["task_id"].as_str().unwrap(), TID_ACTIVE);
    }
}

#[test]
fn list_caches_taskdir_reads_within_one_call() {
    // Three entries all referencing the same terminal task.
    // After this call, the taskdir has been consulted exactly once
    // per distinct task_id (we can't directly count reads without
    // injecting a wrapper, so the behavioural assertion is: all
    // three entries are hidden uniformly — same answer every time).
    use famp::cli::inbox::list::run_list;

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    let entries = vec![
        json!({
            "id": TID_DONE, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e1" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000c1", "class": "commit",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e2" },
        }),
        json!({
            "id": "01913000-0000-7000-8000-0000000000c2", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE },
            "body": { "text": "e3" },
        }),
    ];
    write_inbox(&home, &entries);
    seed_taskdir(&home, TID_DONE, "a", true);

    let mut buf = Vec::<u8>::new();
    run_list(&home, None, false, &mut buf).unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(
        text.lines().next().is_none(),
        "all three entries for the same terminal task are hidden: {text}"
    );
}
```

- [ ] **Step 1.2: Run tests — expect compile failure**

```
cargo test -p famp --test inbox_list_filters_terminal
```

Expected: compile error — `run_list` takes 3 args, not 4. That's the red state.

### Step 2: Implement the filter

- [ ] **Step 2.1: Update `run_list` in `crates/famp/src/cli/inbox/list.rs`**

Two edits to the same file:

**A. Add these imports at the top of the file**, alongside the existing `use` statements (do not remove the existing imports from Task 1):

```rust
use std::collections::HashMap;

use famp_taskdir::{TaskDir, TaskDirError};
```

**B. Replace the existing `run_list` function with the new signature and body** (keep `extract_task_id` and `extract_task_id_for_test` from Task 1 unchanged). Also add the `is_terminal_cached` helper immediately after `run_list`:

```rust
/// Read every inbox entry at or past `since` and write one JSON line
/// per entry to `out`, in the same locked shape as `famp await`.
/// Does NOT advance the cursor.
///
/// # Filtering
///
/// By default (`include_terminal = false`) entries whose `task_id`
/// maps to a taskdir record with `terminal == true` are omitted.
///
/// - Missing taskdir record → **fail-open**: entry is surfaced.
/// - Corrupt taskdir record (TOML parse / IO error) → **fail-closed**:
///   entry is hidden; a diagnostic is written to stderr via
///   `eprintln!`. A corrupt record for a terminal task must not
///   resurrect its history into `list` forever; operator visibility
///   comes through stderr.
///
/// # Canonical completion signal
///
/// `list` is not the place to learn that a task just completed. Once
/// the daemon flips a task's taskdir record to `terminal = true`, the
/// closing deliver is hidden here. Agents that need real-time
/// completion notifications MUST use `famp await`, which is
/// deliberately unfiltered.
pub fn run_list(
    home: &Path,
    since: Option<u64>,
    include_terminal: bool,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let inbox_path = paths::inbox_jsonl_path(home);
    let entries =
        famp_inbox::read::read_from(&inbox_path, since.unwrap_or(0)).map_err(CliError::Inbox)?;

    // Only open the taskdir when filtering. If it fails to open
    // (e.g. fresh FAMP_HOME with no tasks dir), fall back to "filter
    // disabled for this call" — equivalent to include_terminal=true.
    // Opening a TaskDir mkdir -p's the root, so normal paths succeed.
    let taskdir: Option<TaskDir> = if include_terminal {
        None
    } else {
        match TaskDir::open(paths::tasks_dir(home)) {
            Ok(td) => Some(td),
            Err(err) => {
                eprintln!("famp inbox list: taskdir unavailable, filter disabled: {err}");
                None
            }
        }
    };
    let mut terminal_cache: HashMap<String, bool> = HashMap::new();

    for (value, end_offset) in entries {
        let task_id = extract_task_id(&value);
        if let Some(ref td) = taskdir {
            if task_id.is_empty() {
                // Nothing to look up — fail-open.
            } else if is_terminal_cached(td, task_id, &mut terminal_cache) {
                continue;
            }
        }
        let from = value.get("from").and_then(Value::as_str).unwrap_or("");
        let class = value.get("class").and_then(Value::as_str).unwrap_or("");
        let body = value.get("body").cloned().unwrap_or(Value::Null);
        let shaped = json!({
            "offset": end_offset,
            "task_id": task_id,
            "from": from,
            "class": class,
            "body": body,
        });
        let line = serde_json::to_string(&shaped).unwrap_or_default();
        writeln!(out, "{line}").map_err(|e| CliError::Io {
            path: inbox_path.clone(),
            source: e,
        })?;
    }
    Ok(())
}

/// Cached taskdir lookup. Returns `true` if the entry should be hidden.
///
/// Rules:
/// - `NotFound`        → `false` (fail-open; surface entry).
/// - `Ok(rec)`         → `rec.terminal`.
/// - any other error   → `true`  (fail-closed; hide entry + eprintln).
fn is_terminal_cached(
    td: &TaskDir,
    task_id: &str,
    cache: &mut HashMap<String, bool>,
) -> bool {
    if let Some(cached) = cache.get(task_id) {
        return *cached;
    }
    let verdict = match td.read(task_id) {
        Ok(rec) => rec.terminal,
        Err(TaskDirError::NotFound { .. }) => false,
        Err(other) => {
            eprintln!(
                "famp inbox list: hiding entry for task_id={task_id}: {other}",
            );
            true
        }
    };
    cache.insert(task_id.to_string(), verdict);
    verdict
}
```

- [ ] **Step 2.2: Update the CLI dispatcher in `crates/famp/src/cli/inbox/mod.rs`**

Change the `List` arm of `run` to pass `false` for now (Task 3 adds the CLI flag):

```rust
        InboxCommand::List(list_args) => {
            let mut stdout = std::io::stdout();
            list::run_list(&home, list_args.since, false, &mut stdout)
        }
```

- [ ] **Step 2.3: Update existing cursor test call sites**

Edit `crates/famp/tests/inbox_list_respects_cursor.rs`. Both `run_list(&home, None, &mut buf)` and `run_list(&home, Some(off2), &mut buf)` become `run_list(&home, None, true, &mut buf)` and `run_list(&home, Some(off2), true, &mut buf)` respectively (no taskdir records seeded → with `true`, behavior is unchanged from the test's intent).

Rationale: this test was written against the pre-filter API and asserts raw cursor behavior. Running it with `true` preserves that intent and keeps the test focused on cursor mechanics. The new filter-terminal test file exercises the `false` path end-to-end.

- [ ] **Step 2.4: Update MCP tool call site in `crates/famp/src/cli/mcp/tools/inbox.rs`**

Temporarily hardcode `false` so the build compiles. Task 4 wires the field through from the JSON input.

```rust
        "list" => {
            let since = input["since"].as_u64();
            let mut buf = Vec::<u8>::new();
            list::run_list(home, since, /* include_terminal */ false, &mut buf)?;
            // ... rest unchanged
        }
```

- [ ] **Step 2.5: Run the new filter tests**

```
cargo test -p famp --test inbox_list_filters_terminal
```

Expected: all 6 tests in the file (1 from Task 1 + 5 from Task 2) PASS.

- [ ] **Step 2.6: Run the cursor test**

```
cargo test -p famp --test inbox_list_respects_cursor
```

Expected: PASS. Call-site update didn't break cursor semantics.

- [ ] **Step 2.7: Run full workspace tests**

```
cargo nextest run
```

Expected: all PASS.

### Step 3: Commit

- [ ] **Step 3.1: Commit**

```bash
git add crates/famp/src/cli/inbox/list.rs \
        crates/famp/src/cli/inbox/mod.rs \
        crates/famp/src/cli/mcp/tools/inbox.rs \
        crates/famp/tests/inbox_list_filters_terminal.rs \
        crates/famp/tests/inbox_list_respects_cursor.rs
git commit -m "$(cat <<'EOF'
feat(inbox): filter terminal-task entries from run_list by default

run_list now takes include_terminal: bool. When false, entries whose
task_id maps to a famp-taskdir record with terminal=true are omitted.
Missing records fail-open; parse/IO errors fail-closed with an
eprintln! diagnostic. Lookups are cached per-call.

CLI and MCP call sites pin include_terminal=false for now; Tasks 3-4
thread the caller-facing flag through.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Task 3: Wire `--include-terminal` flag into `famp inbox list`

**Files:**
- Modify: `crates/famp/src/cli/inbox/mod.rs`
- Test: `crates/famp/tests/inbox_list_filters_terminal.rs` (extend with CLI invocation)

### Step 1: Write the failing test

- [ ] **Step 1.1: Add a CLI invocation test**

Append to `crates/famp/tests/inbox_list_filters_terminal.rs`:

```rust
use std::process::Command;

/// Drive the `famp` binary through the CLI subcommand to assert the
/// flag is wired end-to-end (parsed, passed to run_list, reflected
/// in stdout).
#[test]
fn cli_inbox_list_respects_include_terminal_flag() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();

    // Fresh identity — `famp init` creates keys + tasks dir.
    let status = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["init"])
        .env("FAMP_HOME", &home)
        .status()
        .expect("famp init");
    assert!(status.success());

    write_inbox(&home, &fixture_entries());
    seed_taskdir(&home, TID_ACTIVE, "a", false);
    seed_taskdir(&home, TID_DONE, "a", true);

    // Default: filtered (2 lines).
    let out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["inbox", "list"])
        .env("FAMP_HOME", &home)
        .output()
        .expect("famp inbox list");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        2,
        "default filter hides terminal task: {stdout}",
    );

    // --include-terminal: unfiltered (4 lines).
    let out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["inbox", "list", "--include-terminal"])
        .env("FAMP_HOME", &home)
        .output()
        .expect("famp inbox list --include-terminal");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        4,
        "override returns every entry: {stdout}",
    );
}
```

- [ ] **Step 1.2: Run the test — expect failure**

```
cargo test -p famp --test inbox_list_filters_terminal cli_inbox_list_respects_include_terminal_flag
```

Expected: FAIL. The binary doesn't know `--include-terminal` yet; clap errors out.

### Step 2: Implement

- [ ] **Step 2.1: Add the flag to `InboxListArgs`**

Edit `crates/famp/src/cli/inbox/mod.rs`:

```rust
#[derive(clap::Args, Debug)]
pub struct InboxListArgs {
    #[arg(long)]
    pub since: Option<u64>,
    /// Include entries for tasks that have reached a terminal FSM state
    /// (COMPLETED, FAILED, CANCELLED). Off by default: finished tasks
    /// stay out of the active view. Use `famp await` for real-time
    /// completion notifications.
    #[arg(long)]
    pub include_terminal: bool,
}
```

- [ ] **Step 2.2: Thread the flag into `run_list`**

Same file, `run` function:

```rust
        InboxCommand::List(list_args) => {
            let mut stdout = std::io::stdout();
            list::run_list(
                &home,
                list_args.since,
                list_args.include_terminal,
                &mut stdout,
            )
        }
```

- [ ] **Step 2.3: Run the CLI test**

```
cargo test -p famp --test inbox_list_filters_terminal cli_inbox_list_respects_include_terminal_flag
```

Expected: PASS.

- [ ] **Step 2.4: Run full workspace tests**

```
cargo nextest run
```

Expected: all PASS.

### Step 3: Commit

- [ ] **Step 3.1: Commit**

```bash
git add crates/famp/src/cli/inbox/mod.rs crates/famp/tests/inbox_list_filters_terminal.rs
git commit -m "$(cat <<'EOF'
feat(cli): add --include-terminal flag to `famp inbox list`

Wires the filter override through the clap subcommand. Default (flag
absent) filters terminal-task entries; --include-terminal returns the
full log. Covered by an end-to-end CARGO_BIN_EXE_famp test driving
the binary through stdin/stdout.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Task 4: Wire `include_terminal` into `famp_inbox` MCP tool + reject non-bool

**Files:**
- Modify: `crates/famp/src/cli/mcp/tools/inbox.rs`
- Test: `crates/famp/tests/mcp_stdio_tool_calls.rs` (extend)
- Test: `crates/famp/tests/mcp_malformed_input.rs` (extend)

### Step 1: Write the failing tests

- [ ] **Step 1.1: Add the MCP round-trip tests**

Append to `crates/famp/tests/mcp_stdio_tool_calls.rs`. The file already defines `McpHarness`, `send_msg`, and `recv_msg` (newline-delimited JSON framing, no Content-Length). Reuse them.

```rust
// Spec 2026-04-20: `famp_inbox` action=list filters terminal tasks
// unless include_terminal=true. These two tests assert the MCP
// surface, driving the binary through its real stdio JSON-RPC loop.

use famp_taskdir::{TaskDir, TaskRecord};

const TID_ACTIVE_MCP: &str = "01913000-0000-7000-8000-0000000000f1";
const TID_DONE_MCP: &str = "01913000-0000-7000-8000-0000000000f2";

/// Write a four-entry inbox fixture (two per task) + matching taskdir
/// records (one active, one terminal) into `home`. Returns the
/// task_ids for the caller to assert against.
fn seed_filter_fixture(home: &std::path::Path) {
    let entries = [
        serde_json::json!({
            "id": TID_ACTIVE_MCP, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE_MCP },
            "body": { "text": "active-request" },
        }),
        serde_json::json!({
            "id": "01913000-0000-7000-8000-0000000000e1", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_ACTIVE_MCP },
            "body": { "text": "active-deliver" },
        }),
        serde_json::json!({
            "id": TID_DONE_MCP, "class": "request",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE_MCP },
            "body": { "text": "done-request" },
        }),
        serde_json::json!({
            "id": "01913000-0000-7000-8000-0000000000e2", "class": "deliver",
            "from": "agent:localhost/a",
            "causality": { "ref": TID_DONE_MCP },
            "body": { "text": "done-deliver" },
        }),
    ];
    let mut body = Vec::<u8>::new();
    for e in &entries {
        body.extend_from_slice(serde_json::to_string(e).unwrap().as_bytes());
        body.push(b'\n');
    }
    std::fs::write(home.join("inbox.jsonl"), body).unwrap();

    let dir = TaskDir::open(home.join("tasks")).unwrap();
    dir.create(&TaskRecord::new_requested(
        TID_ACTIVE_MCP.to_string(),
        "a".to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    ))
    .unwrap();
    let mut done = TaskRecord::new_requested(
        TID_DONE_MCP.to_string(),
        "a".to_string(),
        "2026-04-20T00:00:00Z".to_string(),
    );
    done.state = "COMPLETED".to_string();
    done.terminal = true;
    dir.create(&done).unwrap();
}

fn call_inbox_list(h: &mut McpHarness, include_terminal: Option<bool>) -> serde_json::Value {
    let mut args = serde_json::json!({ "action": "list" });
    if let Some(b) = include_terminal {
        args["include_terminal"] = serde_json::Value::Bool(b);
    }
    h.send(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": "famp_inbox", "arguments": args }
    }));
    h.recv()
}

/// Extract the `entries` array from a tools/call result. The MCP
/// wrapper returns tool output in result.content[0].text as a JSON
/// string; parse it back.
fn entries_from_response(resp: &serde_json::Value) -> Vec<serde_json::Value> {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text in response: {resp}"));
    let parsed: serde_json::Value = serde_json::from_str(text)
        .unwrap_or_else(|_| panic!("tool output not JSON: {text}"));
    parsed["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("no entries array: {parsed}"))
        .clone()
}

#[test]
fn famp_inbox_list_filters_terminal_by_default() {
    let mut h = McpHarness::new();
    seed_filter_fixture(h.home.path());

    let resp = call_inbox_list(&mut h, None);
    let entries = entries_from_response(&resp);
    assert_eq!(entries.len(), 2, "default filter: {resp}");
    for e in &entries {
        assert_eq!(e["task_id"].as_str().unwrap(), TID_ACTIVE_MCP);
    }

    drop(h);
}

#[test]
fn famp_inbox_list_include_terminal_true_returns_all() {
    let mut h = McpHarness::new();
    seed_filter_fixture(h.home.path());

    let resp = call_inbox_list(&mut h, Some(true));
    let entries = entries_from_response(&resp);
    assert_eq!(entries.len(), 4, "include_terminal=true: {resp}");

    drop(h);
}
```

If `McpHarness::home` is private (it is in the file as of this writing — it's a field of the struct), either (a) add a `pub(crate) fn home_path(&self) -> &Path` method on the harness and use that, or (b) make the `home` field accessible to test bodies via `pub(crate)`. Pick whichever matches the surrounding style; a getter is cleaner if the rest of the file has getters.

- [ ] **Step 1.2: Add the malformed-input test**

Append to `crates/famp/tests/mcp_malformed_input.rs`. The file uses newline-delimited JSON framing (see existing tests). Reuse `spawn_mcp` + `recv_msg` at the top of the file.

```rust
#[test]
fn famp_inbox_list_rejects_non_bool_include_terminal() {
    let home = tempfile::tempdir().expect("tempdir");
    let (mut child, mut stdin, mut stdout) = spawn_mcp(home.path());

    // Send a tools/call with include_terminal as a string.
    // The server must reject it with a tool-level error, not coerce.
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "famp_inbox",
            "arguments": {
                "action": "list",
                "include_terminal": "true"
            }
        }
    });
    let mut body = serde_json::to_string(&req).unwrap();
    body.push('\n');
    stdin.write_all(body.as_bytes()).expect("write");
    stdin.flush().expect("flush");

    let resp = recv_msg(&mut stdout, Duration::from_secs(5));
    // The error surfaces either as JSON-RPC error or as an
    // isError=true tool result. Either way the message must name the
    // field and the expected type so a caller can self-correct.
    let text = resp.to_string();
    assert!(
        text.contains("include_terminal"),
        "error must name the field: {resp}",
    );
    assert!(
        text.to_lowercase().contains("boolean") || text.to_lowercase().contains("bool"),
        "error must name the expected type: {resp}",
    );

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}
```

- [ ] **Step 1.3: Run — expect failure**

```
cargo test -p famp --test mcp_stdio_tool_calls famp_inbox_list
cargo test -p famp --test mcp_malformed_input famp_inbox_list_rejects_non_bool_include_terminal
```

Expected: FAIL (either the filter isn't wired, or the schema rejection isn't in place, depending on which test runs).

### Step 2: Implement

- [ ] **Step 2.1: Update `crates/famp/src/cli/mcp/tools/inbox.rs`**

Replace the `"list"` match arm with the version that parses + rejects non-bool:

```rust
        "list" => {
            let since = input["since"].as_u64();
            let include_terminal = match input.get("include_terminal") {
                None | Some(Value::Null) => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => {
                    return Err(CliError::SendArgsInvalid {
                        reason:
                            "famp_inbox: 'include_terminal' must be a boolean"
                                .to_string(),
                    });
                }
            };
            let mut buf = Vec::<u8>::new();
            list::run_list(home, since, include_terminal, &mut buf)?;

            // ... existing parse-line-by-line block unchanged ...
            let text = std::str::from_utf8(&buf).map_err(|e| CliError::Io {
                path: std::path::PathBuf::new(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            })?;
            let mut entries: Vec<Value> = Vec::new();
            for (idx, line) in text.lines().filter(|l| !l.is_empty()).enumerate() {
                let parsed: Value =
                    serde_json::from_str(line).map_err(|err| CliError::SendArgsInvalid {
                        reason: format!("famp_inbox: list line {idx} is not valid JSON: {err}"),
                    })?;
                entries.push(parsed);
            }
            Ok(serde_json::json!({ "entries": entries }))
        }
```

Also update the module docstring at the top of the file to mention the new field:

```rust
//! Input shape (JSON):
//! ```json
//! {
//!   "action": "list" | "ack",
//!   "since":  123,         // optional byte offset for list
//!   "include_terminal": false, // optional bool for list; default false
//!   "offset": 456          // required for ack
//! }
//! ```
```

- [ ] **Step 2.2: Run the new tests**

```
cargo test -p famp --test mcp_stdio_tool_calls famp_inbox_list
cargo test -p famp --test mcp_malformed_input famp_inbox_list_rejects_non_bool_include_terminal
```

Expected: PASS.

- [ ] **Step 2.3: Run full workspace tests**

```
cargo nextest run
```

Expected: all PASS.

### Step 3: Commit

- [ ] **Step 3.1: Commit**

```bash
git add crates/famp/src/cli/mcp/tools/inbox.rs \
        crates/famp/tests/mcp_stdio_tool_calls.rs \
        crates/famp/tests/mcp_malformed_input.rs
git commit -m "$(cat <<'EOF'
feat(mcp): accept include_terminal in famp_inbox list + reject non-bool

- famp_inbox action=list honors optional include_terminal:bool
  (default false) and passes it through to run_list.
- Non-bool values produce a hard CliError::SendArgsInvalid with a
  message naming the field and the expected type — no silent coercion
  of the string "true" to boolean true.
- Extends mcp_stdio_tool_calls with filter + override round-trips
  and mcp_malformed_input with the rejection regression.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Task 5: Extend E2E test — post-completion `list` visibility

**Files:**
- Modify: `crates/famp/tests/e2e_two_daemons.rs`

### Step 1: Inspect the existing test

- [ ] **Step 1.1: Read the current test**

```
cargo test -p famp --test e2e_two_daemons -- --list
```

Then open `crates/famp/tests/e2e_two_daemons.rs` and locate the test that walks a task through completion. Identify:
- the `task_id` of the completed task
- the originator's `FAMP_HOME`
- the point in the test where the task is confirmed terminal

### Step 2: Write the failing assertions

- [ ] **Step 2.1: Add post-completion list assertions**

After the existing block that confirms the task reached `COMPLETED`, add:

```rust
// After task is COMPLETED on the originator, calling
// `run_list` with the default filter must return zero entries
// for that task_id. With include_terminal=true, the closing
// deliver is visible.

use famp::cli::inbox::list::run_list;

let mut filtered = Vec::<u8>::new();
run_list(&alice_home, None, /* include_terminal */ false, &mut filtered).unwrap();
let filtered_text = String::from_utf8(filtered).unwrap();
for line in filtered_text.lines() {
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_ne!(
        v["task_id"].as_str().unwrap(),
        task_id.as_str(),
        "default filter must hide completed task entries: {line}",
    );
}

let mut unfiltered = Vec::<u8>::new();
run_list(&alice_home, None, /* include_terminal */ true, &mut unfiltered).unwrap();
let unfiltered_text = String::from_utf8(unfiltered).unwrap();
let matching = unfiltered_text
    .lines()
    .filter(|l| {
        serde_json::from_str::<serde_json::Value>(l)
            .unwrap()["task_id"]
            .as_str()
            .unwrap()
            == task_id.as_str()
    })
    .count();
assert!(
    matching >= 2,
    "include_terminal=true surfaces the completed task's entries (request + deliver): count={matching}",
);
```

Substitute `alice_home` and `task_id` with the variable names used in the existing test. If the test originates on Bob's side instead, use Bob's home — whichever identity drove the request.

- [ ] **Step 2.2: Run the test**

```
cargo test -p famp --test e2e_two_daemons
```

Expected: PASS (both assertions — the filter behavior implemented in Tasks 2-4 already satisfies them; this task locks that behavior down in an e2e harness).

### Step 3: Commit

- [ ] **Step 3.1: Commit**

```bash
git add crates/famp/tests/e2e_two_daemons.rs
git commit -m "$(cat <<'EOF'
test(e2e): assert run_list hides completed task on originator

Extends the two-daemon end-to-end to assert that after a task reaches
COMPLETED on the originator side, default run_list returns zero
entries for that task_id, and include_terminal=true surfaces the
original request + closing deliver.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Task 6: Rustdoc — document `await` as the canonical completion signal

**Files:**
- Modify: `crates/famp/src/cli/await_cmd/mod.rs` (doc comments only)
- Modify: `crates/famp/src/cli/mcp/tools/await_.rs` (doc comments only)

### Step 1: Update `await_cmd/mod.rs`

- [ ] **Step 1.1: Add/extend the module docstring**

Prepend or merge into the existing module doc comment at the top of `crates/famp/src/cli/await_cmd/mod.rs`:

```rust
//! `famp await` — block until a new inbox entry arrives.
//!
//! # Relationship to `famp inbox list`
//!
//! `await` is deliberately unfiltered. `famp inbox list` filters out
//! entries for tasks in a terminal FSM state by default (spec
//! 2026-04-20-filter-terminal-tasks-from-inbox-list-design.md), which
//! means the closing `deliver` for a task you originated is NOT
//! visible via `list` after the daemon flips the taskdir record.
//!
//! `await` IS the canonical real-time signal for task completion.
//! Agents waiting for a task to close should `await`; `list` is for
//! "what's still on my plate."
```

### Step 2: Update the MCP tool doc

- [ ] **Step 2.1: Add an equivalent note to `crates/famp/src/cli/mcp/tools/await_.rs`**

Append a paragraph to the module docstring:

```rust
//! # Relationship to `famp_inbox`
//!
//! `famp_await` returns every new inbox entry as it arrives, including
//! the terminal `deliver` that closes a task. `famp_inbox` action=list
//! filters those out by default. An agent that needs to act on task
//! completion MUST await; list is not a real-time stream.
```

### Step 3: Check for rustdoc warnings and commit

- [ ] **Step 3.1: Build with doc lints**

```
RUSTDOCFLAGS="-D warnings" cargo doc -p famp --no-deps
```

Expected: clean. Broken intra-doc links fail the build.

- [ ] **Step 3.2: Run the full test suite one last time**

```
cargo nextest run
```

Expected: all PASS.

- [ ] **Step 3.3: Commit**

```bash
git add crates/famp/src/cli/await_cmd/mod.rs crates/famp/src/cli/mcp/tools/await_.rs
git commit -m "$(cat <<'EOF'
docs: mark `famp await` as the canonical task-completion signal

`famp inbox list` filters terminal-task entries by default as of the
previous commits. That means the closing deliver for a task is NOT
visible via list once the daemon flips the taskdir record. Document
in both the CLI and MCP await modules that await is the real-time
unfiltered stream agents should use to learn of completion.

Refs: docs/superpowers/specs/2026-04-20-filter-terminal-tasks-from-inbox-list-design.md
EOF
)"
```

---

## Self-Review Checklist (performed while writing)

**Spec coverage:**
- Filter at read time via taskdir → Task 2 ✓
- `include_terminal` flag default false → Tasks 2-4 ✓
- `famp_await` untouched (rustdoc updates only) → Task 6 ✓
- Fail-open on missing record → Task 2 test + impl ✓
- Fail-closed on corrupt record with `eprintln!` → Task 2 test + impl ✓
- HashMap cache per call → Task 2 ✓
- `extract_task_id` helper + exhaustive MessageClass test → Task 1 ✓
- Non-bool `include_terminal` rejected as tool error → Task 4 ✓
- E2E: originator post-completion list empty + override returns → Task 5 ✓
- Rustdoc noting await is canonical → Task 6 ✓
- Cursor/ack semantics unchanged (no task needed; pre-existing test locks it) ✓

**Type consistency:**
- `run_list` signature: `(&Path, Option<u64>, bool, &mut dyn Write) -> Result<(), CliError>` — consistent across Tasks 2-5 and all callers.
- `is_terminal_cached` name, signature, and cache type (`HashMap<String, bool>`) consistent.
- `InboxListArgs` field `include_terminal` (snake_case for serde/CLI parity).
- MCP JSON field name `include_terminal` (snake_case) everywhere.

**No placeholders:** every task has runnable test code, runnable commands, full commit messages. The only prose-only step is Task 4 Step 1.1 where I describe the two MCP round-trip tests rather than inlining them — the reason is that they reuse test-file-local `spawn_mcp` / `recv_msg` helpers whose exact signatures live in the file being modified, and copying them verbatim here would diverge the moment the helpers evolve. The step names those helpers explicitly and points to `inbox_list_filters_terminal.rs` for the fixture helpers to port over.
