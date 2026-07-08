---
phase: quick-260708-nwv
plan: 01
type: tdd
wave: 1
depends_on: []
files_modified:
  - crates/famp/src/cli/mcp/session.rs
  - crates/famp/src/cli/mcp/tools/inbox.rs
  - crates/famp/src/cli/mcp/tools/register.rs
  - crates/famp/tests/mcp_bus_e2e.rs
autonomous: true
requirements: [ISSUE-13]

must_haves:
  truths:
    - "Two successive famp_inbox calls in one MCP session: the second returns only envelopes that arrived between the two calls (no full-mailbox replay)."
    - "An explicit since:0 from the caller still forces a full mailbox replay (recovery escape hatch preserved)."
    - "Re-registering as a different identity in the same session resets the remembered inbox offset."
  artifacts:
    - "crates/famp/src/cli/mcp/session.rs holds inbox_offset: Option<u64> with async get/set accessors."
  key_links:
    - "tools/inbox.rs reads the remembered offset when no `since` is supplied, and always stores out.next_offset after a successful list."
    - "register.rs bind site (line ~118) and session::set_active_identity both reset inbox_offset to None on identity bind."
---

<objective>
Fix issue #13: the MCP `famp_inbox` tool has no cursor advance, so every call
replays the agent's entire mailbox into agent context. Have the MCP **session
layer** remember `InboxOk.next_offset` per session and pass it as `since` on the
next call. Session-scoped only — no wire change, no broker change, no disk cursor.

Purpose: Protect the agent's *context* (not durable delivery state) from the
double-print pattern `docs/CLAUDE-CODE-CONTEXT-GUIDE.md` warns about.
Output: `inbox_offset` session field + accessors; `tools::inbox` uses/stores it;
identity rebind resets it; doc comment records the first-call-replays decision.

DO NOT REDESIGN. The fix shape below is agreed and locked.
</objective>

<execution_context>
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/workflows/execute-plan.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@crates/famp/src/cli/mcp/session.rs
@crates/famp/src/cli/mcp/tools/inbox.rs
@crates/famp/src/cli/mcp/tools/register.rs
@crates/famp/tests/mcp_bus_e2e.rs
</context>

<constraints_locked>
Preserve ALL of these verbatim in intent — they are hard constraints from the task spec:

- **No wire change.** No new `BusMessage`/`BusReply` field. No broker change.
- **Do not touch the on-disk `.<name>.cursor`.** That authority is slated for
  deletion in backlog 999.11. This fix is deliberately session-scoped, not durable.
