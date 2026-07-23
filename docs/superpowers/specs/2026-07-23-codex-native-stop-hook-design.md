# Codex Native Stop Hook Design

**Date:** 2026-07-23
**Status:** Proposed
**Scope:** Codex listen-mode wake path only, implemented as a native `famp`
hook helper. The same engine should be reusable by other Stop-hook hosts later.

---

## Problem

The current Codex wake path is too brittle for the thing it is supposed to do.
It parks correctly on the bus, but the final wake signal is still assembled in
shell and depends on host utilities. That creates a silent-failure boundary:

- the hook can wait successfully
- the hook can detect a wake successfully
- the hook can still fail to emit the final Codex block decision
- the process can still exit `0`

That is the wrong failure mode for an onboarding path. A missing utility or
shell quirk should not turn wake into a no-op.

The strongest possible end state is a native helper that owns the entire Codex
Stop-hook flow. No `jq`, no Python fallback script, no shell JSON templating.
The hook should be executable, deterministic code.

---

## Goals

1. Codex wake must work even if the host shell has a minimal `PATH`.
2. The wake path must not depend on external utilities such as `jq` or `python3`.
3. The hook must remain fail-open: on malformed input or uncertainty, it exits
   `0` and does not trap the host.
4. The emitted wake signal must stay notification-only. Peer body bytes must
   never enter the final `reason`.
5. The implementation should be reusable for Claude/Codex-style Stop hooks and
   future host adapters.
6. Installation must remain simple enough that “reinstall the new binary” is a
   valid deployment step.

---

## Non-Goals

- Changing the broker protocol.
- Changing the envelope format.
- Changing the meaning of `famp await`.
- Adding a second wake channel for Codex.
- Reintroducing a long blocking shell pipeline as the primary mechanism.

---

## Proposed End State

Replace the shell-based Codex Stop hook with a native `famp` subcommand that
owns the full hook lifecycle.

The hook entry installed into `.codex/hooks.json` should invoke the `famp`
binary directly, for example:

```json
{
  "type": "command",
  "command": "/absolute/path/to/famp",
  "args": ["hook", "codex-stop"],
  "timeout": 86400
}
```

The `famp hook codex-stop` code path should:

1. read the Stop-hook JSON from stdin,
2. resolve the active Codex transcript, using `transcript_path` when present
   and `session_id` fallback when needed,
3. parse the transcript and determine whether listen mode is active,
4. call the existing bus await logic,
5. emit the final `{"decision":"block","reason":"..."}` response itself, and
6. exit `0` on timeout, no-op, or any parse uncertainty.

No shell wrapper is required for correctness. A shell shim may remain only as a
compatibility layer during migration.

---

## Why This Is Stronger

This removes the failure classes that are currently too easy to hit:

- missing `jq`
- shell quoting differences
- environment-dependent `PATH`
- Python availability assumptions
- output templating bugs
- partial success where the hook waits but never wakes the host

The installed hook becomes “just call the binary that already owns FAMP.”
That is the smallest reliable control surface.

---

## Architecture

### 1. Install-time wiring

`famp install-codex` should keep writing:

- `~/.codex/config.toml` MCP entry
- project `.codex/hooks.json`
- Codex hook-trust state

The difference is the installed Stop hook command:

- today: shell script in `.codex/hooks/famp-await.sh`
- proposed: native `famp hook codex-stop`

The binary path should be absolute at install time so Codex does not rely on a
login shell or on a particular `PATH`.

### 2. Native hook helper

Add a `famp` subcommand dedicated to the Codex Stop-hook runtime.
The helper should live in Rust, not shell.

Responsibilities:

- parse the hook payload from stdin
- resolve the correct transcript path
- identify the latest successful `famp_register` / `famp_set_listen` state
- determine the active identity
- call `famp await --as <identity> --timeout 23h` via internal Rust code
- serialize the final block JSON directly from Rust

The helper should not shell out to `jq`, `python3`, or any other utility to
build the final response.

### 3. Shared hook engine

The transcript parsing and wake decision logic should be factored into a shared
Rust module, not duplicated inside a shell asset.

That module should expose three separable pieces:

- input parsing
- listen-state resolution
- wake emission

That keeps Codex and any future host adapters on the same correctness path.

### 4. Fail-open semantics

The helper must preserve the current safety invariant:

- malformed stdin → exit `0`
- missing transcript and unresolved fallback → exit `0`
- transcript parse uncertainty → exit `0`
- broker failure after wake is detected → exit `0`

The only time the helper should emit the block decision is when the hook has a
high-confidence listen-mode match and a wake was actually observed.

---

## Runtime Flow

1. Codex ends a turn and fires the Stop hook.
2. Codex invokes `famp hook codex-stop`.
3. The helper reads stdin and resolves the active transcript.
4. The helper finds the most recent successful listen-mode registration.
5. The helper waits on the bus with the existing await implementation.
6. If a wake arrives, the helper emits the final Codex block JSON directly.
7. Codex resumes, the model calls `famp_inbox`, and the user-visible turn wakes.

If the helper cannot establish a confident listen-mode match, it exits `0`
without blocking the host.

---

## Migration Plan

### Phase 1: Add the native helper

- Implement `famp hook codex-stop` in Rust.
- Reuse the current transcript-resolution logic and `await` logic.
- Serialize the final block decision without shell utilities.
- Add tests that run with `jq` absent from `PATH`.

### Phase 2: Switch install to the native helper

- Update `install-codex` to install a hook entry that invokes the binary
  directly.
- Keep the shell asset only as a compatibility fallback if needed.
- Reinstall Codex in a real project and validate the wake path.

### Phase 3: Remove the shell dependency from the critical path

- Once the native helper is proven, stop treating the shell asset as the source
  of truth for Codex wake.
- Remove the final `jq`/Python-style templating path from the Codex runtime
  path.

---

## Testing Plan

### Unit tests

- transcript with successful `famp_register(listen:true)` resolves to the
  expected identity
- `listen:false` produces a no-op
- `set_listen(false)` cancels listen mode
- malformed or truncated transcript is fail-open
- session-id fallback resolves Codex rollout paths when `transcript_path` is
  missing

### Integration tests

- run the hook with `jq` absent from `PATH`
- verify the hook still emits the correct block decision when a wake arrives
- verify the hook still exits `0` on timeout and uncertainty

### End-to-end check

- install the patched binary
- run `famp install-codex` in a real project
- confirm a message wake produces the expected Codex block response
- confirm `famp_set_listen(false)` stops further wake behavior

---

## Acceptance Criteria

The design is complete only when all of these are true:

- the Codex wake path has no dependency on `jq`
- the Codex wake path has no dependency on `python3`
- the wake path still works with a minimal host `PATH`
- the hook emits the final wake JSON from native code
- install and reinstall both produce the same working behavior
- the tests prove the hook still wakes when common shell utilities are absent

---

## Residual Risk

The host can still fail to invoke the Stop hook at all. This design does not
solve a host that never fires the hook. What it does solve is the class of bugs
where the hook fires, waits, and then silently fails to wake because the shell
environment was incomplete.

That is the failure mode we can and should eliminate permanently.
