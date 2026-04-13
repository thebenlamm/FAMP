# Phase 4: Minimal HTTP Transport + Cross-Machine Example — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 04-CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-13
**Phase:** 04-minimal-http-transport-cross-machine-example
**Areas discussed:** Server topology + inbox wiring, Address discovery + TLS cert trust, Sig-verify middleware + error responses, Adversarial matrix reuse, Example process model (bonus lock)

---

## Gray Area Selection

Claude presented 4 gray areas for selection. All 4 were selected for discussion.

| Gray Area | Description | Selected |
|---|---|---|
| Server topology + inbox wiring | One axum listener per principal vs path-multiplexed | ✓ |
| Address discovery + TLS cert trust | Where does `https://bob:8443` come from, and how is the cert trusted | ✓ |
| Sig-verify middleware + error responses | Tower layer placement, decode reuse, rejection response shape | ✓ |
| Adversarial matrix reuse strategy | Parallel file vs generic harness vs test-util feature | ✓ |

---

## Server Topology + Inbox Wiring

**User's choice:** One axum server per process, path-multiplexed via `POST /famp/v0.5.1/inbox/:principal`, internal `HashMap<Principal, mpsc::Sender<TransportMessage>>` mirroring Phase 3 `MemoryTransport` mental model.

**Rationale given:**
- Mirrors Phase 3's design cleanly.
- One listener per principal is operationally awkward and buys nothing for personal v0.7.
- Path-multiplexing makes the example easier to run and reason about.

**Locked decisions:** D-A1, D-A2, D-A3, D-A4, D-A5, D-A6.

---

## Address Discovery + TLS Cert Trust

**User's choice:** Keep address config separate from the keyring. New `--addr <principal>=<https-url>` flag alongside existing `--peer`. For TLS: self-signed certs, client trusts via explicit `--trust-cert <path>`. No SPKI pinning or custom verifier.

**Rationale given:**
- Identity/authentication in keyring; network location in CLI/config — clean concern separation.
- `--trust-cert` is the simplest mental model for a personal profile. Easy to explain, easy to debug.
- Avoids dragging certificate identity semantics into this milestone.
- Committed fixture certs are fine for tests; runnable example can generate local self-signed certs at startup.

**Rejected alternatives:**
- Overloading the keyring with address info.
- Sibling `peers.toml` file (deferred — not needed for Phase 4).
- SPKI pinning / dev-only custom rustls verifier.

**Locked decisions:** D-B1, D-B2, D-B3, D-B4, D-B5, D-B6, D-B7, D-B8.

---

## Sig-Verify Middleware + Error Responses

**User's choice:** Tower layer order (outer → inner): 1 MB body limit → signature verification → route dispatch → handler. Middleware stashes decoded/verified `SignedEnvelope` in request extensions. Middleware enforces size + decode + signature only — NOT runtime pipeline logic. Rejections return plain HTTP status + small typed JSON body, NOT a signed FAMP ack.

**Rationale given:**
- Body limit first so sig-verify can't read unbounded bytes before rejecting.
- Keep middleware responsibility narrow (enforce, decode, verify, attach, reject) — runtime glue owns sender cross-check, FSM step, and the full receive pipeline (Phase 3 carryover).
- A signed FAMP ack on failure is the wrong complexity level for this phase; middleware failures happen before the app has accepted the message.
- Simple HTTP error responses are easier to test and debug.

**Example response format:**
```json
{ "error": "unauthorized", "detail": "signature verification failed" }
```
With statuses: 400 malformed/bad envelope, 401/403 for signature/key failures, 413 body too large.

**Rejected alternatives:**
- Middleware re-running `cross_check_recipient` / FSM step (rejected — single source of truth stays in runtime glue).
- Signed FAMP ack rejection response.

**Locked decisions:** D-C1, D-C2, D-C3, D-C4, D-C5, D-C6, D-C7, D-C8.

---

## Adversarial Matrix Reuse Strategy

**User's choice:** One generic transport harness. Common case definitions run against both `MemoryTransport` and `HttpTransport` from the same assertions. 3 cases × 2 transports = 6 rows.

**Rationale given:**
- That matches the "3 cases × 2 transports = 6 rows" framing already locked in.
- Do not create a separate `http_adversarial.rs` that reimplements the same logic.
- Avoids drift where HTTP subtly tests something different from memory.
- Prefer a narrow test support constructor/API over a general-purpose `test-util` feature.

**Shape:**
- Common adversarial case definitions
- Transport-specific setup adapters
- Same assertions reused across both transports

**Rejected alternatives:**
- Parallel `http_adversarial.rs` duplicating Phase 3 test bodies.
- `test-util` feature on `famp-transport-http` mirroring Phase 3 D-D6 (not needed — raw HTTP POSTs are already "anyone can inject").

**Locked decisions:** D-D1, D-D2, D-D3, D-D4, D-D5, D-D6.

---

## Bonus: HTTP Example Process Model

**User-initiated lock** (not one of the original 4 gray areas, but flagged during discussion as "otherwise it will creep"):

**User's choice:** `cross_machine_two_agents` is runnable in two terminals with fixed roles. One process runs alice config; one process runs bob config. No single-binary auto-orchestration in the first cut.

**Rationale given:**
- Auto-orchestrating both peers over HTTP tends to hide transport mistakes behind test harness convenience.
- Fixed roles are the honest cross-machine cycle.

**Locked decisions:** D-E1, D-E2, D-E3, D-E4, D-E5, D-E6, D-E7.

---

## Claude's Discretion

Captured in CONTEXT.md. Summary of items explicitly left to the planner/executor:

- Exact module layout inside `crates/famp-transport-http/src/`
- Exact `reqwest` / `rustls` feature flag selection
- Whether `HttpTransport` owns its server `JoinHandle` (recommended: yes)
- Exact CLI flag names (`--role` preferred)
- Whether `--addr` accepts repeated flags or comma-separated (repeated preferred)
- Whether `rcgen` is dev-dep only (preferred) or also an optional feature
- Exact adversarial directory layout
- Whether the adversarial harness uses `async_trait` or native AFIT (native AFIT preferred)
- Subprocess coordination mechanism for the integration test
- Whether to ship both subprocess + same-process integration tests (recommended for CI stability)
- `reqwest::Client` timeout / HTTP version defaults
- Content-Type header value

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Key items:
- `.well-known` Agent Card distribution (v0.8)
- Cancellation-safe spawn-channel send path (v0.9)
- Pluggable TrustStore / federation credential (v0.8+)
- Sibling `peers.toml` file
- SPKI pinning / custom verifier
- mTLS
- HTTP/2, HTTP/3, QUIC
- `test-util` feature on `famp-transport-http`
- Single-binary auto-orchestrated example
- `stateright` model check
- `famp` CLI subcommands
