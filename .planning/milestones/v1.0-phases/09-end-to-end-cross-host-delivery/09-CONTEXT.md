# Phase 9: End-to-End Cross-Host Delivery - Context

**Gathered:** 2026-07-23
**Status:** Ready for planning

> **[--auto]** Discussion ran in autonomous mode. Every gray area below was
> auto-resolved to the recommended option and logged inline. Ben should skim
> `<decisions>` before `/gsd-plan-phase 9` — any decision he wants changed is a
> one-line edit here, then re-plan.

<domain>
## Phase Boundary

Make the **live bidirectional cross-host delivery cycle** real. Phases 7 and 8
built the spine (proxied-principal liveness) and the parts (signed wire format +
TOFU trust + a pure `verify_inbound`). Phase 9 composes them into the actual
product promise: a user on machine A addresses an agent on machine B by
name/principal and a full `request → commit → deliver → ack` task exchange
completes through the gateway, FSM advancing on **both** sides.

Concretely, this phase makes three things true:

1. A message A→B addressed by principal is delivered into **B's local bus
   mailbox** through the gateway (GW-01).
2. B's reply within the same task/conversation is delivered back into **A's
   local bus mailbox** (GW-02).
3. A full `request → commit → deliver → ack` cycle completes across the two
   sides, with the task FSM advancing to a terminal state on both, **observable
   via `famp inspect tasks`** on each side (GW-03).

**This is the phase that wires the transport.** Phase 8 explicitly delivered
`verify_inbound` as a pure in-process function and left the note that "the
gateway has no transport/HTTP ingress yet — that's Phase 9." Phase 9 stands up
the gateway's inbound HTTP listener and outbound HTTP client, bridges the
verified envelope onto the local UDS bus, and proves the round trip.

