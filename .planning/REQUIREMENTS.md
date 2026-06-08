# Requirements: v1.0 Federation Profile — Gate A (Gateway)

**Milestone goal:** Ben's Claude Code on one host exchanges signed FAMP envelopes with another host's Claude Code, over a mesh VPN, via a new `famp-gateway` (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`.

**Scope discipline:** thin vertical slice. Gateway transport + reachability + trust bootstrap + reactivated deferred tests. NO Agent Cards, `.well-known` distribution, negotiation, delegation, provenance, or conformance vector pack (those are later v1.x / Gate B). Crypto is already built (Ed25519/INV-10) — do not rebuild it.

**Sequence (hard ordering):** prove the gateway on Ben's own laptop ↔ home machine first (FED-02), then add the friend (FED-03).

---

## v1.0 Requirements

### Gateway (GW)
- [ ] **GW-01**: A `famp-gateway` process forwards a local-bus envelope addressed to a remote peer over FAMP-over-HTTPS to that peer's gateway.
- [ ] **GW-02**: An envelope arriving at the gateway's HTTPS listener is signature-verified (Ed25519/INV-10 over canonical JSON under the `FAMP-sig-v1\0` domain prefix) and delivered into the local bus mailbox for the addressed local identity.
- [ ] **GW-03**: The gateway rejects any inbound envelope whose signature fails verification or whose sending key is not TOFU-pinned in the keyring — the unsigned/forged path stays closed on the cross-host hop.
- [ ] **GW-04**: A user sends to a remote peer using a name that resolves to a remote gateway; the local send surface (CLI + MCP `famp_send`) is unchanged.

### Reachability (RCH)
- [ ] **RCH-01**: The gateway HTTPS listener binds to a configured address (a tailnet IP); reachability is documented as requiring a mesh VPN (Tailscale/WireGuard). No NAT-traversal / relay / STUN / TURN code ships.
- [ ] **RCH-02**: A send to an unreachable peer surfaces an actionable error that names the mesh-VPN reachability requirement — never a silent drop.

### Trust bootstrap (TRUST)
- [ ] **TRUST-01**: A user can `peer_export` their identity's public key + gateway address as a shareable blob for an out-of-band channel.
- [ ] **TRUST-02**: A user can `peer_import` a peer's blob, which TOFU-pins the peer's key; a later key change for an already-pinned peer is rejected.

### Federation acceptance (FED)
- [ ] **FED-01**: The ~27 deferred tests in `crates/famp/tests/_deferred_v1/` are reactivated and green in CI (`just ci`).
- [ ] **FED-02**: Two gateways on Ben's own two machines exchange a signed envelope end-to-end over the tailnet (the Gate A proof — laptop ↔ home machine; keys copied directly, full network control).
- [ ] **FED-03**: Friend-to-friend — a cross-person signed envelope exchange succeeds after out-of-band `peer_export` / `peer_import` (the milestone's ultimate success criterion).

---

## Future Requirements (deferred to v1.x)

- Agent Cards + federation credentials; `.well-known` card distribution.
- Negotiation / counter-proposal; the three delegation forms; provenance graph; extensions registry.
- Trust-bootstrap UX polish beyond the rough export/import blob (TRUST-01/02 ship rough in v1.0).
- SEED-002: push-notification harness adapter (`famp watch --notify`) — orthogonal to the federation transport; deferred 2026-06-08.
- Replay defense beyond what the preserved transport already provides.

## Out of Scope (this milestone)

- **NAT traversal / relay / STUN / TURN** — reachability is pushed below FAMP onto a mesh VPN (decision 2026-06-08, `project_v10_reachability_meshvpn`). Building relay infra before two friends exchange one message is the over-engineering trap.
- **Conformance vector pack** — Gate B, not Gate A. Event-driven on a 2nd implementer committing to interop (`WRAP-V0-5-1-PLAN.md`; SEED-001 is its serde_jcs gate). Shipping vectors when both sides run Ben's code is theatre.
- **Re-deriving crypto** — Ed25519 sign/verify over canonical JSON is already built and preserved (`v0.8.1-federation-preserved`).

## Traceability

*(Filled by roadmap — maps each REQ-ID to its phase.)*
