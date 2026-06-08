# 999.1 — famp await: cursor-advance / FSM-derivation ordering

- **Filed:** 2026-04-28
- **Status:** Backlog (defer to v0.9 Phase 02 broker-owned FSM)
- **Severity:** silent state corruption; no data loss but silent FSM rot
- **Discovered:** 2026-04-27 — Victoria's Cencora panel produced an in-tree on-disk reproducer
- **Related:** 999.2 (multi-listener lock semantics), `docs/superpowers/specs/2026-04-28-broker-owned-task-fsm.md`

## Problem

`famp await` advances the inbox byte cursor as a side effect of returning a matched envelope, but only applies FSM derivation (`advance_committed`, `advance_terminal`) on the **single matched envelope** in each call. Non-matching envelopes that the cursor crosses are silently abandoned with no FSM update — and there is no other code path that derives FSM state from an envelope after it falls behind the cursor.

Under fan-out (multiple open tasks with interleaved replies), this leaves the originator-side task FSM stuck in stale states even though the wire-level conversation completed correctly. Victims are invisible to the operator: stderr eprintln is the only signal.

## Three sufficient mechanisms (each independently sufficient)

### M1 — `find_match` Some-return leap

`crates/famp/src/cli/await_cmd/poll.rs:71–97`. With `task_filter = Some(X)`, the loop walks entries and skips any whose derived task_id != X. On match, the function returns the matched entry's `end_offset`. The caller at `await_cmd/mod.rs:249` advances the cursor to that offset — leaping over every skipped non-matching entry between the old cursor and the match. **No FSM derivation is applied to the skipped entries.**

The docstring at `poll.rs:69–70` claims "Skipped entries are NOT consumed — the caller's consume-and-discard logic in mod.rs handles advancing past them." This is only true on the `None` return path. When `find_match` returns `Some`, the cursor leap silently consumes the skipped middle.

### M2 — `find_match` None-return + consume-and-discard

`crates/famp/src/cli/await_cmd/mod.rs:255–259`. When `args.task` is `Some` and the entire batch was filter-mismatched, the cursor advances past every entry in the batch via `cursor.advance(*batch_end)`. Zero FSM application. Same bug class as M1, different code path.

### M3 — `advance_terminal` on REQUESTED record

`crates/famp/src/cli/send/fsm_glue.rs:88` requires `state == COMMITTED` to advance to `COMPLETED`. If a prior commit was lost via M1 or M2, the FSM is stuck at REQUESTED. When `await_cmd/mod.rs:218` later sees the terminal deliver and calls `advance_terminal`, it returns `IllegalTransition`, which `await_cmd/mod.rs:224–234` catches and `eprintln!`s. The error is swallowed — the envelope is still printed to the user, the cursor still advances. The terminal envelope is now permanently consumed without any FSM update, and no operator-visible signal exists.

## On-disk reproducer (2026-04-27)

`~/.famp-local/agents/victoria/`:

- `tasks/019dd14f-5a45-7de1-a8c5-1b9f1d3e0602.toml` — peer=magnus, **state=REQUESTED, terminal=false**.
- `inbox.jsonl` line 2 (offset 549–1100): magnus commit envelope, well-formed, causality.ref matches the task UUID.
- `inbox.jsonl` line 4 (offset 4874–11098): magnus terminal deliver envelope, `body.interim = false`.
- `inbox.cursor`: 23238 (end of file). Both magnus envelopes are behind the cursor.

Magnus's side of the same task says `state=COMPLETED`. The peer believes the conversation is closed. Victoria's local state disagrees with no signal.

The mechanism cannot be distinguished post-hoc — M1, M2, and M3 each independently produce the observed end state. Most likely chain: M1 fired during a `famp_await task_id=eric` call that walked past Magnus's commit at line 2 to find Eric's deliver at line 3, leaving Magnus's commit behind the cursor. M3 then fired when a later await consumed Magnus's terminal deliver against a still-REQUESTED record.

**Do not delete or mutate this on-disk state without first capturing it for the regression test fixture.**

## Reproduction recipe

Three open tasks fanning out from one originator. Replies arrive interleaved (taskA-commit, taskB-commit, taskA-deliver-terminal, taskB-deliver-terminal). Originator calls filtered `famp_await task_id=A` first, then filtered `famp_await task_id=B`. Assert both task FSMs reach COMPLETED on the originator side. Test fails on v0.8 HEAD; will pass once Phase 02 ships broker-owned FSM derivation.

Test home for this work: `crates/famp/tests/multi_task_interleave_filtered_await.rs` (to be written; see task #3 in the 2026-04-28 working session).

## Recommended fix path

**Phase 02 (v0.9) — broker-owned FSM:** preferred. Move task-FSM derivation into `Broker::handle` so every dispatched envelope drives a state transition, regardless of which read tool the client calls. See `docs/superpowers/specs/2026-04-28-broker-owned-task-fsm.md`. Kills M1, M2, and M3 simultaneously by removing the side-effect coupling.

**v0.8 patch (only if Sofer or another wild user is actively blocked):** make `find_match` apply FSM derivation as a fold over every entry it walks past, not just the match. Equivalent fix for the consume-and-discard branch. Ship in `cli/await_cmd` and `cli/send/fsm_glue`. Disposable code — Phase 02 replaces this entire surface anyway.

**Either way — surface failures to the operator.** Replace eprintln with audit-log envelopes (the `MessageClass::AuditLog` shipped in Plan 01-03 is the natural conduit). At minimum: every `IllegalTransition` produces an audit_log entry visible to the operator's tooling.

## Out of scope for this item

- **Crash-safety / fsync ordering** of cursor-advance vs FSM-persist: the original 999.1 framing. Still relevant. Once broker-owned FSM lands, this becomes "broker snapshot durability" rather than "client-side cursor.advance + tasks/*.toml fsync ordering" — re-spec at that point.
- **Multi-listener lock semantics:** see 999.2.

## Incidental: architect cursor desync after `famp-local clear`

Observed during the 2026-04-27 sweep: agent `architect` had `inbox.cursor = 35843` while `inbox.jsonl` was 8983 bytes — cursor 4× past EOF. Likely cause: `famp-local clear` truncates the inbox without resetting the cursor. Read paths that compare cursor to file size will see "no new entries" forever. Low blast radius (agent simply receives nothing past the desync) but worth a 30-line fix in `famp-local clear` to also rewrite `inbox.cursor` to 0. File as a sibling backlog item or fold into 999.1's fix; do not bury it.
