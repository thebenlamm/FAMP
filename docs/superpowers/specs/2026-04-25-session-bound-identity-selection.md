# Session-Bound Identity Selection for Same-Repo Windows

- **Date:** 2026-04-25
- **Status:** Proposal
- **Author:** Codex
- **Scope:** Pre-v0.9 scaffolding / roadmap-input proposal

## TL;DR

The current `v0.8` MCP integration binds a window to an identity through
startup configuration:

- Claude Code via repo-local `.mcp.json` with a fixed `FAMP_HOME`
- Codex via user-scope MCP registrations, one MCP server per identity

That model breaks the most important same-host workflow: **multiple agent
windows opened in the same repo should be able to participate as distinct
FAMP identities.**

This proposal introduces a **session-bound identity model**:

- one MCP server configuration per client, not per identity
- a required `famp_register <identity>` step per Claude/Codex window
- identity stored in MCP session state, not inferred from cwd
- `famp_whoami` to inspect the current binding
- all message tools refuse until registration completes

This is best treated as a **pull-forward of the v0.9 MCP contract** onto the
current `v0.8` transport/daemon architecture, so same-repo multi-window
dogfooding becomes possible now without waiting for the local bus.

## Problem Statement

Current dogfooding failure mode:

1. A repo is wired once.
2. Two Claude Code windows are opened in that same repo.
3. Both windows load the same repo-scoped `.mcp.json`.
4. That MCP config points to the same `FAMP_HOME`.
5. Both windows therefore act as the same FAMP identity.

Equivalent problem on Codex:

- Codex can technically see multiple identities, but they are modeled as
  separate global MCP server registrations (`famp-alice`, `famp-bob`, ...),
  not as a per-window identity choice.
- This is workable, but awkward and asymmetric with Claude Code.

Result: **identity is configuration-bound, not window-bound.** That is the
wrong abstraction for same-host multi-agent use.

## Goals

1. Two or more Claude Code windows opened in the same directory can act as
   different FAMP identities.
2. A Claude Code window and a Codex window opened in the same directory can
   act as different FAMP identities.
3. Identity choice happens inside the MCP session, not through per-repo or
   per-identity startup config.
4. The UX validates the planned v0.9 model: windows pick identity at session
   start rather than inheriting it from `FAMP_HOME`.
5. The resulting MCP tool surface is aligned with the planned stable contract,
   not a one-off compatibility hack.

## Non-Goals

1. Implementing the full v0.9 local bus.
2. Removing `FAMP_HOME` or the current daemon/keyring/TLS architecture.
3. Reworking federation semantics.
4. Solving cross-user or cross-host multi-client registration.
5. Designing final slash-command UX for every client.

## Proposed Model

### Core shift

Replace:

- **repo-scoped identity**
- **MCP-server-name-scoped identity**

With:

- **session-scoped identity**

An MCP session starts unbound. The client must explicitly register/select an
identity before calling messaging tools.

### Proposed MCP surface

Minimum additions:

- `famp_register`
- `famp_whoami`

Behavioral change:

- `famp_send`
- `famp_inbox`
- `famp_await`
- `famp_peers`

must refuse with a typed error until the session has registered an identity.

### Identity backing store

For the current pre-v0.9 implementation, registration resolves an identity
name to the existing local wrapper state:

- `~/.famp-local/agents/<identity>`

That lets the MCP server keep using the current `v0.8` architecture under the
hood while presenting a `v0.9`-like session contract at the tool layer.

### MCP server startup

The MCP server should no longer require a fixed `FAMP_HOME` at process start
for same-host local use.

Instead:

- it starts in an **unregistered** state
- `famp_register` binds the session to one identity
- later tool calls resolve against that session binding

For backward compatibility, the old fixed-`FAMP_HOME` mode can remain
temporarily for existing manual setups, but it should be treated as legacy.

## Proposed UX

### Claude Code

One FAMP MCP server is configured for the repo or user. Then, in each window:

```text
register as alice
register as bob
```

or, if explicit commands are preferred:

```text
use famp_register with identity alice
use famp_register with identity bob
```

After registration:

- window A operates as `alice`
- window B operates as `bob`

even though both windows are opened in the same repo.

### Codex

Codex should converge on the same model:

- one FAMP MCP server registration
- per-window `famp_register`
- no separate `famp-alice`, `famp-bob`, etc. registrations required

This removes global MCP server sprawl and makes Codex symmetrical with Claude.

### Failure behavior

Before `famp_register`:

- `famp_send` returns `not_registered`
- `famp_inbox` returns `not_registered`
- `famp_await` returns `not_registered`
- `famp_peers` returns `not_registered`

This is intentional. Silent fallback to a repo-default identity would recreate
the current ambiguity.

## Recommended Defaults

1. **Registration is required.**
   No implicit identity default in the new mode.

2. **Identity is sticky for the session.**
   It persists until the window/session ends.

3. **Re-registration is allowed but explicit.**
   If supported, it should replace the session binding deterministically and be
   visible via `famp_whoami`.

4. **Old fixed-`FAMP_HOME` mode remains temporarily.**
   This is for backward compatibility only, not the preferred dogfooding path.

## Why This Belongs Before Full v0.9

This proposal addresses the exact pain that triggered the local-first re-scope:

- same-host agent workflows are being forced through the wrong abstraction

But it does so with a narrow target:

- change the MCP session model now
- keep the current transport/daemon implementation underneath

That makes it a good **bridge proposal**:

- useful immediately for dogfooding
- aligned with the future `famp_register` shape
- less throwaway than inventing more per-identity config hacks

## Acceptance Criteria

1. Two Claude Code windows in the same repo can register as different
   identities and exchange messages successfully.
2. One Claude Code window and one Codex window in the same repo can register
   as different identities and exchange messages successfully.
3. The MCP server exposes `famp_register` and `famp_whoami`.
4. Calling message tools before registration fails deterministically with a
   typed error.
5. The preferred setup path no longer requires one MCP server config per
   identity.
6. Documentation clearly distinguishes:
   - session-bound identity selection
   - legacy fixed-`FAMP_HOME` behavior

## Risks

1. **Client UX variance.**
   Claude Code and Codex may differ in how reliably they invoke
   registration-first workflows.

2. **Dual-mode complexity.**
   Supporting both legacy fixed-identity MCP startup and session-bound
   registration can create ambiguity if not documented carefully.

3. **Session-state implementation sharp edges.**
   The MCP server must keep identity state per client session, not as shared
   global mutable process state.

## Open Questions

1. Should `famp_register` be allowed to switch identities mid-session, or only
   bind once?
2. Should the preferred configuration be repo-scoped or user-scoped for
   Claude Code when identity is no longer encoded in `FAMP_HOME`?
3. Does Claude Code want slash-command wrappers for registration immediately,
   or is tool-level registration enough for the first pass?
4. Should `famp_peers` remain blocked pre-registration, or be callable before
   identity selection as a discovery aid?

## Recommended Roadmap Placement

Do **not** treat this as an unrelated `v0.8` convenience tweak.

Treat it as one of:

1. a **pre-v0.9 scaffolding milestone** explicitly meant to validate
   session-bound identity choice before the broker lands, or
2. a **pull-forward of the v0.9 MCP contract** onto the current `v0.8`
   implementation

Recommendation: choose option 2 in roadmap language.

That keeps the story coherent:

- `scripts/famp-local` validated local ergonomics
- session-bound MCP identity selection validates `famp_register`
- then the local bus replaces the transport underneath without changing the
  user mental model again

## Suggested Next Step

Have Claude Code review this proposal specifically for:

1. UX clarity
2. MCP-session-state feasibility
3. backward-compatibility risks
4. whether this should be captured as a v0.8.x bridge or a v0.9 phase pull-forward