**Still own-machines-first (v1.0).** Two endpoints Ben controls, hand-copied
keys (Phase 8's export/import), full network control, **no public relay**. The
automated phase gate is a **two-process loopback E2E** (two brokers on distinct
socket paths + two gateways + HTTP over loopback), mirroring Phase 8's
single-machine-proves-the-logic pattern. The live two-physical-machine run and
the `just ci` two-process E2E test belong to **Phase 10** (TEST-02, DOC-04).

**Explicitly out of scope** (deferred per PROJECT.md / REQUIREMENTS.md v2):
public-internet relay (RELAY-01), signed peer directory (DIR-01), active
replay-cache / freshness enforcement (INGRESS-01), no-implicit-peering
(PEER-01), inbound-taint provenance (TAINT-01), and the FAMP-Sec plane
(SEC-01..N). The nonce/expiry fields ride the wire (Phase 8, D-04) but Phase 9
still **does not** build a replay cache or reject on expiry.

</domain>

<decisions>
## Implementation Decisions

### Cross-host topology — how a name resolves to a remote host (GW-01/GW-02)
- **D-01 [auto → recommended]:** **Symmetric "back the remote principal
  locally" model.** Each gateway `back()`s the *remote* peer's principal as a
  local stand-in on its own bus (reusing Phase 7's `GatewayRegistry::back` /
  `ProxiedPrincipal`, which already carries the gateway's own PID for
  LIVE-01/02). On machine A, the gateway backs principal `bob` (B's agent); when
  A's local agent sends to `bob`, the message lands in the gateway-backed
  mailbox for `bob` on A's bus, the gateway drains it, wraps + signs the
  cross-host envelope, and ships it over HTTP to B's gateway. B's gateway
  verifies it and delivers into B's real `bob` mailbox. Rationale: this reuses
  the Phase 7 liveness mechanism verbatim — the same UDS stand-in that keeps a
  remote principal "live" is also the drain point for outbound traffic. No new
  bus primitive.
  *Rejected:* teaching the local broker to route by `authority`/domain (a
  `famp-bus` change — violates the "zero `famp-bus` source change" Design A
  thesis and the Layer-2-only boundary).

- **D-02 [auto → recommended]:** **`to_domain` → remote gateway URL via a small
  peer-endpoint map, sibling to the pinned keyring.** The envelope already
  carries `from_domain`/`to_domain` (Phase 8, D-03 = `Principal.authority`). The
  gateway maps `to_domain` → the remote gateway's HTTPS base URL via a
  hand-configured peer-endpoint table stored alongside
  `~/.famp/gateway/peers.keyring` (recommend `~/.famp/gateway/peers.toml` or a
  `--peer <domain>=<url>` flag; planner picks the exact spelling against the
  existing clap tree + `paths.rs`). Own-machines-first ⇒ endpoints are
  hand-entered, no discovery/directory (DIR-01 is v1.1).

### Outbound drain (egress on the sender's gateway)
- **D-03 [auto → recommended]:** **Drain the backed principal's mailbox via its
  existing UDS `ProxiedPrincipal` connection; ship byte-exact.** The gateway
  awaits/inbox-drains each locally-backed remote principal, and for each drained
  envelope: resolve `to_domain` → URL (D-02), populate + sign the federation
  fields if not already cross-host-signed, and POST to the remote gateway's
  inbox. The gateway is **content-transparent** — it never rewrites `task_id`,
  `MessageClass`, or body; it only adds the outer federation fields + INV-10
  signature over the single canonical form (Phase 8, D-01: one envelope, one
  signature). Preserving `task_id`/class byte-exact is what makes GW-03's FSM
  advance on both sides.

### Inbound ingress (receiver's gateway)
- **D-04 [auto → recommended]:** **Reuse `famp-transport-http`'s axum server +
  TLS + body-limit scaffolding, but route the raw body through the gateway's own
  `verify_inbound` against the gateway peers keyring — do NOT double-verify via
  the transport's `FampSigVerifyLayer`.** Phase 8 locked `verify_inbound(bytes,
  &keyring)` (D-07) as the single verify authority, reading
  `~/.famp/gateway/peers.keyring`. Phase 9 mounts the preserved
  `build_router` / `INBOX_ROUTE` (`POST /famp/v0.5.1/inbox/{principal}`) + rustls
  server for the HTTP/TLS plumbing, but the handler feeds the body to
  `verify_inbound` (not the transport's middleware keyring) so there is exactly
  one trust decision, made against the gateway's pinned peers. Planner reconciles
  the seam (which transport layers to keep vs. bypass) against
  `server.rs` / `middleware.rs`. Add `famp-transport-http` to
  `famp-gateway/Cargo.toml` (not currently a dep).
  *Rejected:* rolling a fresh axum surface (throws away preserved,
  interop-tested TLS/body-limit plumbing the roadmap says to wrap); OR letting
  `FampSigVerifyLayer` verify against a second keyring (two sources of trust
  truth — the exact flat-error / split-authority class Phase 8 D-08 warns about).

- **D-05 [auto → recommended]:** **Verified envelope → local bus via the backed
  *sender* stand-in.** On B, the gateway backs the remote *sender* principal
  (`alice`, A's agent) as a local stand-in and uses that `ProxiedPrincipal`'s
  UDS connection to `send` the verified envelope to the local recipient (`bob`).
  This is the mirror of D-01 and keeps delivery on the existing bus `send`
  path — the local broker's normal task-record machinery advances the FSM,
  making it visible to `famp inspect tasks` (GW-03). A rejected envelope
  (`invalid_signature` / `unpinned_key`, Phase 8 D-08) produces **zero** bus
  writes and surfaces as an HTTP 4xx.

### FSM advancement + task continuity (GW-03)
- **D-06 [auto → recommended]:** **The gateway is FSM-transparent; both local
  brokers own their own FSM.** Because real signed envelopes (unchanged
  `task_id` + `MessageClass`) flow onto each local bus, each side's existing
  broker/taskdir advances its own task record through
  `REQUESTED → COMMITTED → {COMPLETED|FAILED|CANCELLED}` with no gateway-side FSM
  logic. The gateway relays; the brokers advance. Verification asserts the
  terminal state on both sides via `famp inspect tasks --id <task_id>`.

### Phase gate shape
- **D-07 [auto → recommended]:** **Two-process loopback E2E is the Phase 9
  gate.** Two brokers (distinct `FAMP_HOME` / socket paths), two gateways, real
  signed envelopes over loopback HTTPS, driving a full `request → commit →
  deliver → ack` cycle and asserting terminal FSM state on both `famp inspect
  tasks`. Uses `ChildGuard` RAII for every spawned broker/gateway child (memory:
  test-child-guard-convention). The **live two-physical-machine run**, the
  **`just ci`-gated** two-process E2E (TEST-02), and the **setup guide**
  (DOC-04) are Phase 10 — Phase 9 proves the composition works; Phase 10 makes
  it a permanent regression net + onboarding path.

### TLS on the cross-host hop
- **D-08 [auto → recommended]:** **TLS is channel encryption, not the trust
  boundary — the Ed25519 envelope signature is.** Reuse `famp-transport-http`'s
  existing rustls setup (`tls.rs` / `tls_server.rs`, the fixture-cert pattern
  from `cross_machine_two_agents`). Own-machines-first ⇒ self-signed / pinned
  cert, no PKI. Per Recent Decisions ("the relay is an availability dependency,
  never a trust one") the actual trust decision is `verify_inbound`'s Ed25519
  check against the pinned keyring; TLS just protects the wire. Planner confirms
  the exact cert-provisioning path against `tls.rs`.

### Claude's Discretion
- Exact peer-endpoint config surface (D-02): `~/.famp/gateway/peers.toml` vs
  `--peer <domain>=<url>` flags vs both — planner picks against the clap tree in
  `cli/mod.rs` + `paths.rs`.
- Which `famp-transport-http` layers to keep vs. bypass on ingress (D-04) —
  planner reconciles against `server.rs` / `middleware.rs`.
- Cert provisioning for own-machines loopback (D-08) — planner confirms against
  `tls.rs` / the `cross_machine_two_agents` fixture certs.
- Whether outbound drain uses `famp await`-style blocking or a poll loop per
  backed principal — planner picks against the `bus_client` await/inbox API.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase intent & requirements
- `.planning/ROADMAP.md` §"Phase 9: End-to-End Cross-Host Delivery" — goal + 3
  success criteria (the acceptance contract) and the Phase 10 boundary.
- `.planning/REQUIREMENTS.md` — GW-01, GW-02, GW-03 exact text; the v2
  deferral list (RELAY/DIR/INGRESS/PEER/TAINT/SEC) that bounds what NOT to build.
- `.planning/PROJECT.md` §"Current Milestone: v1.0 Federation Profile — Gateway
  Core" — own-machines-first thesis, "no public relay," "Explicitly NOT v1.0."
- `.planning/phases/08-signed-cross-host-envelope-trust-bootstrap/08-CONTEXT.md`
  — Phase 8's locked wire + trust decisions (D-01 one-envelope-one-signature,
  D-07 pure `verify_inbound`, D-08 `invalid_signature` vs `unpinned_key` split)
  that Phase 9 composes.
- `ARCHITECTURE.md` — Layer 0 primitive / Layer 1 bus / Layer 2 gateway model;
  `FAMP-sig-v1\0` domain-prefix + INV-10 invariant statements; the Layer-2-only
  boundary (no `famp-bus` change).

### Gateway (Phases 7 + 8 — build the transport onto this)
- `crates/famp-gateway/src/lib.rs` — public surface: `GatewayRegistry`,
  `ProxiedPrincipal`, `verify_inbound`, `RejectReason`.
- `crates/famp-gateway/src/registry.rs` — `GatewayRegistry::back` /
  `get` / `names` (the local stand-in demux, GW-04); Phase 9's outbound-drain
  and inbound-deliver both hang off backed principals (D-01/D-03/D-05).
- `crates/famp-gateway/src/principal.rs` — `ProxiedPrincipal::register` / `name`
  (the PID-carrying UDS connection).
- `crates/famp-gateway/src/verify.rs` — `verify_inbound(bytes, &keyring)`; the
  ingress handler calls this (D-04).
- `crates/famp-gateway/src/main.rs` — current bin parks after `back()`;
  Phase 9 adds the inbound listener + outbound drain loops here (or a spawned
  task set).
- `.planning/phases/08-signed-cross-host-envelope-trust-bootstrap/08-VERIFICATION.md`
  — what Phase 8 delivered and the explicit "no transport ingress yet" note.

### Preserved HTTP transport (wrap, do not rebuild)
- `crates/famp-transport-http/src/server.rs` — `build_router`, `INBOX_ROUTE`
  (`/famp/v0.5.1/inbox/{principal}`), `InboxRegistry`, `inbox_handler` (D-04).
- `crates/famp-transport-http/src/transport.rs` — `HttpTransport` client send
  path (outbound egress, D-03).
- `crates/famp-transport-http/src/middleware.rs` — `FampSigVerifyLayer` (the
  transport's own verify — **bypassed** in favor of gateway `verify_inbound`,
  D-04; read to understand what you're replacing).
- `crates/famp-transport-http/src/tls.rs`, `tls_server.rs` — rustls client/server
  setup + cert helpers (D-08).
- Escape-hatch tag `v0.8.1-federation-preserved` — the pre-deletion commit the
  transport was preserved at.

### Reused primitives (call, do not re-derive)
- `crates/famp-envelope/src/{wire,envelope,version}.rs` — cross-host envelope
  sign/encode/decode; federation fields added in Phase 8; INV-10 signature site.
- `crates/famp-crypto/src/{keys,verify}.rs` — `verify_strict` under
  `FAMP-sig-v1\0`.
- `crates/famp-keyring/src/lib.rs` — the pinned gateway peers keyring
  `verify_inbound` reads.
- `crates/famp-core/src/identity.rs` — `Principal { authority, name }` →
  `from_domain`/`to_domain` and the D-02 endpoint-map key.
- `crates/famp/src/bus_client/mod.rs` — `send_recv` / `send_recv_abortable`,
  `resolve_sock_path` (the local-bus send path D-05 uses).

### Deferred-test corpus (Phase 10 owns triage, but read for wire behavior)
- `crates/famp/tests/_deferred_v1/` — `e2e_two_daemons.rs.deferred`,
  `send_deliver_sequence.rs`, `send_new_task.rs`, `peer_import.rs`, etc. describe
  the intended cross-host cycle behavior Phase 9 must reproduce live.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `GatewayRegistry::back` / `ProxiedPrincipal` (Phase 7): the same PID-carrying
  UDS stand-in that solves liveness is Phase 9's outbound drain point (D-01) and
  inbound delivery point (D-05) — no new bus primitive needed.
- `verify_inbound` + `RejectReason` (Phase 8): the ingress trust decision is a
  finished pure function; Phase 9 only feeds it the HTTP body (D-04).
- `famp-transport-http` `build_router` / `INBOX_ROUTE` / `HttpTransport` /
  rustls helpers (v0.8.1-preserved): the entire HTTP+TLS plumbing exists; Phase 9
  wraps it (D-04/D-08). **Not currently a `famp-gateway` dependency — add it.**
- `bus_client::send_recv` + `resolve_sock_path`: the local-bus delivery path.

### Established Patterns
- **Zero `famp-bus` change / Layer-2-only** (Design A thesis): all routing lives
  in the gateway; the local broker stays domain-agnostic (D-01 rejected the
  broker-routing alternative).
- **Content-transparent relay** (data-as-input, memory): the gateway relays
  `task_id`/`MessageClass`/body byte-exact and never forges synthetic messages
  (D-03/D-06) — that byte-transparency is what makes FSM advance on both sides.
- **Single verify authority** (Phase 8 D-07/D-08): one keyring, one
  `verify_inbound`, `invalid_signature` vs `unpinned_key` split — do not add a
  second trust decision via the transport middleware (D-04).
- **`ChildGuard` RAII** for every spawned broker/gateway child in tests (memory:
  test-child-guard-convention) — the two-process E2E spawns four+ children.

### Integration Points
- Outbound: backed remote principal's mailbox (A's bus) → gateway drain →
  sign federation fields → `HttpTransport` POST → B's gateway inbox (D-01/D-03).
- Inbound: B's gateway inbox handler → `verify_inbound(body, gateway_keyring)` →
  backed sender stand-in `send` → B's local `bob` mailbox (D-04/D-05).
- FSM: real signed envelopes on each local bus → each broker's taskdir advances
  its own record → `famp inspect tasks` shows terminal state (D-06).
- Config seam: `to_domain` → remote gateway URL map beside
  `~/.famp/gateway/peers.keyring` (D-02).

</code_context>

<specifics>
## Specific Ideas

- Prove GW-01/02/03 with a **two-process loopback E2E** (not two physical
  machines): two brokers on distinct socket paths / `FAMP_HOME`s, two gateways,
  loopback HTTPS between them, one full `request → commit → deliver → ack` task,
  asserting the terminal FSM state on both sides via `famp inspect tasks --id`.
  This mirrors Phase 8's single-machine round-trip: prove the composition here;
  the real two-machine run + CI-gated E2E + setup guide are Phase 10.
- Keep the gateway **content-transparent** end-to-end so a Phase 9 failure is
  unambiguously in the transport/routing layer, never in envelope semantics —
  the same "don't make failures ambiguous between spine and new layer" discipline
  Phase 8 used for deferring active replay/expiry.

</specifics>

<deferred>
## Deferred Ideas

- **Live two-physical-machine run + `just ci`-gated two-process E2E** — Phase 10
  (TEST-02); Phase 9 delivers the loopback composition, Phase 10 makes it a
  permanent regression net.
- **Deferred federation test triage** (`crates/famp/tests/_deferred_v1/`, ~27
  tests) — Phase 10 (TEST-01).
- **Two-machine setup guide** (bind address, out-of-band key exchange,
  connect/verify) — Phase 10 (DOC-04).
- **Active nonce/replay cache + expiry rejection** — v1.1 (INGRESS-01); fields
  ride the wire now, enforcement is the public-internet layer.
- **Public-internet dumb relay, signed peer directory, no-implicit-peering,
  inbound-taint provenance** — v1.1 (RELAY-01/DIR-01/PEER-01/TAINT-01).
- **FAMP-Sec capability/approval/tool-admission plane** — v2.0+ (SEC-01..N).
- **Peer/endpoint discovery** — v1.1 directory; Phase 9 uses a hand-configured
  domain→URL map (own-machines-first).

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 9-End-to-End Cross-Host Delivery*
*Context gathered: 2026-07-23*
