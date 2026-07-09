---
status: complete
quick_id: 260709-cxv
date: 2026-07-09
issue: 21
commits:
  - 37aea7c
  - bf554a6
---

# Quick Task 260709-cxv — `famp await --abort-on-fd` (issue #21)

## What shipped

`famp await --abort-on-fd <n>`: a generic, host-neutral cancellation seam. The
command races the parked bus reply against readability (bytes **or** EOF) on a
caller-supplied fd and exits **3** on abort (0 = message/timeout, 1 = error).
`famp await` contains zero references to Claude Code, transcripts, or queues.

`crates/famp/assets/famp-await.sh` runs a background watcher that writes one byte
to fd 9 when the host has queued input, so the Stop hook releases the turn and
Claude Code drains its session queue. Listen mode self-heals: the drained
notification produces a turn, whose Stop hook re-arms the await.

## The architectural question, answered

The issue asked whether listen mode should block the turn at all — a watcher
process plus a fast-returning hook. **It must block.** Claude Code exposes no
mechanism for an external process to inject input into an idle session. Hooks are
the only in-session control surface, and a *blocked Stop hook is itself the wake
mechanism* — it keeps the turn alive so an arriving message can resume the agent.
A fast-returning hook ends the turn, and a watcher that later sees a message has
no way to start a new one. **The block is the feature; the bug was that it was
uncancellable.** `--abort-on-fd` is therefore not the smaller step, it is the
correct one. Recorded so it is not revisited.

## Two design corrections found by checking the plan against real data

The plan's design (inherited from the issue) was wrong in two places. Both were
caught by reading 96 real transcripts under `~/.claude/projects/`, not by reasoning.

### 1. The queue vocabulary is four ops, not two

Measured across 96 transcripts containing `queue-operation` records:

| operation | count |
|-----------|-------|
| `enqueue` | 710 |
| `dequeue` | 434 |
| `remove`  | 269 |
| `popAll`  | 6 |

`remove` (a queued message the user deleted before it ran) and `popAll` also
**drain** the queue. The planned predicate — "outstanding == enqueues > dequeues"
— never decrements on either. It therefore **latches permanently positive after
the first `remove`**, aborting every subsequent Stop hook and silently disabling
listen mode for the rest of the session. That is precisely the failure that got
the rejected byte-growth approach killed.

Simulated over all 96 transcripts, evaluating each predicate at every record
position:

| predicate | abort-positions | worst single session |
|-----------|-----------------|----------------------|
| `enqueues > dequeues` (planned) | 2.08% | **79.8%** |
| last queue-op is an `enqueue` (**shipped**) | 1.85% | **46.1%** |

**Shipped predicate: "the most recent top-level `queue-operation` record is an
`enqueue`."** It is self-clearing — *any* drain op resets it, including ops we
have never seen — so it cannot latch on an unknown vocabulary. It needs only the
final record, so the 2 MB tail bound can never truncate it into a false positive.
It covers both the enqueue that lands while blocked (the reported bug) and one
already outstanding at hook start (a background agent that finished mid-turn — a
byte baseline captured at hook start would never see it). Its one miss,
`enqueue,enqueue,dequeue`, fails toward *not* aborting — today's behavior — and
the next enqueue fires it. **Uncertainty never aborts.**

### 2. My own C1 claim was wrong, and I retracted it

The plan (and the issue) said a grep would spuriously match `"operation":"enqueue"`
embedded in an `enqueue` record's `content` field. **This is false.** `content` is
a JSON *string*, so those bytes are escaped as `\"operation\":\"enqueue\"` and a
grep for the unescaped sequence never matches them. Verified directly.

The first version of the anti-grep test encoded that wrong theory, and so
**passed under both a grep and a JSON parse** — a vacuous test. It was rewritten
(commit `bf554a6`) to use a nested, *unescaped* `queue-operation` object inside a
non-queue record, the shape a structured tool result takes: a grep matches those
bytes, a top-level type check does not. It now fails under grep.

The real reason to parse rather than grep is stronger than the one in the plan: a
substring match **cannot express recency**. It matches every historical enqueue in
the file, including ones drained hours ago, so it would abort every Stop hook
forever. The code comment was corrected to say this.

## Falsification — every test observed failing with the fix reverted

A green test proves nothing until it has been seen red. Each variant below was
patched in, the test run, and the variant reverted. **Controls** confirm the patch
isolated the intended behavior rather than simply breaking the hook.

### Hook predicate (`crates/famp/tests/hook_runner_await.rs`)

