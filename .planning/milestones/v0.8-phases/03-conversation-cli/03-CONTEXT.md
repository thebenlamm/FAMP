# Phase 3: Conversation CLI - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous --from 2)

<domain>
## Phase Boundary

A developer can open a task, exchange multiple `deliver` messages within it across two terminal sessions, and close it with a terminal deliver — all through CLI commands — with task state persisted to disk and surviving daemon restarts.

Concretely this phase delivers:
- `famp send` — signs + POSTs an envelope to a peer (new task or existing task), recording local task state
- `famp await` — blocks on the inbox cursor until a new line arrives or a timeout elapses; emits structured output
- `famp inbox` — lists, reads, or acknowledges inbox entries (advance cursor)
- `famp peer add` — registers a peer principal + HTTPS endpoint + verifying key into `peers.toml`
- Persistent task records in `~/.famp/tasks/<uuid>.toml`
- Cursor file `~/.famp/inbox.cursor` (0600, atomic replace) — separates "received" from "read"
- Restart-safe: task records and cursor survive `famp listen` restarts

Out of scope (defer to Phase 4):
- MCP server surface (`famp mcp`)
- Live two-session E2E example
- Slash-command orchestration from Claude Code

</domain>

<decisions>
## Implementation Decisions

### Subcommand Surface
- `famp send` — flags: `--to <peer-alias>`, one of `{--new-task "<text>" | --task <id>}`, optional `--terminal`, `--body <text>`. Exit codes: 0 on HTTP 2xx, non-zero with typed error on anything else.
- `famp await` — flags: `--timeout <duration>` (default 30s), `--task <id>` filter (optional). Prints one JSON line per received entry to stdout; exits 0 after printing one entry; exits typed "timeout" non-zero code on expiry; loops only if `--all` is passed (defer `--all` to later if not needed).
- `famp inbox` — subcommands: `famp inbox list [--since <cursor>]`, `famp inbox ack <line-number-or-cursor>`. Non-interactive.
- `famp peer add <alias> --endpoint <https-url> --pubkey <b64>` — validates base64, pubkey length, URL scheme, and writes to `peers.toml` atomically.

