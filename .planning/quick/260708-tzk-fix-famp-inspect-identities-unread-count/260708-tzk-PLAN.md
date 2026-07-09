---
phase: quick-260708-tzk
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp/src/cli/mcp/tools/inbox.rs
  - .planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md
autonomous: true
requirements: [FAMP-INBOX-CURSOR-SYNC]
must_haves:
  truths:
    - "After an MCP famp_inbox drain, `famp inspect identities` unread equals what the next famp_inbox call returns"
    - "The plain CLI `famp inbox list` still full-replays (since=0) with no persisted-state change"
  artifacts:
    - "crates/famp/src/cli/mcp/tools/inbox.rs write-through to the on-disk .{name}.cursor"
    - "crates/famp/tests/inbox_unread_matches_delivered.rs committed and GREEN"
    - ".planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md dated addendum"
  key_links:
    - "famp_inbox Ok arm -> cursor_exec::execute_advance_cursor (the same atomic write ack.rs reuses)"
---

<objective>
Fix the `famp inspect identities` unread-count drift narrowly: after every successful MCP
`famp_inbox` call, write-through the returned `next_offset` to the on-disk `.{name}.cursor`
file (monotonic `max(current_disk_cursor, next_offset)`), so the disk cursor — the single
source `read_mailbox_meta_for` uses to compute `unread` — stops lagging the MCP session cursor.

Purpose: `famp inspect identities`'s `unread` is computed in `cli/broker/mod.rs::read_mailbox_meta_for`
from the on-disk `.{name}.cursor`, which only `register`/`join`/CLI `inbox ack` advance. The MCP
`famp_inbox` tool tracks progress via a separate, never-persisted `session::inbox_offset`. Reading
via `famp_inbox` never advances the disk cursor, so `unread` silently drifts from what the agent saw.

Output: One-line-of-effect change in `inbox.rs` reusing the existing atomic cursor writer, the
acceptance test flipped RED->GREEN and committed, and a HANDOFF.md addendum re-parking the broader
redesign.

Scope guardrails (do NOT touch):
- The plain CLI `famp inbox list` (`cli/inbox/list.rs`) — it intentionally defaults to `since=0`
  (full replay) with no persisted state. Auto-advancing it would silently change CLI behavior for
  existing users. ONLY the MCP `famp_inbox` read path gets the write-through.
- `await_offsets`, `drain_walk.rs`, and any 999.1/999.2/999.11 broker-owned-position machinery.
</objective>

<execution_context>
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/workflows/execute-plan.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@crates/famp/src/cli/mcp/tools/inbox.rs
@crates/famp/src/cli/mcp/session.rs
@crates/famp/src/cli/inbox/ack.rs
@crates/famp/src/cli/broker/cursor_exec.rs
@crates/famp/tests/inbox_unread_matches_delivered.rs
@.planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write-through the MCP famp_inbox cursor to the on-disk .{name}.cursor file</name>
  <files>crates/famp/src/cli/mcp/tools/inbox.rs, crates/famp/tests/inbox_unread_matches_delivered.rs</files>
  <action>
In the `Ok(out)` arm of `call()` in `crates/famp/src/cli/mcp/tools/inbox.rs`, AFTER the existing
`session::set_inbox_offset(Some(out.next_offset)).await;` line, add an on-disk cursor write-through
so `famp inspect identities` (which reads the disk `.{name}.cursor`) stops lagging the MCP session
cursor.

REUSE the existing atomic cursor writer — do NOT duplicate the temp+rename+sync_all+chmod logic.
The shared helper already exists and is the SAME one the CLI `famp inbox ack` path uses (via
`cli/inbox/ack.rs::run_at_structured` -> `execute_advance_cursor`):
`crate::cli::broker::cursor_exec::execute_advance_cursor(bus_dir: &Path, display_name: &str, offset: u64)`.
It is already reusable as-is; no extraction needed. If for any reason it is not callable from this
module, extract a shared function rather than copy-pasting the atomic-write body.

Steps in the Ok arm:
1. Resolve the bound identity: `session::active_identity().await` (dispatch guarantees it is Some by
   this point — the tool already uses it to build `ListArgs.act_as`). Bind it to a local `name`.
2. Derive the bus directory from the socket: `crate::bus_client::bus_dir(&resolve_sock_path())`
   (`resolve_sock_path` is already imported; `bus_dir` is the same helper `ack.rs` uses).
