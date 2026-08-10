# Review round 2 — catalog (NOT yet executed)

**Range reviewed:** `4fe5d3b..dd29dab` (15 commits, 20 files, ~2398 insertions) — the FULL range.
**Reviewers:** Codex CLI 0.146.1 (cold, read-only) and Claude Opus (cold, read-only). Independent, no shared context.

**Why this round exists:** review round 1 saw only `9805135..8bb2180`. The fix-round commits
`c2c8b4e..dd29dab` were written *in response to* that review and had never been reviewed by anyone.
Round 2's top finding is a defect introduced by one of those fixes — the hazard the
"review the commits containing the fixes" rule exists to catch.

**Status of every item below: CATALOGED, NOT FIXED.** Nothing is pushed.

---

## A. HIGH — `woken` suppression is unsound. Found INDEPENDENTLY by both reviewers. **CONFIRMED by me.**

**Where:** `crates/famp-bus/src/broker/handle.rs:568-572`, added by `3b25e86` (fix round).

```rust
let wake_addr = if woken { None } else { recipient_wake_addr(broker, &name, origin) };
```

**Verified by direct reading, not taken on report:**
- `waiting_clients_for_name` (`awaiting.rs:327-360`) matches the canonical holder **OR any proxy**
  whose `bind_as == name`. Its `proxy_holder_alive` guard only checks that *some* live canonical
  holder exists — never that the proxy belongs to it.
- Proxy `ClientState`s are constructed with `pid: None` (`handle.rs:294-308`), and the liveness
  sweep is `let pid = state.pid?` — **a proxy is never reaped by liveness**, only by its socket
  closing.

So `woken == true` means "some client bound to this name was unparked", NOT "the recipient's
Claude Code window was woken". `3b25e86` keyed the new wake path's suppression on that signal.

**Failure scenarios (three, none requiring exotic state):**
1. **Orphan variant** — a pre-D5 orphaned `famp-await.sh` is still parked on `famp await --as dk`
   (exactly the 60-orphan condition this task exists to fix). The dk window restarts and becomes
   canonical holder with a stored `wake_addr`. Bob DMs dk mid-turn → `woken=true` → **no ping**;
   the orphan consumes the `AwaitOk` and advances the *canonical holder's* `await_offsets` past the
   record. The live session gets no hook wake, no ping, and its next await starts past the message.
   Recovery requires an explicit `famp_inbox`.
2. **No orphan needed** — a human runs `famp await --as dk` in a second terminal. Same outcome.
3. **AwaitTimeout** — a filtered waiter is selected (`woken=true`) but `drain_await_batch` stalls
   behind an earlier unmatched envelope and the awaiter gets `AwaitTimeout` (`awaiting.rs:171-200`,
   the documented 999.1 boundary). Nothing delivered, ping suppressed.

**New vs pre-existing:** the orphan eating a wake and the shared cursor advance are pre-existing.
What `3b25e86` added is *keying suppression on that unreliable signal*, disabling the one mechanism
that would have covered a session the hook path missed.

**Consequence:** the spec's "the implementation never enters the untested quadrant **by
construction**" holds only when the sole waiter is the recipient's own live hook.

**Options:**
- **A1 (recommended)** — narrow the suppression: suppress only when the woken waiter **is the
  recipient's canonical holder**. Preserves the intent of `3b25e86` and closes all three paths.
- **A2** — revert the suppression entirely. Never drops a wake, but re-enters the double-wake
  quadrant the spec marks UNTESTED.
- A1 requires resolving the waiter set to the canonical holder at `handle.rs:568`; the machinery
  already exists (`resolve_await_owner`, `awaiting.rs:157`).

---

## B. HIGH — "latency, never loss" is FALSE. Codex. **CONFIRMED by me.**

**Where:** `crates/famp/src/cli/broker/mod.rs:379-389`. On `env.append` failure the executor
`eprintln!`s and continues; the broker still emits `SendOk` (`handle.rs:587`).

Alice is listening with no parked waiter and her mailbox filesystem is full. Bob sends. Append
fails, **Bob is told it succeeded**, Alice's `famp_inbox` never shows it. That is loss, not latency.

