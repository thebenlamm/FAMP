# FAMP Listen Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing `famp-await.sh` Stop hook to activate via transcript detection instead of a sentinel file, switch to the v0.9 `--as <identity>` proxy model, add a `listen: bool` opt-in to `famp_register`, and harden the wake signal to notification-only.

**Architecture:** The Stop hook parses the Claude Code session transcript for a successful `famp_register` call with `listen: true` and no subsequent `famp_leave`. When found, it blocks on `famp await --as <identity> --timeout 23h`. On message arrival it emits a notification-only `{"decision": "block", "reason": "New FAMP message from <sender>. Call famp_inbox to read it."}` — no peer-controlled bytes in the reason field.

**Tech Stack:** Bash (hook), Python 3 (transcript parsing embedded in hook), Rust (MCP schema, tests), `jq` (JSON construction in hook), `cargo nextest` (test runner).

---

## File Map

| File | Action | Purpose |
|------|---------|---------|
| `crates/famp/src/cli/mcp/server.rs` | Modify ~line 88 | Add `listen` field to `famp_register` input schema |
| `~/.claude/hooks/famp-await.sh` | Rewrite | Transcript detection, `--as` proxy, notification-only reason |
| `crates/famp/tests/hook_runner_await.rs` | Create | Unit tests for transcript identity extraction |
| `crates/famp/tests/await_timeout.rs` | Fill stub | Real broker-backed await timeout test |
| `crates/famp/tests/mcp_bus_e2e.rs` | Extend | Listen loop E2E scenario |
| `CLAUDE.md` | Modify | Document `listen: true` usage |

---

### Task 1: Add `listen` field to `famp_register` MCP schema

**Files:**
- Modify: `crates/famp/src/cli/mcp/server.rs` (~line 88)

The Rust tool implementation (`tools/register.rs`) needs no behavior change — `listen` is a hook-only hint read from the transcript. Only the JSON schema the LLM sees needs updating.

- [ ] **Step 1: Update the inputSchema**

In `crates/famp/src/cli/mcp/server.rs`, find the `famp_register` tool block (around line 86) and replace the `inputSchema` properties:

```rust
// BEFORE:
"properties": {
    "identity": { "type": "string", "description": "Identity name (matches [A-Za-z0-9_-]+). Resolves to $FAMP_LOCAL_ROOT/agents/<identity>/." }
},
"required": ["identity"]

// AFTER:
"properties": {
    "identity": { "type": "string", "description": "Identity name (matches [A-Za-z0-9_-]+)." },
    "listen": { "type": "boolean", "description": "If true, this window enters listen mode: the Stop hook will block on famp_await after each turn and wake Claude when a message arrives. Default false. Use true for dedicated agent windows; omit for general-purpose windows." }
},
"required": ["identity"]
```

- [ ] **Step 2: Also update the description string** to mention `listen`:

```rust
// Replace the famp_register "description" value with:
"description": "Bind this MCP session to a FAMP identity. CALL THIS FIRST in every new window — without it, famp_send/famp_await/famp_inbox/famp_peers return a typed 'not_registered' error. Pass listen:true to enter listen mode: after each turn the Stop hook will block waiting for inbound messages and wake Claude automatically (sub-minute latency). Omit listen or pass false for general-purpose windows that check inbox on demand."
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p famp 2>&1 | tail -5
```
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/famp/src/cli/mcp/server.rs
git commit -m "feat(mcp): add listen field to famp_register input schema"
```

---

### Task 2: Write transcript extraction unit tests (RED)

**Files:**
- Create: `crates/famp/tests/hook_runner_await.rs`

These tests validate the Python snippet that will be embedded in `famp-await.sh`. They spawn the hook with a fake `famp` binary and a crafted transcript, asserting whether or not `famp await --as <name>` is invoked.

The hook does not yet implement transcript detection, so all "should enter listen mode" tests will fail (no-op exit) and all "should not listen" tests will pass trivially. Both outcomes confirm the tests are meaningful.

- [ ] **Step 1: Create the test file**

```bash
cat > crates/famp/tests/hook_runner_await.rs << 'RUST'
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

//! Tests for the transcript-detection path of `famp-await.sh`.
//!
//! Each test spawns the hook with a crafted transcript and a mock `famp`
//! binary that records its argv. Tests assert whether `famp await --as
//! <name>` was invoked (listen mode entered) or not (no-op).

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn hook_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir")
        .join(".claude/hooks/famp-await.sh")
}

