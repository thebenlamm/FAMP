# Requirements: FAMP — v1.0 Federation Profile — Gateway Core

**Defined:** 2026-07-23 (supersedes the 2026-06-08 mesh-VPN Gate A draft)
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Milestone goal:** An agent on one of Ben's machines exchanges a signed FAMP envelope with an agent on a second machine he controls, bidirectionally and reliably, over a network he fully controls (direct or a VPN he already runs — no public relay). Closes Gate A; tags `v1.0.0`. Full arc: `~/.claude/plans/first-work-out-the-nested-star.md`.

**Scope discipline:** thin vertical slice. Gateway transport + proxied-principal liveness + two-machine key bootstrap + signed cross-host envelope + reactivated deferred tests. NO relay, public internet, cross-person trust, signed directory, or capability/approval/tool-admission plane. Crypto is already built (Ed25519/INV-10) — do not rebuild it.

## v1 Requirements

### Gateway (GW) — cross-host bridge

- [ ] **GW-01**: A user registers an agent on machine A, addresses an agent on machine B by name/principal, and the message is delivered to B's local bus.
- [ ] **GW-02**: An agent on machine B can reply within the same task/conversation, and the reply is delivered back to machine A.
- [ ] **GW-03**: A full task exchange (`request → commit → deliver → ack`) completes across the two machines with the task FSM advancing correctly on both sides.
- [x] **GW-04**: A single gateway process backs multiple remote principals concurrently with no cross-talk between them.

### Liveness (LIVE) — proxied-principal liveness (the gating fork)

- [x] **LIVE-01**: A gateway-proxied remote principal remains registered/live on the local broker for as long as the gateway process is alive — it is not reaped by the broker's same-host `kill(pid,0)` liveness check.
- [x] **LIVE-02**: When the gateway process exits, its proxied principals are reaped cleanly, leaving no orphan holders.

### Wire (WIRE) — signed cross-host envelope

- [x] **WIRE-01**: Every envelope crossing between machines is Ed25519-signed under the `FAMP-sig-v1\0` domain prefix; an unsigned or signature-invalid envelope is rejected at the receiving gateway before it touches the local bus (INV-10 on the cross-host path).
- [x] **WIRE-02**: The cross-host envelope carries sender/receiver domain + key_id, a nonce, and an expiry, with capability/approval fields omitted when empty — forward-compatible with v1.1/v2.0 without a wire break.

### Trust (TRUST) — two-machine key bootstrap

- [x] **TRUST-01**: A user can export an agent's peer identity on machine A and import it on machine B (and vice versa), establishing mutual Ed25519 key trust via TOFU pinning.
- [x] **TRUST-02**: A message signed by an unknown/unpinned peer key is rejected — no implicit trust.

### Test (TEST) — reactivation & E2E

- [ ] **TEST-01**: The ~27 parked federation tests in `crates/famp/tests/_deferred_v1/` are triaged — still-valid tests run green in CI, obsolete tests are removed with documented rationale.
- [ ] **TEST-02**: A live two-process end-to-end test exercises the full signed cross-host task cycle and runs in `just ci`.

### Docs (DOC)

- [ ] **DOC-04**: A setup guide documents standing up the gateway on two machines — bind address, out-of-band key exchange, and connect/verify.

## v2 Requirements (deferred — v1.1 / v2.0+)

Tracked but not in this milestone's roadmap. See `~/.claude/plans/first-work-out-the-nested-star.md`.

### Open-internet talk (v1.1)

- **RELAY-01**: Two different users' gateways connect out to a dumb relay (availability-only, never trust) for public-internet reachability.
- **DIR-01**: Signed peer directory (`.well-known/famp-directory.json`), JCS + Ed25519 under a domain root key, monotonic serial; doubles as revocation.
- **INGRESS-01**: Protocol-grade ingress — freshness-window + replay-cache (nonce) enforcement at the receiving gateway boundary.
- **PEER-01**: No implicit peering — inbound from an unenrolled domain dropped with no state created.
- **TAINT-01**: Inbound cross-host content surfaced to the receiving agent as untrusted/tainted and persisted with provenance so it never re-enters as clean.

### Security plane (v2.0+, demand-gated)

- **SEC-01..N**: FAMP-Sec §5–§9 — typed body + provenance enforcement, capabilities + invocation binding, abstract operations + MCP binding, tool-admission gateway + custodian, approvals + receipts. Built only on real demand for remote-triggered tools. Spec: `~/Downloads/famp-sec-v1-draft-2.md`.

## Out of Scope

Explicitly excluded from v1.0. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Public-internet relay / NAT traversal | v1.1 — prove the gateway spine on own machines first (controlled network) |
| Cross-person trust bootstrap | v1.1 — v1.0 uses hand-copied keys between machines one person controls |
| Signed peer directory | v1.1 — own-machines uses direct TOFU pin, no directory needed |
| Freshness / replay-cache enforcement | v1.1 protocol-grade ingress — v1.0 signs + verifies signatures; boundary replay defense is an open-internet concern |
| Capability / approval / tool-admission plane | v2.0+ demand-gated — v1.0 is conversation only, no remote-triggered tools |
| Conformance vector pack | Gate B (event-driven, parallel) — ships when a 2nd implementer commits, not on a schedule |

## Traceability

Which phases cover which requirements. Populated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| GW-01 | Phase 9 | Pending |
| GW-02 | Phase 9 | Pending |
| GW-03 | Phase 9 | Pending |
| GW-04 | Phase 7 | Complete |
| LIVE-01 | Phase 7 | Complete |
| LIVE-02 | Phase 7 | Complete |
| WIRE-01 | Phase 8 | Complete |
| WIRE-02 | Phase 8 | Complete |
| TRUST-01 | Phase 8 | Complete |
| TRUST-02 | Phase 8 | Complete |
| TEST-01 | Phase 10 | Pending |
| TEST-02 | Phase 10 | Pending |
| DOC-04 | Phase 10 | Pending |

**Coverage:**

- v1 requirements: 13 total
- Mapped to phases: 13 (100%)
- Unmapped: 0

**Phase summary:**

- Phase 7 — Broker-Liveness Fork + Gateway Skeleton: GW-04, LIVE-01, LIVE-02 (3 reqs)
- Phase 8 — Signed Cross-Host Envelope + Trust Bootstrap: WIRE-01, WIRE-02, TRUST-01, TRUST-02 (4 reqs)
- Phase 9 — End-to-End Cross-Host Delivery: GW-01, GW-02, GW-03 (3 reqs)
- Phase 10 — Test Reactivation + Setup Docs: TEST-01, TEST-02, DOC-04 (3 reqs)

---
*Requirements defined: 2026-07-23*
*Last updated: 2026-07-23 — roadmap created (ROADMAP.md Phases 7–10); traceability populated, 13/13 requirements mapped, 100% coverage.*
