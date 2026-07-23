# Codex Native Stop Hook Design

**Date:** 2026-07-23
**Status:** Final
**Scope:** Codex listen-mode Stop-hook path, implemented as a native `famp`
hook helper. The same engine should be reusable by other Stop-hook hosts later.

---

## Problem

The current Codex wake path can succeed at the bus layer and still fail at the
host wake layer. The failure mode is silent:

- the hook parks correctly
- the hook observes a wake correctly
- the hook then fails to emit the final block decision
- the process still exits `0`

That is the wrong failure mode for onboarding more AIs. A missing utility,
shell quirk, or PATH mismatch should not turn a successful wake into a no-op.

The strongest possible end state is a native helper that owns the full Codex
Stop-hook lifecycle. No `jq`, no Python fallback script, no shell JSON
templating in the critical path.

---

## Goals

1. Codex wake emission must work with a minimal host `PATH`.
2. The final block decision must be serialized from native code in the same
   binary that owns FAMP.
3. Fail-open remains: malformed input or uncertainty exits `0`.
4. The wake signal stays notification-only. Peer body bytes never enter
   `reason`.
5. Behavior stays parity-compatible with the current shell adapter where it
   matters.
6. Install remains simple enough that reinstalling the binary is a valid
   deployment step.
7. The shared Rust engine should be structured so Claude can adopt it later
   without a second architecture.
8. Post-wake failures must be loud in logs, even when the helper exits fail-open.

---

## Non-Goals

- Changing the broker protocol or envelope format.
- Changing the meaning of `famp await`.
- Adding a second wake channel for Codex.
- Reintroducing a long blocking shell pipeline as the correctness path.
- Claiming to fix hosts that never invoke Stop hooks.
- Replacing Grok’s non-blocking `listen-wake` path with a blocking Stop hook.

---

## Proposed End State

Replace the shell-based Codex Stop hook with a native `famp` subcommand that
owns the full hook lifecycle.

The hook entry installed into `.codex/hooks.json` should invoke the `famp`
binary directly. Codex today uses a single `command` string for the hook entry;
do not invent a separate `args` array unless verified in a real Codex release.
The command string must be the absolute `famp` path followed by the native hook
entrypoint tokens.

The native `famp hook codex-stop` path should:

1. read the Stop-hook JSON from stdin,
2. resolve the active Codex transcript, using `transcript_path` when present
   and `session_id` fallback when needed,
3. parse the transcript and determine whether listen mode is active,
4. call the existing bus await logic in-process,
5. emit the final `{"decision":"block","reason":"..."}` response itself, and
6. exit `0` on timeout, no-op, or any parse uncertainty.

No shell wrapper is required for correctness. A shell shim may remain only as a
temporary migration aid.

---

## Architecture

### 1. Shared hook engine

The transcript parsing and wake decision logic should live in a Rust module,
not in shell. The engine should expose separable pieces for:

- stdin parsing
- transcript / rollout resolution
- identity replay
- PID-correlated fallback
- await execution
- notification shaping
- JSON emission
- logging

Codex is the first consumer. Claude can adopt the same engine later.

### 2. Runtime entry

`famp hook codex-stop` is the runtime entrypoint.

Responsibilities:

- parse Stop-hook stdin
- resolve the transcript or rollout path
- determine the active listen identity
- await on the bus in-process
- build the notification-only reason
- emit the final block JSON directly from Rust

The helper should not shell out to `jq`, `python3`, or `famp` itself in the
critical path.

### 3. Install-time wiring

`famp install-codex` should continue to write:

- `~/.codex/config.toml` MCP entry
- project `.codex/hooks.json`
- Codex hook-trust state

The difference is the installed Stop hook command:

- today: shell script in `.codex/hooks/famp-await.sh`
- final: native `famp hook codex-stop`

Resolve the `famp` path at install time and seed trust over the exact installed
command string.

### 4. Uninstall-time wiring

`famp uninstall-codex` must remove:

- MCP entry
- native Stop command entries
- legacy shim Stop entries
- shim file if present
- matching trust keys and stale FAMP hashes

Leaving a native hook after uninstall is a ship blocker.

---

## Fail-Open Semantics

The helper must preserve the current safety invariant:

- malformed stdin → exit `0`
- missing transcript and unresolved fallback → exit `0`
- transcript parse uncertainty → exit `0`
- broker failure after wake is detected → exit `0`

The only time the helper should emit the block decision is when it has a
high-confidence listen-mode match and a wake was actually observed.