/// Write a mock `famp` binary into `bin_dir` that records its full argv
/// to `log_file` and then exits 0.
fn stage_mock_famp(bin_dir: &Path, log_file: &Path) {
    std::fs::create_dir_all(bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> \"{}\"\nexit 0\n",
            log_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Build a Claude Code transcript JSONL with a `famp_register` tool call
/// and a matching tool_result. `listen` controls the input flag; `ok`
/// controls whether the result is a success.
fn make_transcript(
    path: &Path,
    identity: &str,
    listen: bool,
    ok: bool,
    with_leave_after: bool,
) {
    let tool_use_id = "toolu_test1";
    let result_content = if ok {
        format!(r#"[{{"type":"text","text":"{{\\"active\\":\\"{identity}\\",\\"drained\\":0,\\"peers\\":[]}}"}}"#)
    } else {
        r#"[{"type":"text","text":"name already taken"}]"#.to_string()
    };
    let is_error = if ok { "false" } else { "true" };
    let listen_str = if listen { "true" } else { "false" };

    let mut body = format!(
        r#"{{"type":"user","message":{{"role":"user","content":"register"}}}}
{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{tool_use_id}","name":"mcp__famp__famp_register","input":{{"identity":"{identity}","listen":{listen_str}}}}}]}}}}
{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{tool_use_id}","is_error":{is_error},"content":{result_content}}}]}}}}
"#
    );

    if with_leave_after {
        let leave_id = "toolu_leave1";
        body.push_str(&format!(
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{leave_id}","name":"mcp__famp__famp_leave","input":{{}}}}]}}}}
"#
        ));
    }

    std::fs::write(path, body).unwrap();
}

fn run_hook(
    hook: &Path,
    transcript: &Path,
    bin_dir: &Path,
    log: &Path,
    xdg_state: &Path,
) -> std::process::Output {
    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());

    let mut child = Command::new("bash")
        .arg(hook)
        .env("PATH", &new_path)
        .env("FAKE_FAMP_LOG", log)
        .env("XDG_STATE_HOME", xdg_state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stop_json.as_bytes())
        .unwrap();
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn listen_true_and_successful_register_enters_listen_mode() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook failed: {:?}", String::from_utf8_lossy(&out.stderr));

    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("await") && argv.contains("--as") && argv.contains("dk"),
        "expected famp await --as dk invocation, got: {argv:?}"
    );
}

#[test]
fn listen_false_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", false, true, false);

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for listen:false"
    );
}

#[test]
fn failed_register_result_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, false, false);  // ok=false

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for failed register"
    );
}

#[test]
fn register_then_leave_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, true);  // with_leave_after=true

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation after famp_leave"
    );
}

#[test]
fn no_register_in_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    std::fs::write(&transcript, r#"{"type":"user","message":{"role":"user","content":"hello"}}"#).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation with no register"
    );
}

#[test]
fn missing_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("does_not_exist.jsonl");

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook must exit 0 on missing transcript");
    assert!(
        !log.exists() || std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "expected no famp invocation for missing transcript"
    );
}

#[test]
fn malformed_transcript_is_noop() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");
    std::fs::write(&transcript, "not json at all\n{broken\n").unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success(), "hook must exit 0 on malformed transcript");
}

#[test]
fn last_registration_wins_when_multiple_in_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("famp.log");
    let xdg = dir.path().join("xdg");
    stage_mock_famp(&dir.path().join("bin"), &log);
    let transcript = dir.path().join("t.jsonl");

    // First register as "alice" (listen:true, ok), then re-register as "dk" (listen:true, ok)
    let body = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"mcp__famp__famp_register","input":{"identity":"alice","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","is_error":false,"content":[{"type":"text","text":"{\"active\":\"alice\"}"}]}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t2","name":"mcp__famp__famp_register","input":{"identity":"dk","listen":true}}]}}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t2","is_error":false,"content":[{"type":"text","text":"{\"active\":\"dk\"}"}]}]}}