| # | Test | Reverted to | Result |
|---|------|-------------|--------|
| 1 | `hook_does_not_abort_when_the_enqueued_item_was_removed` | counting (`enq > deq`) | **FAIL** |
| 2 | `hook_does_not_abort_when_the_queue_was_pop_all_ed` | counting (`enq > deq`) | **FAIL** |
| — | `hook_aborts_when_transcript_has_outstanding_enqueue` (control) | counting | PASS |
| 3 | `hook_does_not_abort_when_enqueue_is_matched_by_dequeue` | grep substring | **FAIL** |
| 4 | `hook_does_not_abort_on_a_nested_non_toplevel_queue_operation` | grep substring | **FAIL** |
| 5 | `hook_does_not_abort_when_the_enqueued_item_was_removed` | grep substring | **FAIL** |
| — | `hook_aborts_when_transcript_has_outstanding_enqueue` (control) | grep | PASS |
| 6 | `hook_aborts_when_transcript_has_outstanding_enqueue` | new-enqueue-only baseline | **FAIL** |
| — | `hook_aborts_when_enqueue_appears_while_blocked` (control) | baseline | PASS |

### Rust seam (`crates/famp/tests/abort_on_fd.rs`)

| # | Test | Reverted to | Observed failure |
|---|------|-------------|------------------|
| 7 | `abort_fd_write_cancels_parked_await_with_exit_3` | abort seam never armed | **FAIL** (panicked at `abort_on_fd.rs:142`) |
| 8 | `abort_fd_close_eof_also_cancels` | abort seam never armed | **FAIL** (panicked at `abort_on_fd.rs:174`) |
| 9 | `inflight_awaitok_beats_a_simultaneous_abort` | abort wins tie (no `biased;`, no grace) | **FAIL** — `in-flight message must beat the abort (exit 0); stdout="{\"aborted\":true}\n"` |
| — | `abort_fd_write_cancels_parked_await_with_exit_3` (control) | abort wins tie | PASS |
| 10 | `invalid_abort_fd_is_a_hard_error_not_ub` | `F_GETFD` validation removed | **FAIL** — `fatal runtime error: IO Safety violation: owned file descriptor already closed, aborting` |

Test 10's failure output is the direct justification for the `mem::forget` on the
EBADF path: dropping an `OwnedFd` that wraps an invalid fd makes `close(2)` return
EBADF, which Rust's IO-safety runtime treats as a fatal double-close and `abort()`s
the process.

**Two falsification runs were themselves invalid and were redone.** The first F2/F3
patches anchored on `import json, os, sys`, which occurs **twice** in the hook — the
identity-extractor block comes first. They clobbered the extractor, so the hook
no-op'd and never armed the watcher: both the target *and* the control failed. The
control is what exposed it. Re-anchored on the watcher block; results above.

## Verification (run in the worktree, post-fix)

- `cargo test -p famp` — **342 passed, 0 failed** (includes `abort_on_fd` 4/4 and
  `hook_runner_await` 20/20). Plain `cargo test`; `cargo nextest -p famp` hangs in
  the test-binary `--list` phase in this repo.
- `cargo test -p famp-bus` — 71 lib + integration, 0 failed.
- `cargo clippy --workspace --all-targets` — **0 warnings, 0 errors** under
  pedantic lints. Nothing silenced or downgraded.
- `cargo fmt --all --check` — clean. (The pre-commit fmt gate caught real drift;
  fixed with `just fmt`, never `--no-verify`.)
- `shellcheck crates/famp/assets/famp-await.sh` — clean.

## The one `unsafe`

A single narrowly-scoped `#[allow(unsafe_code)]` for `OwnedFd::from_raw_fd`,
mirroring the existing documented precedent in `bus_client::spawn`. The `famp`
crate is `unsafe_code = "deny"` (not `forbid`) expressly to permit this. **No
`[lints]` table was touched.** nix 0.31's `fcntl` takes `AsFd`, not `RawFd`, so the
fd must be adopted before it can be validated; validation is the first thing that
happens after adoption.

## Deviations from the plan

- **Predicate changed** from `enqueues > dequeues` to `last op is enqueue` (§1).
- **Anti-grep test rewritten** and its rationale corrected (§2).
- **`Justfile`**: `check-shellcheck` never actually shellchecked `famp-await.sh` —
  it only checked `hook-runner.sh`. Added. This widens CI coverage; it softens nothing.
- **`wait_reply.rs` / `mcp/tools/await_.rs`**: mechanical `aborted: false` /
  `abort_on_fd: None` propagation. The MCP tool surface is unchanged, so no
  `just install` is required on that account.

## Deployment (NOT done by this task)

`crates/famp-bus/` was **not** touched, so **no broker restart is required** — this
is binary + hook only, and no live listen window gets deafened.

Order matters:
1. `just install` — refresh `~/.cargo/bin/famp` so the binary understands `--abort-on-fd`.
2. `famp install-claude-code` — write the new hook to `~/.claude/hooks/famp-await.sh`.

If the hook is installed **before** the binary, every live listen window runs
`famp await --abort-on-fd 9` against a binary that rejects the flag; the hook
fail-opens (`exit 0`) and listen mode is silently off until the binary lands.
