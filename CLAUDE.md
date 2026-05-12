<!-- GSD:project-start source:PROJECT.md -->
## Project

**FAMP — Federated Agent Messaging Protocol (Reference Implementation)**

A Rust reference implementation of FAMP (Federated Agent Messaging Protocol) v0.5 — a protocol defining semantics for communication among autonomous AI agents within a trusted federation. The implementation provides a conformance-grade library covering identity, causality, negotiation, commitment, delegation, and provenance across three protocol layers, plus a reference HTTP transport binding.

**Core Value:** **A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.** If canonicalization or signature verification disagrees, nothing else matters.

### Constraints

- **Tech stack**: Rust (stable, latest). `ed25519-dalek` for signatures, `serde` + custom canonicalizer for RFC 8785 JCS, `proptest` + `stateright` for state-machine model checking, `axum` or `hyper` for HTTP transport reference.
- **Tech stack (deferred)**: No Python/TS bindings in v1; keep FFI surface clean but unwired.
- **Transport**: HTTP/1.1 + JSON over TLS as reference wire; in-process `MemoryTransport` for tests. Other transports live behind the `Transport` trait.
- **Conformance target**: Staged conformance is supported — each milestone tags conformance level achieved; vector pack ships in v1.0 alongside federation gateway.
- **Spec fidelity**: v0.5.1 fork is the authority for this implementation. All diffs from v0.5 documented with reviewer rationale.
- **Security**: Every message signed (INV-10); unsigned messages rejected. Ed25519 non-negotiable. Domain separation prefix added in v0.5.1 fork.
- **Developer onboarding**: Rust toolchain install is Phase 0; assume zero prior Rust experience.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

Full crate selection rationale, alternatives, version compatibility, and beginner friction map: see `research/STACK.md`.

**TL;DR:** ed25519-dalek 2.2, serde_jcs 0.2 (MEDIUM confidence — gate with RFC 8785 test vectors), serde_json 1.x, uuid 1.23 (v7), base64 0.22, sha2 0.11, axum 0.8, reqwest 0.13 (rustls backend), rustls 0.23, tokio 1.51, thiserror 2/anyhow 1, proptest 1.11, stateright 0.31, insta 1.47, cargo-nextest, just.
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

**MCP tool surface changes:** When modifying `crates/famp/src/cli/mcp/server.rs` (tool schemas, tool descriptors, new tools), run `just install` before closing the PR. The installed binary at `~/.cargo/bin/famp` is what every agent session reads — `target/release/famp` is not the deployment target.
<!-- GSD:conventions-end -->

## Listen Mode

Listen mode is ON BY DEFAULT for MCP `famp_register` calls (as of 2026-05-12). Agents auto-wake on inbound messages without an explicit flag:

```
famp_register({identity: "dk"})              // listen mode ON (default)
famp_register({identity: "dk", listen: false}) // opt out for general-purpose windows
```

When listen mode is active, the Stop hook (`~/.claude/hooks/famp-await.sh`) blocks after each turn waiting for an inbound FAMP message (up to 23h). When a message arrives, Claude wakes automatically and receives: `"New FAMP message from <sender>. Call famp_inbox to read it."` — then calls `famp_inbox` to retrieve the content.

**Flipping listen mode without re-registering:** Use `famp_set_listen({listen: true|false})`. This mutates the canonical holder's listen flag in place — no mailbox replay, no new identity binding. Use this when a window registered with the wrong mode, or when an interactive window needs to toggle into listen mode for a long-running peer conversation.

**Opt out (`listen: false`):** The window registers but stays idle between turns. Check inbox on demand by prompting the agent: "check your famp messages" → agent calls `famp_inbox`. Use this for general-purpose dev windows where auto-wake would be intrusive.

**CLI surface (unchanged):** `famp register --as <name>` still defaults to `listen: false` — the default flip applies only to the MCP `famp_register` tool. The bus wire frame is identical either way; only the surface-level default differs.

**Context cost and tool sequencing:** See [`docs/CLAUDE-CODE-CONTEXT-GUIDE.md`](docs/CLAUDE-CODE-CONTEXT-GUIDE.md) for the two retrieval flows, task_id resolution, and how to avoid the double-print pattern that doubles context cost per received message.

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

**FAMP today is local-first** (v0.9): a UDS-backed broker for same-host agent
messaging. **FAMP at v1.0 is federated**: cross-host messaging via
`famp-gateway` wrapping the local bus. See [ARCHITECTURE.md](ARCHITECTURE.md)
for the full layered model (Layer 0 protocol primitives -> Layer 1 local bus ->
Layer 2 federation gateway).

In v0.8 the federation transport used `famp listen` HTTPS daemons with
TOFU-pinned peers; v0.9 replaces this with the local bus. Every federation
wire envelope stayed Ed25519-signed over canonical JSON under the
`FAMP-sig-v1\0` domain prefix (INV-10). 5-state task FSM (`famp-fsm`):
REQUESTED -> COMMITTED -> {COMPLETED | FAILED | CANCELLED}, terminals
absorbing.

Note: as of v0.8.x (the session-bound MCP identity bridge phase), the
`famp mcp` server reads identity from session state via `famp_register`,
not from `FAMP_HOME`. The v0.8 federation transport used `FAMP_HOME` per
identity; v0.9's local bus collapses this distinction.

**v0.9 shipping path:** collapse same-host agents onto a single
UDS-backed broker; drop crypto on the local path; treat federation
(cross-host) as a v1.0 gateway that wraps the bus. IRC-style channels,
durable per-name mailboxes, stable MCP tool surface across v0.8 / v0.9 / v1.0.

**v1.0 readiness trigger (named):** v1.0 federation milestone fires
when Sofer (or a named equivalent) runs FAMP from a different machine
and exchanges a signed envelope. If 4 weeks pass after v0.9.0 ships
with no movement on this trigger, federation framing is reconsidered.
Concrete forcing function for the local-case-black-hole risk; the
conformance vector pack ships at the same trigger (deferred from
v0.5.1 wrap, see `.planning/WRAP-V0-5-1-PLAN.md` DEFERRED banner).

Full write-up in [`ARCHITECTURE.md`](ARCHITECTURE.md) and the design spec
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
Pre-v0.9 scaffolding moved to
[`docs/history/v0.9-prep-sprint/famp-local/famp-local`](docs/history/v0.9-prep-sprint/famp-local/famp-local).

**When working here:** protocol-primitive crates (`famp-canonical`,
`famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope`) are
transport-neutral and reused across both v0.9 and v1.0. Transport crates
(`famp-transport-http`, `famp-keyring`) are v1.0-federation internals —
don't conflate them with the primitive layer.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
