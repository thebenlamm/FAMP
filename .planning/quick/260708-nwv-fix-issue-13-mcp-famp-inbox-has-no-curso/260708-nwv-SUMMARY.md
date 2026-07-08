---
quick_id: 260708-nwv
description: Fix issue #13 — MCP famp_inbox has no cursor advance (double-print pattern)
date: 2026-07-08
status: complete
commits:
  - 89e2162 test(quick-260708-nwv): failing tests for inbox cursor advance
  - 9b519a5 fix(quick-260708-nwv): session-scoped inbox cursor advance (#13)
---

# 260708-nwv — MCP `famp_inbox` session-scoped cursor advance

Three tasks, executed in the mandated order (RED, GREEN, deploy). TDD: the two
new tests were committed failing (89e2162) — a compile-only `inbox_offset`
stub existed but was not yet wired into `set_active_identity`, `register.rs`,
or `tools/inbox.rs` — and go green in the same follow-up commit (9b519a5).

**Files changed:** `crates/famp/src/cli/mcp/session.rs`,
`crates/famp/src/cli/mcp/tools/inbox.rs`,
`crates/famp/src/cli/mcp/tools/register.rs`,
`crates/famp/tests/mcp_bus_e2e.rs`.

**What changed:**
- `SessionState` gained `inbox_offset: Option<u64>` with async
  `inbox_offset()`/`set_inbox_offset()` accessors, matching the existing
  `last_send` pattern.
- `tools::inbox::call`: when the caller supplies no `since`, the session's
  remembered offset is used as the effective `since`. An explicit caller
  `since` (including `0`) always wins and still updates the stored value —
  `since: 0` remains a deliberate full-replay escape hatch. After every
  successful call, the stored value is always overwritten with the
  *returned* `next_offset` (never `max(stored, returned)`), so it follows
  the broker's clamp down when a mailbox shrinks (#11, #16).
- `session::set_active_identity` resets `inbox_offset` to `None` on rebind.
  `register.rs`'s `RegisterOk` arm binds identity inline on its own held
  mutex guard and cannot call `set_active_identity` (would deadlock
  re-locking the same tokio `Mutex`), so it duplicates the reset next to its
  own `active_identity` assignment — this is the actual production rebind
  path.
- Doc comment on the tool records the accepted decision: the *first*
  `famp_inbox` of a session still replays the whole mailbox (register's
  `RegisterOk.drained` is discarded and can't seed the offset without a wire
  change), and this is a once-per-session cost / recovery affordance, not a
  bug — so the next person doesn't file it as one.

**No wire change.** No new `BusMessage`/`BusReply` field, no broker change, no
touch to the on-disk `.<name>.cursor`, no reintroduction of `action`/`offset`
to the `famp_inbox` schema.

**Verification:** RED confirmed both new tests failed pre-implementation for
the right reason. Green gate all passed: `cargo build --workspace
--all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p famp --lib` (176 passed), `cargo test -p famp --tests`,
`cargo test -p famp-bus` — all clean.
`crates/famp-bus/tests/prop04_drain_completeness.rs` and
`tdd02_drain_cursor_order.rs` are byte-identical at HEAD — no broker semantics
touched. No `cargo nextest`, no `--no-verify`.

**Deploy:** `cli/mcp/` changed, so `just install` was run —
`~/.cargo/bin/famp` is the fresh binary. No broker restart needed (no broker
code changed). Not pushed.