"#;
    std::fs::write(&transcript, body).unwrap();

    let out = run_hook(
        &hook_path(),
        &transcript,
        &dir.path().join("bin"),
        &log,
        &xdg,
    );
    assert!(out.status.success());
    let argv = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        argv.contains("--as dk"),
        "expected last identity 'dk', got: {argv:?}"
    );
    assert!(
        !argv.contains("--as alice"),
        "must not use first identity 'alice': {argv:?}"
    );
}
RUST
```

- [ ] **Step 2: Add `dirs` dependency if not present** (needed for `dirs::home_dir()`)

```bash
grep -q '"dirs"' crates/famp/Cargo.toml || \
  cargo add dirs --dev -p famp 2>&1 | tail -3
```

- [ ] **Step 3: Run the tests and confirm they fail for the right reason**

```bash
cargo nextest run -p famp hook_runner_await 2>&1 | tail -30
```

Expected: `listen_true_and_successful_register_enters_listen_mode` and `last_registration_wins_when_multiple_in_transcript` FAIL (no `famp await --as` invoked yet). All "noop" tests PASS.

---

### Task 3: Rewrite `famp-await.sh` with transcript detection

**Files:**
- Modify: `~/.claude/hooks/famp-await.sh`

This is the core change. The rewrite replaces the sentinel gate and `FAMP_HOME` blocks with transcript-based identity extraction. Notification-only reason replaces full envelope injection.

- [ ] **Step 1: Overwrite `~/.claude/hooks/famp-await.sh`**

```bash
cat > ~/.claude/hooks/famp-await.sh << 'HOOK'
#!/usr/bin/env bash
# ~/.claude/hooks/famp-await.sh — FAMP inbound listen-mode Stop hook (v0.9)
#
# Activates when the session transcript contains a successful famp_register
# call with listen:true and no subsequent famp_leave. Blocks on
# `famp await --as <identity> --timeout 23h`. On message, emits a
# notification-only {"decision":"block","reason":"..."} so Claude calls
# famp_inbox to retrieve the content — peer bytes never touch `reason`.
#
# Exit 0 always (fail-open): never trap Claude in a session.
set -uo pipefail

# --- Read transcript_path from stdin BEFORE redirecting stdin -----------
STDIN_JSON="$(cat 2>/dev/null || true)"
TRANSCRIPT="$(printf '%s' "$STDIN_JSON" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("transcript_path",""))' \
    2>/dev/null || true)"

# Disconnect stdin now to avoid SIGPIPE during the long await block.
exec 0</dev/null

# --- Logging -----------------------------------------------------------
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/famp"
LOG_FILE="${FAMP_HOOK_LOG:-$STATE_DIR/await-hook.log}"
mkdir -p "$(dirname "$LOG_FILE")" 2>/dev/null || true
[ -L "$LOG_FILE" ] && LOG_FILE=/dev/null
log() { printf '[%s pid=%s] %s\n' "$(date -Iseconds)" "$$" "$*" >> "$LOG_FILE" 2>/dev/null || true; }
log "hook invoked"

# --- Transcript gate ---------------------------------------------------
if [ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ]; then
    log "no transcript_path; exiting no-op"
    exit 0
fi

# --- Extract identity from transcript ---------------------------------
# Looks for the last successful famp_register with listen:true that is
# not followed by a famp_leave. Returns empty string if none found.
ACTIVE_IDENTITY="$(python3 - "$TRANSCRIPT" <<'PY' 2>/dev/null || true
import json, sys

path = sys.argv[1]
regs   = []    # (line_pos, tool_use_id, identity)
results = {}   # tool_use_id -> ok (bool)
leaves  = []   # line positions of famp_leave tool_use blocks

pos = 0
with open(path) as f:
    for line in f:
        pos += 1
        try:
            ev = json.loads(line)
        except Exception:
            continue
        msg = ev.get("message") if isinstance(ev.get("message"), dict) else ev
        content = msg.get("content") or []
        if isinstance(content, str):
            continue
        for block in content:
            if not isinstance(block, dict):
                continue
            t    = block.get("type", "")
            name = str(block.get("name", ""))
            if t == "tool_use":
                if name.endswith("famp_register"):
                    inp = block.get("input") or {}
                    if inp.get("listen"):
                        ident = inp.get("identity") or inp.get("name", "")
                        uid   = block.get("id", "")
                        if ident and uid:
                            regs.append((pos, uid, ident))
                elif name.endswith("famp_leave"):
                    leaves.append(pos)
            elif t == "tool_result":
                uid = block.get("tool_use_id", "")
                results[uid] = not bool(block.get("is_error", False))