**Scoping — this matters:** the durability gap is **PRE-EXISTING** (the code comment names Phase 4
as future work); this range did not regress it. What this range introduced is a new **absolute
claim** that the behavior falsifies, in three places we wrote:
`CLAUDE.md:54`, `crates/famp/src/cli/mcp/tools/send.rs:181`,
`docs/superpowers/specs/2026-08-10-native-wake-ping-design.md:130` — plus the `260810-hac` STATE row.

**Options:**
- **B1 (recommended)** — qualify the claim to what is actually true: *the ping path* never loses a
  message; mailbox durability is a separate, pre-existing gap. File the `SendOk`-on-append-failure
  bug as its own issue.
- B2 — fix the durability gap (convert append failure into a broker-internal `Err` reply). Real
  work, out of scope for a quick task, and it changes the send reply contract.

---

## C. HIGH — a failed `Register` round-trip poisons the cached bus. Codex. **CONFIRMED by me.**

**Where:** `crates/famp/src/cli/mcp/tools/register.rs:~150` — the `send_recv(Register)` error maps
and `?`-returns **without clearing `guard.bus`**. The only production clear is the new
`WakeAddrOutcome::ConnectionPoisoned` path at `register.rs:172`; `session.rs:319` is `#[cfg(test)]`.
`ensure_bus` returns early whenever `guard.bus.is_some()` and `send_recv` never reconnects.

The daemon restarts mid-`Register` → first `famp_register` fails → the dead stream stays cached →
**retrying cannot recover**; the window must restart its MCP process.

**Why this is worse than it looks in this range:** we bumped `BUS_PROTO_VERSION` 2→3, which
*forces* every live window to re-register after the daemon restart — and CLAUDE.md's new text
asserts "Every live window then re-registers". That sentence depends on the path that can wedge.
The fix round fixed the new `SetWakeAddr` path and left the older, likelier `Register` path
poisoned. `8fce079`'s own commit body records this as "observed, NOT fixed".

**Recommended:** clear `guard.bus` on the `Register` transport-error path too. Small, same pattern
already written next to it.

---

## D. MEDIUM — re-register with no socket leaves a stale `wake_addr` live. Codex. Unverified.

Idempotent `Register` does not clear `wake_addr` (`handle.rs:448`), and when socket detection
returns `None`, `record_wake_addr` reports success **without** sending `SetWakeAddr(None)`
(`register.rs:377`). A socket removed or replaced between registrations leaves the broker handing
out a stale address; pings then go to a dead or reused socket. The broker *can* clear (it has a
unit test for it) — the MCP path just never asks.

---

## E. MEDIUM — always-loaded docs describe pre-suppression behavior. Claude. Unverified.

`CLAUDE.md:54`, `crates/famp/src/cli/mcp/tools/send.rs:24-26`, and `crates/famp-bus/src/proto.rs:433-438`
all list **three** gates for `wake_ping` and omit the fourth that `3b25e86` added — and the omitted
case is the *common* one (a parked listening window never gets a ping). `dd29dab`, whose subject is
"correct the durable record after the fix round", touched STATE.md, SUMMARY.md and the spec but
**not CLAUDE.md**.

A false *behavior* claim in the always-loaded project doc. **Note: if A is fixed via A1, this text
must be rewritten to match the narrowed rule, not merely to add a fourth bullet.**

---

## F. MEDIUM — the SUMMARY's "Estimate calibration" re-asserts pre-fix numbers as current. Claude. Unverified.

The block added in `40c1634` states `067df35..HEAD` is "15 files, +1681/−15, 89,419 chars =
22,354 est-tokens", labelled "Measured, not rounded toward the estimate". At `40c1634` that range
was **22 files, +2699/−34, ~175,948 chars ≈ 43,987 est-tokens** — the quoted figures are the
`067df35..8bb2180` snapshot re-asserted at a commit roughly 2× larger. Frontmatter
`actuals.tokens: 22354` carries the same understatement.

