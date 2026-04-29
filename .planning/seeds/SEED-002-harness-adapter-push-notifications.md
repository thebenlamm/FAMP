---
id: SEED-002
status: dormant
planted: 2026-04-28
planted_during: v0.9 Local-First Bus / Phase 02 mid-execution
trigger_when: v0.9.0 ships and v0.10/v1.0 scoping begins
scope: Small
---

# SEED-002: Harness-adapter for push-based message notification

Replace the `famp await` polling pattern (and the `.famp-listen` sentinel + global Stop hook trick) with a small `famp watch --notify <command>` subscriber. The subscriber connects to the broker's per-identity event stream, and on envelope arrival fires a harness-specific notification (e.g. Claude Code's `RemoteTrigger` to inject the envelope into the running session as a system message, like the `<task-notification>` pattern that fires when background subagents complete). Other harnesses get their own adapters (Copilot, MCP push notifications for spec-pure clients).

## Why This Matters

The current UX has agents block on `famp await` (long-poll) and uses a global Stop hook + `.famp-listen` sentinel file to make Claude Code agents wait for incoming messages. Sofer's first-wild-user field report (2026-04-26) surfaced this as the dominant pain point: agents burn wall-clock time blocking, and the `.famp-listen` convention is brittle (sentinel files in cwd, hooks fighting harness behavior).

The structurally clean answer is event-driven delivery — when an envelope arrives for a bound identity, the harness gets notified and injects the envelope as a system message. The agent responds without ever calling `famp await`. This matches how Claude Code's own `<task-notification>` mechanism works for background subagents: zero polling, push-on-arrival.

The v0.9 broker already has a per-identity event stream (it has to, to drive the `await` long-poll). So this is **additive, not a rework** — `famp watch` is a ~50-line subscriber that translates broker events into harness-specific notifications. The FAMP wire protocol stays harness-agnostic. Each harness gets its own adapter binary or wrapper.

## When to Surface

**Trigger:** v0.9.0 ships and v0.10/v1.0 scoping begins.

This seed should be presented during `/gsd-new-milestone` when the milestone scope matches any of these conditions:
- v0.9.0 has shipped and Sofer (or named equivalent) has run FAMP across machines (the v1.0 readiness trigger)
- "polling" or "await" or "Sofer field report" appears in milestone scope discussion
- v0.10 / v1.0 federation milestone is being scoped
- Any milestone targeting agent-orchestration UX or Claude Code integration polish

**Do NOT surface during:** v0.9.x patch milestones, conformance vector pack work, or anything narrower than a full minor version bump.

## Scope Estimate

**Small** — a few hours to a day for the reference Claude Code adapter.

Rough breakdown:
- `famp watch <name> --notify <command>` subcommand: ~30 lines (subscribe to broker per-identity stream, exec command per envelope)
- Claude Code adapter: shell wrapper that calls `RemoteTrigger` with the envelope as the prompt — ~20 lines
- Documentation: usage example replacing `.famp-listen` convention
- Migration note: `.famp-listen` + Stop hook stays as fallback for harnesses without push support

Other harness adapters (Copilot, MCP-push-spec-pure) are separate small follow-ups, not blockers.

## Breadcrumbs

**Existing polling pattern (the thing this replaces):**
- `.planning/PROJECT.md` — describes the `.famp-listen` sentinel + global Stop hook convention
- Memory: `project_famp_listener_sentinel.md` — codifies the current convention
- `.planning/RETROSPECTIVE.md` — Sofer field report documenting the pain

**v0.9 broker foundation this builds on:**
- `.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-02-PLAN.md` — UDS broker daemon, has per-identity event stream
- `crates/famp/src/cli/broker/accept.rs` — per-client task already does the per-identity routing
- `famp-bus` `BrokerEnv` trait — clean place to attach a subscriber

**Harness mechanism:**
- Claude Code: `RemoteTrigger` deferred tool (runtime supports external prompt injection)
- The `<task-notification>` pattern visible during `/gsd-execute-phase` background-agent completion is the UX target

## Notes

Discovered 2026-04-28 mid-`/gsd-execute-phase 02` after Wave 3 landed the broker daemon. Conversation context: Ben asked "can we add a harness to famp instead of polling await polling that it does now?" after seeing the harness-managed `<task-notification>` flow for the background executor agents.

Key design constraint surfaced in that conversation: **the FAMP wire protocol must stay harness-agnostic**. The adapter is an out-of-protocol convenience, not a protocol concern. This keeps interop with non-Claude-Code clients (the conformance vector pack target) clean.

Tradeoff to revisit at trigger time: should the adapter live in this repo (`crates/famp-watch-claude-code`) or as a separate companion repo? Per-harness adapters multiplying inside the protocol repo could blur the harness-agnostic line.

Related but distinct: MCP push notifications (server-to-client `notifications/...`) are the spec-pure path for MCP-aware clients. Worth scoping in the same milestone but as a parallel track, not a substitute — Claude Code's MCP push handling is limited as of this writing.