# Find the last listen registration that succeeded and wasn't followed by a leave.
active = ""
for reg_pos, uid, ident in reversed(regs):
    if not results.get(uid, False):
        continue
    if any(lp > reg_pos for lp in leaves):
        continue
    active = ident
    break

print(active)
PY
)"

if [ -z "$ACTIVE_IDENTITY" ]; then
    log "no listen registration in transcript; exiting no-op"
    exit 0
fi

# --- Validate identity (belt-and-suspenders after Python extraction) ---
if ! printf '%s' "$ACTIVE_IDENTITY" | grep -qE '^[A-Za-z0-9._-]{1,64}$'; then
    log "invalid identity from transcript: $ACTIVE_IDENTITY; exiting no-op"
    exit 0
fi

FAMP_BIN="$(command -v famp 2>/dev/null || echo "$HOME/.cargo/bin/famp")"
log "listen mode active: identity=$ACTIVE_IDENTITY bin=$FAMP_BIN"

# --- Block on inbox ---------------------------------------------------
ERR_FILE="$(mktemp "${TMPDIR:-/tmp}/famp-await-err.XXXXXX")" || ERR_FILE=""
if [ -n "$ERR_FILE" ]; then
    MSG=$("$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h 2>"$ERR_FILE")
    STATUS=$?
    ERR=$(cat "$ERR_FILE" 2>/dev/null || true)
    rm -f "$ERR_FILE"
else
    MSG=$("$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h 2>&1)
    STATUS=$?
    ERR="$MSG"
fi
log "await returned status=$STATUS msg_bytes=${#MSG}"
[ -n "$ERR" ] && log "stderr: $ERR"

# --- Backup received envelope -----------------------------------------
if [ -n "${MSG//[[:space:]]/}" ]; then
    BACKUP_DIR="$STATE_DIR/received"
    if mkdir -p "$BACKUP_DIR" 2>/dev/null; then
        TS=$(date +%Y%m%dT%H%M%S)
        printf '%s\n' "$MSG" > "$BACKUP_DIR/${TS}-$$.jsonl" 2>/dev/null \
            && log "envelope backed up: $BACKUP_DIR/${TS}-$$.jsonl"
    fi
fi

# --- Error / empty handling -------------------------------------------
if [ $STATUS -ne 0 ] && [ -z "${MSG//[[:space:]]/}" ]; then
    log "await error (no stdout); fail-open exit 0"
    exit 0
fi

if [ -z "${MSG//[[:space:]]/}" ]; then
    log "await timeout or empty; clean stop"
    exit 0
fi

# --- Extract sender for notification string ---------------------------
# Best-effort: parse `from` field from the envelope JSON.
SENDER="$(python3 -c "
import json, sys
try:
    env = json.loads(sys.argv[1])
    print(env.get('from', env.get('sender', 'unknown')))
except Exception:
    print('unknown')
" "$MSG" 2>/dev/null || echo "unknown")"

# --- Emit notification-only block decision ----------------------------
# SECURITY: peer-controlled envelope bytes are NOT included in reason.
# The agent calls famp_inbox to retrieve the actual content.
if ! command -v jq >/dev/null 2>&1; then
    log "jq not found; cannot emit block decision"
    exit 0
fi

REASON="New FAMP message from ${SENDER}. Call famp_inbox to read it."
OUT=$(jq -n --arg r "$REASON" '{decision: "block", reason: $r}')
log "emitting block decision (${#OUT} bytes); sender=$SENDER"
printf '%s\n' "$OUT"
HOOK
chmod +x ~/.claude/hooks/famp-await.sh
```

- [ ] **Step 2: Verify the hook is executable and has no obvious syntax errors**

```bash
bash -n ~/.claude/hooks/famp-await.sh && echo "syntax ok"
```
Expected: `syntax ok`

- [ ] **Step 3: Run the Task 2 tests — they should now pass**

```bash
cargo nextest run -p famp hook_runner_await 2>&1 | tail -20
```
Expected: all 8 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/famp/tests/hook_runner_await.rs
git commit -m "test(listen-mode): add hook_runner_await transcript extraction tests"

git commit -am "feat(listen-mode): rewrite famp-await.sh — transcript detection + notification-only reason

- Replace sentinel gate with Python transcript parse for famp_register(listen:true)
- Parse tool_result to confirm registration success (not just tool_use call)
- Respect famp_leave ordering: register→leave→stop = no-op
- Replace FAMP_HOME with famp await --as <identity> (v0.9 proxy model)
- Reason field is notification-only; peer bytes go to backup only
- Identity validated against [A-Za-z0-9._-]{1,64} before subprocess call"
```

Note: `~/.claude/hooks/famp-await.sh` is not tracked in the repo. Commit only the test file above; the hook update is a local file change.

---

### Task 4: Write shell hook behavior tests (notification-only reason assertion)

**Files:**
- Modify: `crates/famp/tests/hook_runner_await.rs`

Add tests that assert the `reason` field in stdout is notification-only (no envelope bytes) and that the block decision JSON is valid.

- [ ] **Step 1: Append these tests to `crates/famp/tests/hook_runner_await.rs`**

```rust
#[test]
fn block_decision_is_notification_only_no_envelope_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that prints a fake envelope when called with `await`
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        r#"#!/usr/bin/env bash
if [[ "$*" == *"await"* ]]; then
    printf '{"from":"alice","body":{"details":{"summary":"SECRET_PAYLOAD"}}}\n'
fi
exit 0
"#,
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "bob", true, true, false);
    let log = dir.path().join("famp.log");

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stop_json.as_bytes()).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Must be valid JSON with decision=block
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}\nstdout={stdout:?}"));
    assert_eq!(v["decision"], "block", "stdout: {stdout}");

    // Peer-controlled content must NOT appear in reason
    let reason = v["reason"].as_str().unwrap_or("");
    assert!(
        !reason.contains("SECRET_PAYLOAD"),
        "peer bytes leaked into reason field: {reason:?}"
    );

    // Reason must mention famp_inbox
    assert!(
        reason.contains("famp_inbox"),
        "reason must direct agent to call famp_inbox: {reason:?}"
    );
}

#[test]
fn timeout_exits_zero_with_no_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that exits 0 with no output (simulates timeout)
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(&famp, "#!/usr/bin/env bash\nexit 0\n").unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);
    let log = dir.path().join("famp.log");

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stop_json.as_bytes()).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(out.status.success(), "must exit 0 on timeout");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.trim().is_empty(), "no stdout expected on timeout: {stdout:?}");
}

#[test]
fn broker_error_fails_open_exit_zero() {
    let dir = tempfile::tempdir().unwrap();
    let xdg = dir.path().join("xdg");
    let bin_dir = dir.path().join("bin");

    // Mock famp that exits non-zero with no stdout (broker unreachable)
    std::fs::create_dir_all(&bin_dir).unwrap();
    let famp = bin_dir.join("famp");
    std::fs::write(
        &famp,
        "#!/usr/bin/env bash\nprintf 'broker unreachable' >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&famp, std::fs::Permissions::from_mode(0o755)).unwrap();

    let transcript = dir.path().join("t.jsonl");
    make_transcript(&transcript, "dk", true, true, false);

    let stop_json = format!(
        r#"{{"transcript_path":"{}","hook_event_name":"Stop"}}"#,
        transcript.display()
    );
    let host_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{host_path}", bin_dir.display());
    let mut child = Command::new("bash")
        .arg(hook_path())
        .env("PATH", &new_path)
        .env("XDG_STATE_HOME", &xdg)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stop_json.as_bytes()).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();

    assert!(out.status.success(), "must fail-open (exit 0) on broker error");
}
```

- [ ] **Step 2: Run all hook_runner_await tests**

```bash
cargo nextest run -p famp hook_runner_await 2>&1 | tail -20
```
Expected: all 11 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/famp/tests/hook_runner_await.rs
git commit -m "test(listen-mode): add notification-only, timeout, and fail-open behavior tests"
```

---

### Task 5: Fill `await_timeout.rs` stub

**Files:**
- Modify: `crates/famp/tests/await_timeout.rs`

Replaces the placeholder with a real broker-backed test confirming `famp await --as <name>` returns `{"timeout":true}` when no message is sent before the deadline.

- [ ] **Step 1: Replace the stub**

```bash
cat > crates/famp/tests/await_timeout.rs << 'RUST'
#![cfg(unix)]
#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

//! CLI-05 (v0.9): `famp await --as <name>` returns {"timeout":true} when
//! no message arrives before the deadline. Uses a real broker subprocess
//! spawned via BusClient's spawn-on-demand path.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use serde_json::{json, Value};

struct McpProc {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

impl McpProc {
    fn spawn(sock: &std::path::Path) -> Self {
        let mut child = Command::cargo_bin("famp")
            .unwrap()
            .args(["mcp"])
            .env("FAMP_BUS_SOCKET", sock)
            .env_remove("FAMP_HOME")
            .env_remove("FAMP_LOCAL_ROOT")
            .env_remove("FAMP_LOCAL_IDENTITY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut p = Self { child, stdin, stdout, next_id: 0 };
        let _ = p.rpc("initialize", &json!({}));
        p
    }

    fn rpc(&mut self, method: &str, params: &Value) -> Value {
        self.next_id += 1;
        let req = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": method,
            "params": params,
        }))
        .unwrap();
        writeln!(self.stdin, "{req}").unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or(Value::Null)
    }

    fn tool_call(&mut self, name: &str, args: &Value) -> Value {
        self.rpc("tools/call", &json!({ "name": name, "arguments": args }))
    }
}

impl Drop for McpProc {
    fn drop(&mut self) { let _ = self.child.kill(); }
}

#[test]
fn await_returns_timeout_when_no_message_arrives() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let mut proc = McpProc::spawn(&sock);

    // Register with listen:false so we can call famp_await directly
    // without the Stop hook interfering.
    let reg = proc.tool_call("famp_register", &json!({ "name": "waiter" }));
    let reg_body = &reg["result"]["content"][0]["text"];
    let reg_val: Value = serde_json::from_str(reg_body.as_str().unwrap_or("{}")).unwrap();
    assert_eq!(reg_val["active"], "waiter", "register failed: {reg}");

    // famp_await with 2s timeout — no sender, so it must time out.
    let start = Instant::now();
    let await_resp = proc.tool_call("famp_await", &json!({ "timeout_seconds": 2 }));
    let elapsed = start.elapsed();

    let body_str = await_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("{}");
    let body: Value = serde_json::from_str(body_str).unwrap_or(Value::Null);

    assert_eq!(
        body["timeout"], true,
        "expected timeout:true, got: {body}"
    );
    assert!(
        elapsed >= Duration::from_secs(1),
        "await returned too quickly ({elapsed:?}); should have blocked"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "await took too long ({elapsed:?})"
    );
}
RUST
```

- [ ] **Step 2: Run the new test**

```bash
cargo nextest run -p famp await_timeout 2>&1 | tail -15
```
Expected: `await_returns_timeout_when_no_message_arrives` PASS (takes ~2s).

- [ ] **Step 3: Commit**

```bash
git add crates/famp/tests/await_timeout.rs
git commit -m "test(listen-mode): fill await_timeout stub with real broker-backed test"
```

---

### Task 6: Extend E2E test with listen loop scenario

**Files:**
- Modify: `crates/famp/tests/mcp_bus_e2e.rs`

Adds a second test function verifying the full notification path: alice sends to bob, bob's `famp_await --as bob` unblocks, and the response body contains a sender reference (not envelope bytes — the listener calls `famp_inbox` in real use, but here we assert the wake signal arrived).

- [ ] **Step 1: Append to `crates/famp/tests/mcp_bus_e2e.rs`**

Add this after the existing `test_mcp_bus_e2e` function:

```rust
/// TEST-06 — listen mode notification path.
///
/// Verifies that when alice sends to bob and bob is parked on
/// `famp_await --as bob`, the await unblocks and returns an envelope
/// with a non-empty `from` field. The hook then emits a notification;
/// the agent would call `famp_inbox` to retrieve content. Here we just
/// assert the envelope arrived with correct routing metadata.
#[test]
fn test_listen_mode_await_unblocks_on_send() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sock = tmp.path().join("listen-test-bus.sock");

    let mut alice = McpHarness::spawn(&sock, "alice-listen");
    let mut bob = McpHarness::spawn(&sock, "bob-listen");

    // Register both (listen:true is a hook hint, ignored by the MCP tool itself)
    let reg_a = alice.tool_call("famp_register", &json!({ "name": "alice-listen" }));
    McpHarness::ok_result(&reg_a, "alice-listen register");

    let reg_b = bob.tool_call("famp_register", &json!({ "name": "bob-listen", "listen": true }));
    let bob_body = McpHarness::ok_result(&reg_b, "bob-listen register");
    assert_eq!(bob_body["active"], "bob-listen");

    // Bob parks await before alice sends
    let bob_await_id = bob.next_id();
    bob.send_msg(&json!({
        "jsonrpc": "2.0",
        "id": bob_await_id,
        "method": "tools/call",
        "params": {
            "name": "famp_await",
            "arguments": { "timeout_seconds": 10 },
        },
    }));
    std::thread::sleep(Duration::from_millis(500));

    // Alice sends
    let send = alice.tool_call(
        "famp_send",
        &json!({
            "peer": "bob-listen",
            "mode": "new_task",
            "title": "Listen mode test message",
        }),
    );
    McpHarness::ok_result(&send, "alice send");

    // Read bob's await response
    let bob_await_resp: Value = {
        let mut line = String::new();
        bob.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };

    let envelope_str = bob_await_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("{}");
    let envelope: Value = serde_json::from_str(envelope_str).unwrap_or(Value::Null);

    // The envelope must have arrived (not timeout)
    assert!(
        envelope.get("timeout").is_none() || envelope["timeout"] != true,
        "expected envelope, got timeout: {envelope}"
    );

    // Must have a `from` field (hook uses this for the notification string)
    assert!(
        envelope.get("from").is_some() || envelope.get("envelope").is_some(),
        "envelope missing routing metadata: {envelope}"
    );
}
```

Also add `use std::time::Duration;` to the imports at the top if not already present.

- [ ] **Step 2: Run the E2E tests**

```bash
cargo nextest run -p famp mcp_bus_e2e 2>&1 | tail -20
```
Expected: both `test_mcp_bus_e2e` and `test_listen_mode_await_unblocks_on_send` PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/famp/tests/mcp_bus_e2e.rs
git commit -m "test(listen-mode): add E2E listen mode await-unblocks-on-send scenario"
```