**On the round-1 banner:** accurate about the three claims it retracts and correctly scoped to
"the body below", but **not sufficient** — the frontmatter sits *above* it and is also stale
(`commits: 4` for a 15-commit range; `key-files.modified` omits `README.md`,
`crates/famp/tests/quarantine_skew.rs`, and all three `plugins/*/hooks/famp-await.sh` copies that
the banner's own text says were regenerated).

---

## G. LOW — D3's absolute payload wording. Codex flags, Claude clears. Both are right about different things.

The spec says "**No** peer-influenced byte appears in the SendMessage payload." The `text` field is
a fixed constant (Claude confirmed structurally: `wake_ping` takes only `target_addr`, and that
value is broker-validated against `^uds:/tmp/cc-socks/[0-9]{1,10}\.sock$` before it can reach a
`Delivered` row). But `target_addr` is still peer-**chosen** and appears in `to` and in the
`instruction` sentence. Shape-constrained ≠ not peer-influenced.

This is issue **#48**'s behavior; only the absolute wording needs correcting. Same overstatement
appears in the `260810-hac` STATE row ("literally true").

---

## H. LOW — residual test weaknesses. Both reviewers.

- `the_ping_payload_does_not_vary_with_the_sender` (`send.rs:353`) never supplies two senders and
  never executes `famp_send` — it is weaker than its name. (Consistent with my own mutation run:
  it stayed green while `the_ping_payload_is_byte_exact` went red.)
- The suppression test (`handle/tests.rs:3961`) never verifies the waiter actually received
  `AwaitOk`; `AwaitTimeout` still passes it — i.e. it cannot catch finding A.
- The unreadable-`ps` test (`hook_runner_await.rs:1693`) asserts only that `owner-gone` was not
  logged; a regression that logs "not armed" and exits immediately would pass while disabling
  listen mode.
- Removing the production call sites (`register.rs:159`, `send.rs:114`) leaves the isolated unit
  tests green.

Claude's counterpoint, recorded: `the_ping_payload_is_byte_exact` pins the whole JSON, so a
reintroduced slot must *delete* a test rather than forget a rule. That one is genuinely strong.

---

## I. OPEN, not a finding — the D5 hook spawn-path topology is still unvalidated.

Both reviewers reached the same limit independently.

**Established:** the Stop entry in `~/.claude/settings.json` is a bare path with no wrapper; the
guard refuses to arm when the captured ppid is empty/`0`/`1` (`famp-await.sh:523-529`), so a hook
already reparented at t=0 can never abort; all three test arms (kill / live-owner control /
ps-unreadable fail-open) pass; every degraded path is fail-open. Codex additionally probed this
macOS host and saw `/bin/sh -c '<simple command>'` exec-replace itself, which preserves the direct-
parent relationship — but that is an optimization, not a host contract.

**Not established:** whether the real Claude Code spawn path ever interposes a shell that forks and
exits. If it does, `_now_ppid != OWNER_PPID` on the first poll and **every park aborts, silently
disabling listen mode.** The tests manufacture a long-lived wrapper and so cannot settle it.

**One command settles it, on a window actually in listen mode:**
```sh
p=$(pgrep -x -f '.*famp-await\.sh'); ps -o pid=,ppid=,command= -p $p; sleep 5; ps -o ppid= -p $p
```
Compare against the `owner-liveness guard armed (owner ppid=…)` line in
`${XDG_STATE_HOME:-$HOME/.local/state}/famp/await-hook.log`. Beware: a bare
`pgrep -f famp-await.sh` false-positives on codex processes carrying the string in their prompt.

---

## Confirmed clean by round 2 — do not re-litigate

- Wire bump 2→3 rejects cleanly in **both** directions (`hello()` compares `!=`, not `<`); no
  silent half-working peer; `Delivered` is constructed only inside `famp-bus`; no on-disk state
  reads the old shape.
- All three plugin hook copies byte-synced with the asset; regeneration idempotent across two
  passes in a clean exported checkout (Codex verified independently of my own check).
- The sender-name removal is the right fix; `sole_delivery_row_wake_addr` closes the genuine
  zero-case hole from round 1.
- The wake-address **ownership** gap is documented with rationale and filed as issue #48 —
  deliberate, not a new finding.
- The four-step deploy text is accurate (`install`, `install-gateway`, `install-all`,
  `famp install-claude-code` all exist).
- No error-swallowing defect in the `WakeAddrOutcome` three-way split — it is the opposite of
  swallowing.

## Discrepancy to resolve, not yet explained

Claude reported `cargo test -p famp --lib` showing 2 pre-existing failures
(`cli::pair::revoke::tests::cli_rejects_*`, "`CARGO_BIN_EXE_famp` is unset"). My own
`cargo test --workspace` run on this range reported **0 failures**. Both cannot be describing the
same conditions; resolve before quoting either number as a gate result.