### Task Records
- Path: `~/.famp/tasks/<uuid>.toml` (one file per task; uuid is the envelope's `task_id` field).
- Schema (TOML):
  ```toml
  task_id = "01913..."           # UUIDv7, matches the envelope task_id
  state = "REQUESTED"            # famp-fsm state name
  peer = "alice"                 # peer alias from peers.toml
  opened_at = "2026-04-14T..."   # RFC 3339
  last_send_at = "..."
  last_recv_at = "..."
  terminal = false
  ```
- Writer: atomic replace via tempfile in `~/.famp/tasks/` → fsync → rename. Same pattern Phase 1 used for config.toml.
- FSM advancement: each `send` or `await` that produces a terminal envelope calls the v0.7 `famp-fsm` engine with the appropriate transition; the returned state is written to disk.
- Records survive daemon restart because they're plain TOML files; the daemon never touches them.

### Inbox Cursor
- Path: `~/.famp/inbox.cursor`, 0600.
- Format: single line `byte_offset\n` — the next byte to read in `inbox.jsonl`.
- Write strategy: write to `inbox.cursor.tmp`, fsync, rename — atomic replace, same invariant as task records.
- `famp await` reads the cursor, opens `inbox.jsonl`, seeks, and blocks via inotify/kqueue OR poll-fallback (tokio `Notify` + periodic re-read every 100ms). Prefer poll for Phase 3 — simpler and cross-platform enough for personal runtime.
- On receiving a line: parse as raw envelope bytes, extract `task_id` / `from` / `message_class`, advance cursor, print structured output, exit.
- `famp inbox ack` moves the cursor without printing — for "drop this entry, don't block on it again."

### Library Crate: `famp-taskdir`
- New crate: `crates/famp-taskdir/` — pure storage primitive, no network.
- Public API: `TaskDir::open(path)`, `read(task_id)`, `create(task_id, peer)`, `update(task_id, |rec| …)`, `list() -> impl Iterator`.
- Uses `serde` + `toml` (already in workspace).
- Atomic replace helpers reused from famp-inbox-style patterns (or lifted into a shared `famp-atomic` crate — defer to Plan 03-01 decision).
- Errors: `TaskDirError` narrow thiserror enum.

### Library Crate: `famp-cursor`
- Could merge into famp-inbox OR live as its own crate. Decision: **add cursor to `famp-inbox`** as a sibling type `InboxCursor::{read, advance}`. Keeps cursor and jsonl together since they're tightly coupled.

### Peer Registry
- Phase 1 shipped `peers.toml` as an empty file. Phase 3 adds the read/write path.
- Extend `crates/famp/src/cli/config.rs` `Peers` type with `PeerEntry { alias, endpoint, pubkey_b64 }`.
- `famp peer add` loads, appends, writes atomically. Duplicate alias → typed error.
- `famp send --to <alias>` looks up the peer in this registry.

### Send Path
- `famp send` builds a `SignedEnvelope` via famp-envelope: fills `from`, `to`, `task_id`, `message_class`, `body`, signs with the loaded signing key.
- POST over HTTPS via reqwest with cert-pinning against the peer's TLS cert fingerprint. For Phase 3, use **TOFU**: first contact accepts any cert; subsequent contacts pin the fingerprint into `peers.toml` under `tls_fingerprint_sha256`. Mirror the v0.7 keyring TOFU pattern.
- If the send fails (network, 4xx, 5xx), the task record is NOT created — exit non-zero with the error.

### Error Types
- New `CliError` variants: `Send(SendError)`, `Await(AwaitError)`, `PeerNotFound { alias }`, `PeerDuplicate { alias }`, `TaskNotFound { task_id }`, `TaskTerminal { task_id }`, `AwaitTimeout`.
- `famp-taskdir` has its own `TaskDirError`.
- `famp-inbox` grows `CursorError`.

### Claude's Discretion
- Exact JSON output shape for `famp await` (pick one, document it, lock it with a test)
- Duration parser (`humantime` vs manual `30s`/`5m` regex — pick one)
- Whether `famp send` retries on transient network errors (default: no retry, fail fast)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 1 `load_identity`, `IdentityLayout`, `Config`, `Peers` types — direct consumers
- Phase 2 `famp-inbox::Inbox::read_all` (already tail-tolerant) — reused by `famp inbox list` and `famp await`
- v0.7 `famp-envelope` — envelope construction, signing, canonicalization
- v0.7 `famp-fsm` — `TaskFsm::step` for terminal deliver handling
- v0.7 `famp-keyring` — pubkey storage; reused for peer verification keys
- v0.7 `famp-transport-http::client` (if present) — cert-pinned reqwest pattern

### Established Patterns
- Narrow thiserror enums per crate (Phase 1 D-16 pattern, extended in Phase 2)
- Atomic replace for all config writes (Phase 1 atomic.rs)
- Integration tests one per success criterion, named after the criterion
- cargo nextest, clippy -D warnings, no openssl

### Integration Points
- `crates/famp/src/cli/mod.rs` — dispatcher gains Send, Await, Inbox, PeerAdd variants
- `crates/famp/src/bin/famp.rs` — clap subcommand definitions
- `crates/famp-inbox/` — cursor type added alongside append/read
- Root Cargo.toml — `crates/famp-taskdir` added to workspace members

</code_context>

<specifics>
## Specific Ideas

- Test: open a task, send 3 deliver messages within it (non-terminal), confirm the task stays in its non-terminal state between sends
- Test: send terminal, assert state transitions via FSM, assert subsequent send on same task exits non-zero with TaskTerminal
- Test: restart `famp listen` between send and await, confirm task record and inbox cursor survive
- Test: `famp peer add` with duplicate alias exits non-zero, does not corrupt `peers.toml`

</specifics>

<deferred>
## Deferred Ideas

- Multi-task `famp await --all` streaming loop — Phase 4 if MCP needs it
- Inbox rotation / retention — never for personal runtime
- Agent Cards (peer discovery beyond TOFU) — Federation Profile v0.9+
- Windows signal handling — out of scope

</deferred>