---

### Task 7: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add a Listen Mode section to CLAUDE.md**

Find the `## Architecture` section header and insert the following block immediately before it:

```markdown
## Listen Mode

Agents can opt into automatic message wake-up by registering with `listen: true`:

```
famp_register({identity: "dk", listen: true})
```

When registered with `listen: true`, the Stop hook (`~/.claude/hooks/famp-await.sh`) blocks after each turn waiting for an inbound FAMP message (up to 23h). When a message arrives, Claude wakes automatically and receives: `"New FAMP message from <sender>. Call famp_inbox to read it."` — then calls `famp_inbox` to retrieve the content.

**Default (`listen: false`):** The window registers but stays idle between turns. Check inbox on demand by prompting the agent: "check your famp messages" → agent calls `famp_inbox`.

**When to use listen mode:** Dedicated agent windows that need sub-minute response to peer messages (e.g., Sofer's 5-agent mesh). General-purpose dev windows should omit `listen` or pass `false`.
```

- [ ] **Step 2: Verify CLAUDE.md renders cleanly**

```bash
grep -A 20 "## Listen Mode" CLAUDE.md
```
Expected: the new section appears cleanly with no merge artifacts.

- [ ] **Step 3: Run the full test suite to confirm nothing regressed**

```bash
cargo nextest run -p famp 2>&1 | tail -10
```
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(listen-mode): document listen:true registration and on-demand inbox check"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|-----------------|------|
| `listen: bool` on `famp_register` (default false) | Task 1 |
| Transcript detection replaces sentinel gate | Task 3 |
| `famp_leave` → no-op | Task 3 (hook); Task 2 (test) |
| Parse result, not just call | Task 3 (hook); Task 2 (test) |
| `--as <identity>` replaces `FAMP_HOME` | Task 3 |
| Notification-only reason | Task 3 (hook); Task 4 (test) |
| Fail-open on broker error | Task 3 (hook); Task 4 (test) |
| Timeout exits 0 cleanly | Task 3 (hook); Task 5 (test) |
| E2E listen loop | Task 6 |
| `await_timeout.rs` stub filled | Task 5 |
| CLAUDE.md updated | Task 7 |
| Identity regex with length cap | Task 3 (hook validates `{1,64}`) |

**Placeholder scan:** None found.

**Type consistency:** `make_transcript` helper used consistently across Tasks 2 and 4. `McpHarness` struct unchanged from existing tests; new test in Task 6 follows identical pattern.
