# Phase 9: End-to-End Cross-Host Delivery - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-23
**Phase:** 9-End-to-End Cross-Host Delivery
**Mode:** `--auto` (autonomous — all gray areas auto-resolved to the recommended option)
**Areas discussed:** Cross-host topology, Peer-endpoint resolution, Outbound drain, Inbound ingress + verify reconciliation, Local-bus delivery, FSM advancement, Phase gate shape, TLS trust

---

## Cross-host topology — how a name resolves to a remote host (GW-01/GW-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Back the remote principal locally (symmetric stand-in) | Each gateway `back()`s the remote peer's principal as a local stand-in; outbound drains its mailbox, inbound `send`s as it | ✓ |
| Teach the local broker to route by authority/domain | Add domain routing to `famp-bus` | |

**Choice:** Symmetric "back the remote principal locally" model.
**Notes:** Reuses Phase 7's `GatewayRegistry::back` / `ProxiedPrincipal` verbatim — the same PID-carrying UDS stand-in that solves liveness is the drain/deliver point. Rejected broker-routing: violates Design A "zero `famp-bus` change" + Layer-2-only boundary.

---

## Peer-endpoint resolution

| Option | Description | Selected |
|--------|-------------|----------|
| `to_domain` → URL map beside the pinned keyring | Hand-configured domain→gateway-URL table (`peers.toml` or `--peer` flag) | ✓ |
| Directory / discovery | `.well-known` signed directory | |

**Choice:** Hand-configured peer-endpoint map, sibling to `~/.famp/gateway/peers.keyring`.
**Notes:** Own-machines-first ⇒ no discovery. DIR-01 (signed directory) is v1.1. Exact spelling (`peers.toml` vs `--peer` flags) left to planner.

---

## Inbound ingress + verify reconciliation

| Option | Description | Selected |
|--------|-------------|----------|
| Wrap preserved axum/TLS plumbing, route body to gateway `verify_inbound` | Reuse `build_router`/`INBOX_ROUTE`/rustls; bypass `FampSigVerifyLayer`; single trust decision | ✓ |
| Let transport `FampSigVerifyLayer` verify against its own keyring | Two keyrings, two trust decisions | |
| Roll a fresh axum surface | Discard preserved plumbing | |

**Choice:** Reuse transport-http HTTP/TLS scaffolding, single `verify_inbound` authority against the gateway peers keyring.
**Notes:** Phase 8 D-07 locked `verify_inbound` as the single verify authority. Two keyrings would reproduce the split-authority / flat-error class Phase 8 D-08 warns about. Add `famp-transport-http` to `famp-gateway/Cargo.toml` (not currently a dep).

---

## Phase gate shape

| Option | Description | Selected |
|--------|-------------|----------|
| Two-process loopback E2E | Two brokers/sockets + two gateways + loopback HTTPS on one host | ✓ |
| Live two-physical-machine run | Requires two real machines | |

**Choice:** Two-process loopback E2E as the Phase 9 gate.
**Notes:** Mirrors Phase 8's single-machine round-trip. The live two-machine run + `just ci`-gated E2E (TEST-02) + setup guide (DOC-04) are Phase 10.

---

## TLS trust on the cross-host hop

| Option | Description | Selected |
|--------|-------------|----------|
| TLS = channel encryption; Ed25519 signature = trust boundary | Reuse rustls fixture-cert pattern; trust is `verify_inbound` | ✓ |
| Build cert PKI / mutual-TLS trust | Full cert-based peer auth | |

**Choice:** TLS for channel encryption only; the Ed25519 envelope signature is the trust boundary.
**Notes:** Per Recent Decisions — the relay/transport is an availability dependency, never trust. Own-machines-first ⇒ self-signed/pinned cert, no PKI.

---

## Claude's Discretion

- Exact peer-endpoint config surface (`peers.toml` vs `--peer` flags) — planner picks against clap tree + `paths.rs`.
- Which transport-http layers to keep vs. bypass on ingress — planner reconciles against `server.rs`/`middleware.rs`.
- Cert provisioning for loopback — planner confirms against `tls.rs` / `cross_machine_two_agents` fixtures.
- Outbound drain style (blocking `await` vs poll loop per backed principal) — planner picks against `bus_client` API.

## Deferred Ideas

- Live two-machine run + CI-gated two-process E2E → Phase 10 (TEST-02).
- Deferred federation test triage (`_deferred_v1/`) → Phase 10 (TEST-01).
- Two-machine setup guide → Phase 10 (DOC-04).
- Active nonce/replay cache + expiry rejection → v1.1 (INGRESS-01).
- Public relay, signed directory, no-implicit-peering, inbound-taint → v1.1 (RELAY/DIR/PEER/TAINT).
- FAMP-Sec plane → v2.0+ (SEC-01..N).