3. Read the CURRENT on-disk cursor value for `name` and compute the monotonic target
   `max(current_disk_cursor, out.next_offset)`. NEVER regress the disk cursor — even when the caller
   passed an explicit `since` override for a manual replay (`since: 0` full-replay must not rewind
   the disk cursor). To read+parse the current value, mirror the parse pattern in
   `cli/broker/mod.rs::read_mailbox_meta_for` (path `<bus_dir>/mailboxes/.<name>.cursor`, body is a
   single ASCII decimal + newline); treat a missing/unparseable file as `0`.
   RATIONALE for `max` here (vs the session layer's deliberate non-max `set_inbox_offset`): the disk
   cursor is the `unread` floor read by the inspector and by `register`/`join`; it must advance
   monotonically and never rewind on a manual `since` replay. The session offset can follow a broker
   clamp DOWN when a mailbox shrinks; the disk cursor must not. Keep the two behaviors distinct and
   do not "unify" them.
4. Only write when the target strictly exceeds the current value (skip a no-op write).
5. Call `execute_advance_cursor(bus_dir, &name, target).await`. This side-effect is best-effort
   relative to the inbox READ, which has already succeeded: on `Err`, emit a `tracing::warn!` (or the
   crate's existing logging macro) including the identity, the target offset, and the io error — do
   NOT swallow it silently, and do NOT fail the tool call or discard the already-fetched `entries`.
   The read succeeded; the cursor sync is an observability write.

Do NOT change the `session::set_inbox_offset` line's semantics (it must stay `Some(out.next_offset)`,
NOT max — see the module doc comment #11/#16). Do NOT touch `cli/inbox/list.rs`, `await_offsets`, or
`drain_walk.rs`.

Then make the acceptance test at `crates/famp/tests/inbox_unread_matches_delivered.rs` go GREEN. It
is currently uncommitted and RED on HEAD. Do NOT weaken its assertions — the fix should flip it by
making both authorities track the same position. Commit the source fix and this test together.
  </action>
  <verify>
    <automated>cargo test -p famp --test inbox_unread_matches_delivered -- inspect_identities_unread_equals_subsequent_famp_inbox_delivered_count</automated>
  </verify>
  <done>
The named acceptance test passes (was RED on HEAD). `famp inspect identities` unread for `receiver`
equals the entry count a subsequent `famp_inbox` call returns across two send-then-drain rounds.
`session::set_inbox_offset` still stores the raw returned offset. `cli/inbox/list.rs`,
`await_offsets`, and `drain_walk.rs` are untouched.
  </done>
</task>

<task type="auto">
  <name>Task 2: Run full gates and add the 999.11 HANDOFF.md re-park addendum</name>
  <files>.planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md</files>
  <action>
First run the standard gates and confirm all pass:
- `cargo fmt --check`
- `cargo clippy -p famp --all-targets -- -D warnings`
- `cargo test -p famp`

Then append a NEW dated addendum section to the END of
`.planning/phases/999.11-broker-owned-delivery-position/HANDOFF.md` — do NOT delete or rewrite any
existing content. Use a heading like `## Addendum — 2026-07-08: unread divergence fixed narrowly;
redesign re-parked`. The addendum must state:

(a) The `unread`-vs-delivered divergence this HANDOFF diagnosed (§2/§6) is now fixed NARROWLY: the
    MCP `famp_inbox` read path write-throughs its returned `next_offset` to the on-disk `.{name}.cursor`
    via `cursor_exec::execute_advance_cursor` (monotonic `max`), so the inspector's `unread` no longer
    lags the MCP session cursor. Cite the commit hash from Task 1 once known.
(b) The broader broker-owned-delivery-position redesign is explicitly RE-PARKED behind the federation
    spike per the 2026-07-01 v0.12-reliability-bucket decision — NOT abandoned. The full design doc
    survives at `docs/superpowers/specs/2026-07-08-999-11-broker-owned-delivery-position-design.md`
    for whenever the spike fires and this is picked back up.
(c) Before implementation resumes, three independent reviews' findings must be addressed — most
    notably: the design doc as written does NOT repoint the two cursor authorities the way this narrow
    fix just did, and its "bounded hole-set" claim was found to be UNBOUNDED in the exact starvation
    scenario it targets.

Commit the HANDOFF.md addendum (docs commit). If gates surface any failure, fix the root cause — do
NOT downgrade, ignore, or `--no-verify` past it.
  </action>
  <verify>
    <automated>cargo fmt --check && cargo clippy -p famp --all-targets -- -D warnings && cargo test -p famp</automated>
  </verify>
  <done>
All three gates pass. HANDOFF.md has a new dated addendum (existing content intact) covering points
(a) narrow fix + commit hash, (b) redesign re-parked behind the federation spike with the surviving
design-doc path, and (c) the three reviews' must-fix findings (authority-repoint gap + unbounded
hole-set). Addendum committed.
  </done>
</task>

</tasks>

<verification>
- `cargo test -p famp --test inbox_unread_matches_delivered` passes (was RED on HEAD).
- `cargo fmt --check`, `cargo clippy -p famp --all-targets -- -D warnings`, `cargo test -p famp` all green.
- `git diff` shows changes ONLY in `crates/famp/src/cli/mcp/tools/inbox.rs`, the committed test, and
  HANDOFF.md. No changes to `cli/inbox/list.rs`, `await_offsets`, or `drain_walk.rs`.
- The atomic cursor write is REUSED (`execute_advance_cursor`), not duplicated.
</verification>

<success_criteria>
- MCP `famp_inbox` write-throughs its cursor to disk monotonically; `famp inspect identities` unread
  matches delivered.
- Acceptance test committed alongside the fix, GREEN.
- CLI `famp inbox list` full-replay behavior unchanged.
- HANDOFF.md addendum records the narrow fix and re-parks the redesign.
</success_criteria>

<output>
Create `.planning/quick/260708-tzk-fix-famp-inspect-identities-unread-count/260708-tzk-SUMMARY.md` when done.
</output>