If a wake is observed but the helper cannot emit a valid block decision, it
must log a distinct error line including identity, mailbox kind, and failure
reason. A one-shot diagnostic block is optional, but only if the host schema
accepts it cleanly.

Never exit non-zero to “force” attention if that would trap the host in a
broken Stop state.

---

## Runtime Flow

1. Codex ends a turn and fires the Stop hook.
2. Codex runs the installed native command string.
3. The helper reads stdin JSON and ignores further stdin.
4. Resolve transcript: `transcript_path` if valid; else session-id rollout
   resolution.
5. Resolve identity: transcript listen-state replay; else PID-correlated
   fallback unless disabled.
6. If no confident listen identity is found, log and exit `0`.
7. Validate identity syntax and length.
8. Await on the bus in-process for up to 23h.
9. On timeout, empty wake, or abort semantics, log and exit `0`.
10. Shape the notification and validate sender.
11. Emit the final block JSON directly from Rust.

The hook should never need a second shell-layer transformation to become the
final Codex wake signal.

---

## Behavior Parity

The native helper should preserve the parts of the shell adapter that matter:

- fail-open exit `0`
- stdin JSON parsing
- transcript / rollout resolution
- listen-state replay
- PID-correlated fallback with anti-hijack rules
- minimal 2 MB transcript tail scan
- identity and sender validation
- channel vs agent reason templates
- unread suppression for agent mailboxes
- logging of every branch

The helper should replace the parts that are brittle:

- `jq` for final emission
- Python for final emission
- shell JSON templating
- PATH-dependent helper execution

---

## Migration Plan

### Phase 1: Implement the native helper

- Add `famp hook codex-stop` in Rust.
- Port the shared transcript / rollout / identity logic.
- Serialize the final block decision without shell utilities.
- Add tests that run with `jq` and `python3` absent from `PATH`.

### Phase 2: Switch `install-codex`

- Point the Stop hook at the native helper.
- Prune legacy shell-hook entries and stale trust.
- Reinstall Codex in a real project and validate the wake path.

### Phase 3: Remove the shell dependency from the critical path

- Stop treating the shell asset as the source of truth for Codex wake.
- Keep the shell shim only if it is still needed for a migration window.

---

## Testing Plan

### Unit tests

- successful `famp_register(listen:true)` resolves to the expected identity
- `listen:false` produces a no-op
- `set_listen(false)` cancels listen mode
- malformed or truncated transcript is fail-open
- session-id fallback resolves Codex rollout paths when `transcript_path` is
  missing

### Integration tests

- run the helper with `jq` absent from `PATH`
- verify the hook still emits the correct block decision on wake
- verify the hook still exits `0` on timeout and uncertainty
- verify trust hash and uninstall symmetry for the native command string

### End-to-end check

- install the patched binary
- run `famp install-codex` in a real project
- confirm a wake produces the expected Codex block response
- confirm `famp_set_listen(false)` stops future wake behavior

---

## Acceptance Criteria

The design is complete only when all of these are true:

- the Codex wake path has no dependency on `jq`
- the Codex wake path has no dependency on `python3`
- the wake path still works with a minimal host `PATH`
- the hook emits the final wake JSON from native code
- install and reinstall both produce the same working behavior
- uninstall removes both native and legacy FAMP Stop entries
- tests prove the hook still wakes when common shell utilities are absent

---

## Residual Risk

The host can still fail to invoke the Stop hook at all. This design does not
solve a host that never fires the hook.

What it does solve is the class of bugs where the hook fires, waits, and then
silently fails to wake because the shell environment was incomplete. That is
the failure mode we should eliminate permanently.

---

## Deferred Parity

The shell adapter (`famp-await.sh`) supports issue-#21 queue-watch semantics:
an `--abort-on-fd` await mode that lets a parked awaiter release early when the
host's own input queue has pending user input, rather than holding the park
for the full timeout.

The native `famp hook codex-stop` helper does **not** implement this. It always
calls `await_cmd::run_at_structured` with `abort_on_fd: None`, so
`AwaitOutcome::aborted` is currently always `false`. The `if outcome.aborted`
branch in `codex_stop.rs` is kept as defensive code (correct if abort is ever
armed later) but is unreachable today.

This is intentional and out of scope for this design (parity item P14,
deferred). The user-visible consequence: a parked Codex Stop hook will not
release early when the host queues new user input mid-park — it holds the
park until a bus wake or the timeout, same as if `--abort-on-fd` had never
existed for Codex. Revisit if Codex gains an equivalent host-side signal
worth wiring up.