- **Do not reintroduce `action`/`offset` to the `famp_inbox` schema.** They were
  deleted on purpose (PR #8); `action: "ack"` was a silent no-op.
- Channel cursors (`inbox_offsets`) are already broker-owned and advance
  server-side. `InboxOk.next_offset` is the **agent mailbox** offset ONLY —
  `since` does NOT affect the channel merge. Confirm before assuming otherwise.
- Mailboxes can shrink (#11, #16). ALWAYS store the *returned* `next_offset`,
  NEVER `max(stored, returned)` — the broker clamps and you must follow it down.
- Workspace lints: `clippy::all` + `pedantic` = deny; `unwrap_used`/`expect_used`
  = deny. `cargo fmt --all` before every commit. Never `--no-verify`.
- **Never `cargo nextest`** — it hangs in this repo's test-binary `--list` phase.
  Plain `cargo test`.
- `crates/famp-bus/tests/prop04_drain_completeness.rs` and
  `tdd02_drain_cursor_order.rs` MUST end byte-identical. If your change needs
  them edited, STOP — you have drifted into broker semantics.
</constraints_locked>

<tasks>

<task type="tdd" tdd="true">
  <name>Task 1: RED — failing tests for session-scoped inbox cursor advance</name>
  <files>crates/famp/tests/mcp_bus_e2e.rs, crates/famp/src/cli/mcp/session.rs</files>
  <behavior>
    Three behaviors, written as failing tests BEFORE any implementation:

    - Incremental read (e2e, in mcp_bus_e2e.rs using McpHarness): one identity
      registers; a peer sends message A; the identity calls famp_inbox (sees A);
      the peer sends message B; the identity calls famp_inbox AGAIN with no
      `since` — the second call's `entries` contains ONLY B, not A and B. Assert
      the second call did not replay A. Follow the existing McpHarness patterns
      already in this file (test_inbox_task_id_populated_for_new_task, the
      listen-mode test) for process spawn + register + send + inbox wiring.
    - since:0 escape hatch (e2e): after the incremental read above, a third
      famp_inbox call with explicit {"since": 0} returns the FULL mailbox again
      (both A and B). Proves the deliberate full-replay recovery path survives.
    - Identity-rebind reset (unit test in session.rs #[cfg(test)] mod, following
      the existing clear()/set_active_identity test-double pattern): set
      inbox_offset to Some(N); call set_active_identity with a different name;
      assert inbox_offset is now None. This pins the reset that stops a stale
      byte offset from reading the wrong mailbox at a meaningless position.

    NOTE for compile: the unit test references the new inbox_offset field +
    accessors, so add the field/accessors as minimal stubs ONLY as far as needed
    to compile the RED test. The e2e behavioral tests must still FAIL RED because
    tools/inbox.rs does not yet use/store the remembered offset.
  </behavior>
  <action>
    Add the three tests described in <behavior>. Prefer e2e (McpHarness) for the
    incremental-read and since:0 behaviors; use a session.rs unit test for the
    rebind reset. Add a minimal `inbox_offset: Option<u64>` field to SessionState
    (default None in `state()` init) plus async get/set accessors mirroring the
    existing `last_send`/`set_last_send` pattern, ONLY so the RED tests compile.
    Do NOT yet wire tools/inbox.rs — the e2e tests must fail RED (second call
    still replays the whole mailbox). Run the tests and confirm RED before Task 2.
    Commit RED: `test(quick-260708-nwv): failing tests for inbox cursor advance`.
  </action>
  <verify>
    <automated>cargo test -p famp --test mcp_bus_e2e 2>&1 | grep -E "FAILED|test result" | head; cargo build --workspace --all-targets</automated>
  </verify>
  <done>Incremental-read and since:0 e2e tests compile and FAIL (mailbox replays); rebind-reset unit test compiles. Workspace builds. Committed RED.</done>
</task>

<task type="tdd" tdd="true">
  <name>Task 2: GREEN — wire the session offset through tools::inbox and identity rebind</name>
  <files>crates/famp/src/cli/mcp/session.rs, crates/famp/src/cli/mcp/tools/inbox.rs, crates/famp/src/cli/mcp/tools/register.rs</files>
  <action>
    Implement the agreed fix so the Task 1 tests go GREEN:

    1. session.rs: finalize `inbox_offset: Option<u64>` on SessionState with async
       `inbox_offset()` getter and `set_inbox_offset(Option<u64>)` setter,
       mirroring the `last_send()`/`set_last_send()` accessor pattern already in
       the file. Update the SessionState doc comment to mention the new field.
    2. session.rs `set_active_identity`: reset `inbox_offset = None` when the
       identity is (re)bound — a stale byte offset against a different mailbox
       would read at a meaningless position. Also reset it in `clear()`
       (#[cfg(test)] only) for consistency.
    3. register.rs (~line 118): register binds identity INLINE on the held mutex
       guard (`guard.active_identity = Some(active.clone())`) and CANNOT call
       `session::set_active_identity` (that re-locks the same tokio Mutex →
       deadlock). So add `guard.inbox_offset = None;` right next to the
       active_identity assignment on the RegisterOk arm. This is the production
       rebind path; the reset MUST live here, not only in set_active_identity.
    4. tools/inbox.rs `call`: when the caller supplies NO `since` (the None arm),
       use `session::inbox_offset().await` as the effective `since`. An explicit
       caller `since` (including 0) wins and is used as-is. After a successful
       `run_at_structured`, ALWAYS `session::set_inbox_offset(Some(out.next_offset)).await`
       — store the RETURNED value, never max(stored, returned) (mailboxes shrink,
       #11/#16; follow the broker's clamp down). `since: 0` therefore stays a
       deliberate full-replay escape hatch AND updates the stored value.
    5. tools/inbox.rs doc comment: document (a) that the session layer now
       remembers next_offset and passes it as `since` on the next call, (b) that
       explicit `since: 0` forces full replay for recovery, and (c) THE DECISION:
       because `famp_register` discards `RegisterOk.drained` and seeding the
       offset from register would need a wire change (out of scope), the FIRST
       famp_inbox of a session still replays the whole mailbox — this is an
       accepted once-per-session cost and a recovery affordance, NOT a bug. State
       this explicitly so the next person does not re-file it.

    Run the Task 1 tests to GREEN, then the full green gate. Confirm the
    prop04/tdd02 broker property test files are untouched (byte-identical).
    Commit GREEN: `fix(quick-260708-nwv): session-scoped inbox cursor advance (#13)`.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p famp --lib && cargo test -p famp --tests && cargo test -p famp-bus && git diff --stat HEAD -- crates/famp-bus/tests/prop04_drain_completeness.rs crates/famp-bus/tests/tdd02_drain_cursor_order.rs | grep -q . && echo "BROKER-PROP-TESTS-CHANGED-STOP" || echo "broker-prop-tests-untouched-ok"</automated>
  </verify>
  <done>All Task 1 tests GREEN. Green gate passes: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p famp --lib`, `cargo test -p famp --tests`, `cargo test -p famp-bus` all pass. prop04_drain_completeness.rs and tdd02_drain_cursor_order.rs are byte-identical (no diff). Doc comment records the first-call-replays decision. Committed GREEN.</done>
</task>

<task type="auto">
  <name>Task 3: Deploy the changed MCP surface</name>
  <files>(no source files — build/install step)</files>
  <action>
    `cli/mcp/` changed, so run `just install` before the PR closes.
    `~/.cargo/bin/famp` is what every agent session reads; `target/release/famp`
    is NOT the deployment target. Broker restart is NOT required — no broker code
    changed. Do NOT push without asking the user first.

    If `cargo test -p famp --test http_happy_path` was run and failed with
    `ReqwestFailed(... TimedOut)`: that is a known stale-`target/` artifact, not
    this change. Re-run under `CARGO_TARGET_DIR=$(mktemp -d)`. Do not chase it.
  </action>
  <verify>
    <automated>just install && test -x "$HOME/.cargo/bin/famp" && echo "installed-ok"</automated>
  </verify>
  <done>`just install` succeeded; `~/.cargo/bin/famp` is the freshly built binary. No broker restart performed (not needed). No push without user approval.</done>
</task>

</tasks>

<verification>
Full green gate (from the task spec, verbatim intent):
- `cargo build --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p famp --lib`
- `cargo test -p famp --tests`
- `cargo test -p famp-bus`
- `crates/famp-bus/tests/prop04_drain_completeness.rs` and
  `tdd02_drain_cursor_order.rs` end byte-identical (no diff vs HEAD). If they
  needed editing → STOP, you drifted into broker semantics.
- `cargo fmt --all` clean before every commit. Never `--no-verify`. Never `cargo nextest`.
- Known non-issue: `http_happy_path` `ReqwestFailed(... TimedOut)` → stale
  `target/`, re-run under `CARGO_TARGET_DIR=$(mktemp -d)`, do not chase.
</verification>

<success_criteria>
- Second famp_inbox in a session returns only envelopes that arrived since the first.
- Explicit `since: 0` still forces a full replay.
- Re-registering as a different identity resets the remembered offset.
- No wire/broker/disk-cursor change; `famp_inbox` schema unchanged (no action/offset).
- Stored offset is always the returned next_offset (follows shrink down).
- tools/inbox.rs doc comment records the first-call-replays-is-accepted decision.
- Green gate passes; broker property test files byte-identical.
- `just install` run; no push without approval.
</success_criteria>

<output>
Create `.planning/quick/260708-nwv-fix-issue-13-mcp-famp-inbox-has-no-curso/260708-nwv-SUMMARY.md` when done.
</output>
