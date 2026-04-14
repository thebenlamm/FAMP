# Phase 2: Daemon & Inbox - Context

**Gathered:** 2026-04-14
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous --from 2)

<domain>
## Phase Boundary

A running `famp listen` process accepts inbound signed messages over HTTPS, persists each one durably to a JSONL inbox, and shuts down cleanly — all without any change to the v0.7 wire protocol or transport code.

Concretely this phase delivers:
- `famp listen` subcommand wired into the CLI dispatcher from Phase 1
- Durable JSONL inbox at `~/.famp/inbox.jsonl` with per-line fsync before the HTTP 200
- Single-process bind guard (second `famp listen` on same port fails fast with a typed error)
- Graceful shutdown on SIGINT/SIGTERM with flushed writes and exit 0
- Read path that tolerates a truncated trailing JSONL line (mid-write crash recovery)

Out of scope (defer to later phases):
- Task record creation, FSM progression, or any business logic beyond persistence (Phase 3)
- Authenticated outbound send / peer management (Phase 3)
- MCP server surface (Phase 4)

</domain>

<decisions>
## Implementation Decisions

### Daemon Surface
- New binary target is NOT required — `famp listen` is a subcommand on the existing `famp` bin.
- Daemon reuses v0.7's `famp-transport-http` `Server` builder unmodified. No transport code changes.
- Bind address comes from `config.toml` `listen_addr` (default `127.0.0.1:8443`, set in Phase 1). CLI `--listen` flag overrides.
- On start, daemon prints the bound address to **stderr** (one line, `listening on https://127.0.0.1:8443`); stdout stays reserved for structured data.
- Identity (signing key + TLS cert+key) is loaded via Phase 1's `load_identity` helper. No re-scaffolding.

### Inbox Storage
- Format: newline-delimited JSON (JSONL), UTF-8, one envelope per line.
- Canonical filename: `${FAMP_HOME}/inbox.jsonl` — created with 0600 mode if absent.
- Each inbound envelope is serialized with `serde_json::to_string` (compact, no pretty-print) then written as `{line}\n`.
- Durability contract: `write_all` → `sync_data` (fsync) → HTTP handler returns 200. The 200 is the durability receipt. No buffered writer.
- Writer is a single `tokio::sync::Mutex<File>` shared via `Arc` so concurrent HTTP handlers serialize at the append point. File opened once with `OpenOptions::append(true)`.
- No rotation, no compaction, no index file in this phase. File grows unbounded — acceptable for personal-runtime scope.

### Bind Guard
- Attempt `TcpListener::bind` at startup. On `AddrInUse`, exit non-zero with `CliError::PortInUse { addr }` — human-readable "another famp listen is already bound to 127.0.0.1:8443".
- No PID file, no lock file — the OS-level bind is the lock. Simpler and race-free across SIGKILL scenarios.

### Graceful Shutdown
- Use `tokio::signal::unix` for SIGINT + SIGTERM. Windows is out of scope for v0.8.
- Wire shutdown via `axum::serve(...).with_graceful_shutdown(signal_future)`.
- `signal_future` resolves on first of `ctrl_c()` / `signal(SIGTERM)`. After it fires: server stops accepting, in-flight requests finish (they're already fsynced pre-200), binary exits 0.
- Shutdown deadline: rely on axum default (no explicit timeout). Inbox writes are synchronous + fast; a stuck handler is a bug, not a feature.

### Crash Recovery Read Path
- A truncated trailing line (no final `\n`) must not poison subsequent reads.
- Reader implementation: read file line-by-line; for each line, attempt `serde_json::from_str`; on the LAST line only, tolerate a parse failure (log warning, skip). Any non-terminal parse failure is still an error — mid-file corruption is not acceptable.
- This read path is a library helper in `famp-inbox` (new crate) exposed to Phase 3's `famp await` and `famp inbox` consumers.

### Error Types
- `famp-inbox` crate ships its own `InboxError` via `thiserror` — narrow enum (`Io`, `Serialize`, `TruncatedTailLine`).
- `famp` CLI wraps it through `CliError::Inbox(InboxError)` (new variant).
- `PortInUse { addr: SocketAddr }` is a new `CliError` variant.

### Crate Layout
- New crate: `crates/famp-inbox/` — pure library, no binary, no network.
  - Public surface: `Inbox::open(path)`, `Inbox::append(&self, &SignedEnvelope)`, `Inbox::read_all(path) -> impl Iterator`, `InboxError`.
  - Depends on `famp-envelope` (for the envelope type), `serde_json`, `thiserror`, `tokio` (with `fs` + `sync` features).
- Listen command lives in `crates/famp/src/cli/listen/mod.rs` and wires inbox + transport together.
- The axum handler from `famp-transport-http` stays unchanged — `famp listen` passes a closure that does `inbox.append(env).await` inside the existing dispatch point.

### Claude's Discretion
- Exact shutdown log message format
- Unit test internal helper names
- Whether `listen_addr` override flag is `--listen`, `--bind`, or `--addr` (pick one, stay consistent)
- Whether to fsync the file descriptor or parent directory too (fsync fd is sufficient given we only append, never rename)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp-transport-http` v0.7: `Server` builder, axum routing, rustls TLS, signed-envelope verification middleware — all reusable without modification.
- `famp-envelope` v0.7: `SignedEnvelope` type with serde derives — direct input to inbox append.
- Phase 1 `load_identity` (`crates/famp/src/cli/init/mod.rs`): returns `(SigningKey, TlsCertPair, Config)` for the listen command.
- Phase 1 `CliError`: base error enum — extend with `Inbox` and `PortInUse` variants.
- Phase 1 atomic file helpers (`crates/famp/src/cli/init/atomic.rs`): `write_secret` (0600) — reuse for initial inbox file creation.

### Established Patterns
- thiserror in libs, narrow error enums per crate (Phase 1 D-16 / Phase 2 01-01 pattern)
- tokio runtime (v0.7 HTTP transport already on tokio 1.x)
- No openssl — cargo tree -i openssl must stay empty (E2E-03 regression guard)
- cargo nextest for tests; each requirement maps to a named integration test file

### Integration Points
- `crates/famp/src/cli/mod.rs` — dispatcher registers `Listen` variant
- `crates/famp/src/bin/famp.rs` — clap subcommand definition
- `crates/famp/Cargo.toml` — pull in new `famp-inbox` workspace crate + `tokio` full features
- `Cargo.toml` workspace — add `crates/famp-inbox` to `members`

</code_context>

<specifics>
## Specific Ideas

- Test plan must include an adversarial case: write a half-written JSONL line, then call the read path, assert it returns the preceding lines successfully and surfaces exactly one "truncated tail line" warning.
- A bind-collision integration test: start two `famp listen` processes in a test, assert the second exits non-zero with the typed error.
- Durability test: spawn `famp listen` as a child process, send one signed envelope via reqwest, hard-kill the child (`SIGKILL`) immediately after the 200, assert the line is present on disk.

</specifics>

<deferred>
## Deferred Ideas

- Inbox rotation / size-based truncation — Phase 3+ if needed
- Structured tracing / JSON logs — Phase 4 if MCP needs it
- Windows support — never, per v0.8 scope
- Multi-tenant inbox (one file per principal) — defer to Federation Profile if ever needed

</deferred>
