---
status: complete
---

# Quick Task 260708-tzk: Fix famp inspect identities unread-count bug narrowly

## What shipped

`famp_inbox` (MCP) now write-throughs its returned `next_offset` to the on-disk
`.{name}.cursor` file after every successful call, via the existing
`cursor_exec::execute_advance_cursor` helper (the same atomic writer the CLI
`famp inbox ack` path already used — no new write logic, no duplication).
The write is monotonic (`max(current_disk_cursor, next_offset)`) so a manual
`since: 0` full-replay never rewinds the disk cursor. `session::set_inbox_offset`
is unchanged (still stores the raw returned offset, not max — that's a
deliberately different behavior, since it must follow a broker clamp down on
mailbox shrink).

`famp inspect identities`'s `unread` (computed from the disk cursor in
`cli/broker/mod.rs::read_mailbox_meta_for`) no longer lags what `famp_inbox`
has actually delivered.

## Acceptance

`crates/famp/tests/inbox_unread_matches_delivered.rs` flipped RED → GREEN.
Test drives two send-then-drain rounds via two MCP `Harness` sessions sharing
one broker socket; asserts `unread` after round 2 equals the entry count the
next `famp_inbox` call returns.

## Gates

- `cargo fmt --check` — clean
- `cargo clippy -p famp --all-targets -- -D warnings` — clean
- `cargo test -p famp` — full suite green
- `just install` run (this changes the deployed MCP tool surface, per project
  convention in CLAUDE.md)

## Scope guardrails verified

`git diff --stat` confirms only `crates/famp/src/cli/mcp/tools/inbox.rs` and
the new test file changed in the code diff (plus the HANDOFF.md addendum,
docs-only). `cli/inbox/list.rs` (CLI `inbox list` still full-replays with no
persisted state), `await_offsets`, and `drain_walk.rs` are untouched — no
999.1/999.2/999.11 broker-owned-position machinery was touched.

## Commits

- `fda9de9` — fix(quick-260708-tzk): write-through MCP famp_inbox cursor to disk
- `1294004` — fix(quick-260708-tzk): satisfy cast_possible_truncation clippy pedantic lint (bundled with the HANDOFF.md re-park addendum)
- `74ff15b` — chore: merge executor worktree

## Follow-up filed

`.planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md` got a dated
addendum: the broader broker-owned-delivery-position redesign is re-parked
behind the federation spike (2026-07-01 decision), not abandoned. The full
design doc (`docs/superpowers/specs/2026-07-08-999-11-broker-owned-delivery-position-design.md`)
and three independent reviews' findings (most notably: the design doc doesn't
repoint the two cursor authorities the way this narrow fix just did, and its
"bounded hole-set" claim is unbounded in the exact starvation scenario it
targets) are preserved for whenever the spike fires and 999.11 is picked back
up.
