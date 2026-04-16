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

## Still TODO

### Onboarding UX
Current setup is manual and tedious. Need easier onboarding:
- Option A: Bootstrap prompt (markdown instructions)
- Option B: `/famp-register` skill (Claude Code native)
- Option C: `famp setup` CLI wizard

See prompt in session for tackling this.

### Known Issues
- Daemons must be started manually (`famp listen`)
- Peer exchange requires manual copy/paste of pubkeys
- No auto-discovery of local agents

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
