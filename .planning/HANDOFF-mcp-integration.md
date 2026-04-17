# FAMP MCP Integration - Session Handoff

**Date:** 2026-04-16
**Commits:** 41044e2, 0378d75, 2d6b403

## What Was Done

Successfully debugged and fixed FAMP MCP server integration with Claude Code. Four bugs were identified and fixed:

### 1. Wire Format Mismatch (CRITICAL)
- **Problem:** Server used LSP-style Content-Length framing, but Claude Code sends newline-delimited JSON (NDJSON)
- **Fix:** Changed `read_msg`/`write_msg` in `server.rs` to use NDJSON
- **Files:** `crates/famp/src/cli/mcp/server.rs`

### 2. Tool Descriptions Unclear
- **Problem:** Bob (receiving agent) didn't know to use `task_id` from inbox when replying
- **Fix:** Improved tool descriptions to explicitly state workflow
- **Files:** `crates/famp/src/cli/mcp/server.rs` (tool_descriptors)

### 3. Inbox task_id Empty
- **Problem:** Inbox showed `task_id: ""` because code looked for wrong field
- **Fix:** Extract from `id` for requests, `causality.ref` for delivers/commits
- **Files:** `crates/famp/src/cli/inbox/list.rs`

### 4. Task Record Not Found
- **Problem:** When replying to received task, no local record existed
- **Fix:** Create record on-demand when replying to received tasks
- **Files:** `crates/famp/src/cli/send/mod.rs`

### 5. FSM State Inconsistency (follow-up)
- **Problem:** Created received-task records in REQUESTED state (should be COMMITTED)
- **Fix:** Added `TaskRecord::new_committed()`, use for received tasks
- **Files:** `crates/famp-taskdir/src/record.rs`, `crates/famp/src/cli/send/mod.rs`

## Testing Done

- Two Claude Code windows (Alice + Bob) successfully exchanged messages
- All 4 MCP integration tests pass
- E2E flow verified: new_task → commit → deliver → terminal

## Onboarding UX (DONE)

Three new CLI commands simplify setup:

### `famp setup` — One-command onboarding
```bash
famp setup --name alice --home /tmp/famp-alice --port 8450
```
- Creates FAMP_HOME if needed
- Generates Ed25519 identity + TLS cert
- Auto-selects available port (or uses --port)
- Sets principal to `agent:localhost/<name>`
- Outputs peer card JSON for sharing

### `famp info` — Output peer card
```bash
FAMP_HOME=/tmp/famp-alice famp info
```
Outputs the current agent's peer card (JSON by default).

### `famp peer import` — Import peer card
```bash
echo '<peer-card-json>' | FAMP_HOME=/tmp/famp-alice famp peer import
# or
famp peer import --card '<peer-card-json>'
```

### Claude Code Skill
`/famp-setup` skill in `.claude/skills/famp-setup/SKILL.md` guides users through setup.

## Quick Start (New Flow)

```bash
# 1. Setup two agents
famp setup --name alice --home /tmp/famp-alice --port 8450
famp setup --name bob --home /tmp/famp-bob --port 8451

# 2. Exchange peer cards
FAMP_HOME=/tmp/famp-alice famp info | FAMP_HOME=/tmp/famp-bob famp peer import
FAMP_HOME=/tmp/famp-bob famp info | FAMP_HOME=/tmp/famp-alice famp peer import

# 3. Start daemons
FAMP_HOME=/tmp/famp-alice famp listen &
FAMP_HOME=/tmp/famp-bob famp listen &

# 4. First send — opt in to TOFU bootstrap
#    First contact has no pinned TLS leaf hash for the peer, so it fails
#    closed by default. Set FAMP_TOFU_BOOTSTRAP=1 for the very first send;
#    after success the leaf hash is pinned in peers.toml and subsequent
#    sends do not need the flag.
FAMP_TOFU_BOOTSTRAP=1 FAMP_HOME=/tmp/famp-alice famp send \
  --to bob --new-task "hello"

# 5. Configure MCP (.mcp.json)
# See "MCP Configuration" section below.
# For MCP, set "FAMP_TOFU_BOOTSTRAP": "1" in env until the first send to
# each peer succeeds, then remove it.
```

## Known Issues
- Daemons must be started manually (`famp listen`)
- No auto-discovery of local agents
- First contact requires explicit TOFU opt-in (`FAMP_TOFU_BOOTSTRAP=1`).
  The error kind `tofu_bootstrap_refused` surfaces if a send is attempted
  against an unpinned peer without the env var.

## MCP Configuration

Working config (`.mcp.json` in project root):
```json
{
  "mcpServers": {
    "famp-alice": {
      "command": "/path/to/famp",
      "args": ["mcp"],
      "env": { "FAMP_HOME": "/path/to/alice-home" }
    }
  }
}
```

## Key Files

| File | Purpose |
|------|---------|
| `crates/famp/src/cli/mcp/server.rs` | MCP stdio server, NDJSON framing |
| `crates/famp/src/cli/mcp/tools/*.rs` | Tool implementations |
| `crates/famp/src/cli/inbox/list.rs` | Inbox listing with task_id extraction |
| `crates/famp/src/cli/send/mod.rs` | Send logic with on-demand record creation |
| `crates/famp-taskdir/src/record.rs` | TaskRecord with new_committed() |
