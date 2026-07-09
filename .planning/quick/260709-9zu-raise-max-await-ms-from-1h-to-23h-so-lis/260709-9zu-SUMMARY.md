---
status: complete
quick_id: 260709-9zu
date: 2026-07-09
commits:
  - 5a27769
  - 5661a1a
  - de067df
---

# Quick Task 260709-9zu — Raise broker `MAX_AWAIT_MS` from 1h to 23h

> **Note:** this SUMMARY.md was reconstructed by the orchestrator. The executor
> wrote it inside its isolated worktree but was instructed not to commit docs
> artifacts; `worktree.cleanup-wave` then removed the worktree, destroying the
> uncommitted file. Every claim below was independently re-verified on the main
> checkout after the merge — none of it is copied from the executor's report.

## The bug

FAMP listen-mode windows install a Claude Code Stop hook
(`crates/famp/assets/famp-await.sh`) that blocks on:

    famp await --as <identity> --timeout 23h

`crates/famp-bus/src/broker/awaiting.rs:70` silently clamped that request:

    const MAX_AWAIT_MS: u64 = 60 * 60 * 1000; // 1 hour
    let timeout_ms = timeout_ms.min(MAX_AWAIT_MS);

So the broker parked every await for **at most 1 hour**. On expiry the hook logs
`clean stop` and `exit 0`. Claude Code Stop hooks do not re-fire until the
session's next turn — and an idle listen window never has one. **Every listen
window therefore went deaf one hour after its last turn**, silently. The hook log
(`~/.local/state/famp/await-hook.log`) contained **54** such `clean stop` events.

## The fix

One production line plus its comments:

    const MAX_AWAIT_MS: u64 = 23 * 60 * 60 * 1000; // 23 hours

Line 70's `.min(MAX_AWAIT_MS)` is **byte-for-byte unchanged**. It is the WR-05
guard: `Instant + Duration` panics on overflow, and `Duration::from_millis(u64::MAX)`
is ~584M years, so a malicious or buggy client sending the max would crash the
broker actor task and take down every connected client. Raising the ceiling does
not weaken that guard — `.min()` still clamps a hostile value.

The WR-05 comment block was extended to record why **23h and not 24h**: the Stop
hook is installed with a Claude Code hook timeout of 86400s (24h). A 23h broker
cap means the broker returns a clean `BusReply::AwaitTimeout` and the hook exits 0
gracefully, rather than the harness SIGKILLing the hook mid-block at 24h.
`MAX_AWAIT_MS` must not be raised to >= 24h.

## Why this is safe

Verified against the code by two independent reviewers (matt-essentialist and a
Fable 5 reviewing architect), then spot-checked by the orchestrator:

- **No gap-hole.** `awaiting.rs:45-64` — a fresh await drains any backlog
  immediately before parking. A longer cap cannot cause a missed message.
- **No leaked registrations.** `handle.rs:888,900` — disconnect prunes the parked
  await on socket close, *not* at deadline. The 1h cap was never performing
  cleanup; disconnect is. A dead window leaks nothing under a 23h cap.
- **No timer cost.** The expiry sweep (`handle.rs:933`) is a fixed 1s
  `tokio::time::interval` driving `now >= deadline`, not per-await timers.
- **No memory cost.** `ParkedAwait { client, filter, deadline }` is
  deadline-value independent.
- **Laptop sleep is benign.** Rust's `Instant` on Darwin freezes during system
  suspend, so a parked await burns zero budget while asleep and can never expire
  early. A "23h" await means 23h of uptime — for a nightly-sleeping laptop that
  stretches across several calendar days, which is the desired behavior.

## Tests

Two tests added to `crates/famp-bus/src/broker/handle/tests.rs`, reusing the
module's existing `hello_canonical` / `register` helpers and its synthetic-clock
style (the broker takes `now: Instant` as an input parameter, so no sleeping):

- `await_over_one_hour_survives_past_old_1h_ceiling` — parks a 23h await, drives
  `BrokerInput::Tick` at `now + 3700s` (past the old 3600s ceiling), asserts no
  `AwaitTimeout` was emitted and the client remains in `pending_awaits`.
- `await_u64_max_timeout_parks_without_overflow_panic` — WR-05 regression: a
  hostile `u64::MAX` timeout must still park (clamped), not overflow-panic.

## Verification (run on the main checkout, post-merge)

- `cargo test -p famp-bus` — **71 lib tests pass, 0 failed** (plus 4 integration,
  0 doc-tests). Plain `cargo test`, not nextest: `cargo nextest -p famp` hangs in
  the test-binary `--list` phase in this repo.
- `cargo clippy --workspace --all-targets` — clean, zero warnings. This repo runs
  pedantic lints in CI; nothing was silenced or downgraded.
- **Falsification.** Reverting the constant to `60 * 60 * 1000` makes
  `await_over_one_hour_survives_past_old_1h_ceiling` FAIL with
  `"a 23h await must NOT expire at the old 1h ceiling"`. Restoring 23h makes it
  pass. The test therefore actually pins the constant rather than passing
  vacuously under both values.

## Explicitly out of scope

A second, larger bug was found during the same investigation and **deliberately
deferred**: a Stop hook blocked inside `famp await` owns the turn, so Claude Code's
main loop never drains its session queue. A background subagent that finishes has
its completion notification enqueued and stranded until the user hits Esc.

Evidence (AgentOS session `d428a24c`): `enqueue` at 10:26:08.056Z, `dequeue` at
11:01:02.537Z — 34m54s later, the instant the hook was killed.

That fix needs an abort mechanism, not a constant. Both reviewers agreed it should
not be rushed into a security-sensitive bash hook under time pressure. Recommended
seam when it is picked up: an `--abort-on-fd <n>` cancellation pipe on `famp await`
(Rust `select()`s over broker socket + abort fd, distinct exit code on abort), with
the Claude-Code-specific "watch the transcript for a new `"operation":"enqueue"`
record" logic staying in the hook where it belongs. `famp await` stays host-neutral.
Nothing in this task's code or comments references that deferred work.

## Deployment

`MAX_AWAIT_MS` is compiled into the broker, so this needs `just install` **and** a
broker restart. Restart drops currently-parked awaits; each affected listen window
fail-opens and re-arms on its next turn. Those windows were already deaf at 1h, so
the restart deploys the fix and never regresses behavior.
