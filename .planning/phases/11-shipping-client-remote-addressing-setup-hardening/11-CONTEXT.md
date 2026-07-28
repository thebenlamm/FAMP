# Phase 11: Shipping-Client Remote Addressing + Setup Hardening - Context

**Gathered:** 2026-07-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Make a **shipping** FAMP client (`famp send` / `famp_send`) able to address a
**remote** principal and drive a signed cross-host delivery — replacing the
hand-written injector the Gate A dogfood had to use — and correct the 8 setup
defects that same dogfood surfaced. This is the last gap between "the gateway
wire is proven bidirectionally" (Phase 9/10, done) and "v1.0.0 is tagged."

**In scope:** the C2/C5 sender-side split-addressing fix; the transport
error-chain fix; the `docs/GATEWAY-SETUP.md` corrections; regenerating the
committed TLS fixtures + a macOS CI leg; a shipping-surface integration test
(retiring the throwaway injector); and the final live re-run of the Gate A
dogfood with the real client.

**Out of scope (deferred to v1.1/v2.0 per REQUIREMENTS.md):** relay,
public-internet reachability, cross-person trust, signed directory,
replay-cache/freshness enforcement (nonce/expiry are format-validated only,
per the Phase 8 decision), capability/approval/tool-admission plane. Do NOT
reopen the v0.9 unsigned-local-bus decision.
</domain>

<decisions>
## Implementation Decisions

### Addressing model (SETTLED by the external design pass — do not re-litigate)
- **D-01 (C5 pass-through for `to`):** In `crates/famp/src/cli/send/mod.rs::build_envelope_value`, make the target conditional — if the `--to` string already parses as a `Principal` (i.e. `agent:<domain>/<name>`), use it **verbatim** as the envelope `to`; otherwise wrap in `agent:local.bus/<name>` as today. One chokepoint covers `famp send`, `/famp-send`, and the MCP tool; no new required flag; `--to bob` unchanged; scales to N peers. (`--to <name> --domain <domain>` sugar is acceptable but the full-principal form is the primary.)
- **D-02 (BOTH `from` and `to` must be domain-qualified):** A `to`-only rewrite is INSUFFICIENT and strictly worse — ingress verifies on `from` (`crates/famp-gateway/src/verify.rs:62-63`, `peek_sender` returns `from`); a `local.bus` `from` → `UnpinnedKey`, trading `UnknownRecipient` for a symptom that looks like a trust-bootstrap bug. So a remote send MUST stamp `from = agent:{own-domain}/{identity}` too. (Proven by zed's source control during the dogfood.)
- **D-03 (typed unsigned `request`, NO local crypto):** Remote sends emit a typed `RequestBody` envelope constructed via the sanctioned sign-then-strip / BUS-11 pattern (sign with a throwaway key, strip the signature → unsigned on the local bus; the gateway re-signs at egress). This drives the FSM WITHOUT reopening the unsigned-local-bus decision. The E2E and the wire-proof injector both do exactly this — mirror their shape.
- **D-04 (gate the class upgrade on remote):** Bare-name local sends stay class `audit_log` (unchanged local-chat behavior — do NOT make all local chat fire the FSM). Only domain-qualified (remote) sends emit the typed `request`.

### OPEN — resolve in research/planning (ADDR-03, the one genuinely unresolved sub-problem)
- **D-05 (own-domain source for `from`):** Neither the CLI nor `famp-gateway` has an own-domain input today (`famp-gateway` arg surface = `--socket --listen --tls-cert --tls-key --peer --trust-cert` + positional names; no `--domain`). AND `--as agent:<domain>/<name>` cannot carry it because `--as` becomes the broker `Hello{bind_as}`, which is charset-validated `[A-Za-z0-9._-]+` (rejects `:` `/`). Candidate resolutions for research to pick (with invariant analysis):
  - **(a)** a host-level own-domain config both the CLI and gateway read (single source of truth);
  - **(b)** add `--domain` to `famp-gateway` and have **egress** rewrite a `local.bus` `from`/`to` to `agent:{own-domain|peer-domain}/{name}` *before* signing (localizes the fix to the gateway; the CLI change shrinks to just accepting a domain-qualified `to`);
  - **(c)** derive own-domain from the gateway's `peer export` identity/label.
  Load-bearing coupling to preserve: whatever domain lands in `from` MUST equal the label the peer pinned under (today set in two unrelated places with nothing enforcing agreement).

### Setup-guide + code + test hardening (the 8 Gate A findings — see canonical refs)
- **D-06 (transport error chain, OBS-01) — SEQUENCE THIS FIRST:** `crates/famp-transport-http/src/error.rs:63` Display=`"reqwest failure"` + `egress.rs:211` `e.to_string()` discard the `#[source]`. Log the full `.source()` chain (or `{e:?}`). This masked findings #5, #6, #8 — each was a one-line diagnosis that cost a full round-trip ONLY because the log said "reqwest failure". It is the cheapest fix on the list AND the force-multiplier on every other debug in this phase (zed's explicit recommendation, and mine — the fix's own two-machine re-test will be far cheaper to debug with it in place). Plan it as the first plan/wave.
- **D-07 (GATEWAY-SETUP.md, DOC-05):** correct all 8 findings — §4 back the remote principal; §3 pin under the sender agent principal; state pin-before-launch / no-hot-reload; warn on the duplicate-pubkey keyring brick; move the "ready" line to AFTER keyring load; replace "self-signed is fine" with the CA:FALSE+serverAuth cert recipe that works on BOTH platforms; document macOS host-firewall pre-auth. Also strengthen the DOC-04 accuracy gate beyond flag-grep (it can't catch semantic inversion, ordering, keyring bricking, or cert policy).
- **D-08 (fixtures + CI, TEST-03):** regenerate `crates/famp/tests/fixtures/cross_machine/*` to CA:FALSE+serverAuth EKU (they are currently EC/CA-shaped in a way that's Linux-conditionally-green); add a macOS CI leg so the Apple-verifier path is exercised.
- **D-09 (shipping-surface test, TEST-03):** add an integration test that drives the FIXED `famp send` cross-host, plus a NEGATIVE test that a `local.bus`-authority envelope through the federated path yields a typed error (not a silent drop). DELETE the throwaway artifacts once it exists: `crates/famp-gateway/tests/wire_proof_inject.rs` and `crates/famp-transport-http/examples/probe_tls.rs`.

### Claude's Discretion
- Exact flag ergonomics (`--to agent:...` vs `--domain`), the precise own-domain config key/name, and test file layout — planner/researcher decide, subject to D-01..D-05.

### Definition of Done (UAT-01)
- `famp send --to agent:<peer-domain>/<name>` from one machine delivers into the real remote agent's mailbox with the task FSM reaching a terminal state, verified by **re-running the Gate A two-machine dogfood with NO injector** (opus laptop ↔ zed server, or equivalent). That live re-run is the final human gate before tagging v1.0.0.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design verdict (settled — consume, do not re-derive)
- `.planning/DESIGN-BRIEF-v1-remote-addressing.md` — the full finding + constraint + candidate-fix analysis (C1–C5), corrected with zed's source control (from+to must both be rewritten; own-domain gap; pass-through C5).
- `.planning/phases/11-shipping-client-remote-addressing-setup-hardening/DESIGN-RESEARCH-B-grounded.md` (was ~/Downloads/deep-research-report-4.md) — grounded external design pass (recommends C2/C5 split-addressing; invariant analysis A–E). Trustworthy.
- `.planning/phases/11-shipping-client-remote-addressing-setup-hardening/DESIGN-RESEARCH-A-verdict-only.md` (was ~/Downloads/FAMP Addressing Fix Evaluation.md) — second external pass; **verdict is fine but its code sketch is HALLUCINATED** (`EnvelopeBuilder`/`class("Request")`/`payload_json` do not exist — the real code is typed `UnsignedEnvelope<RequestBody>`). Ignore its code.

### Gate A dogfood record (the 8 findings + the honest UAT outcome)
- `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` — status=failed-as-guide-test / wire-proven-bidirectionally; the 8 findings + the canonical cert recipe + the recommended §5 pre-flight discipline.

### Source (ground truth for the fix)
- `crates/famp/src/cli/send/mod.rs` — `build_envelope_value` (~L413): the `local.bus` hardcode + `audit_log` stub. THE primary edit site (D-01/D-03/D-04).
- `crates/famp-gateway/src/verify.rs:62-63` + `crates/famp-gateway/src/peek.rs` — ingress verifies on `from` (proves D-02).
- `crates/famp-gateway/src/egress.rs` (`relay_one` ~L191, `sign_federation_fields` ~L89) — routes on the envelope `to`; only ADDS `from_domain`/`to_domain`. Relevant to D-05 option (b).
- `crates/famp-gateway/src/main.rs:255` — the `agent:{domain}/{name}` peer route-map cross-product.
- `crates/famp-transport-http/src/error.rs:63` + `crates/famp-gateway/src/egress.rs:211` — the error-chain swallow (D-06).
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` — GROUND TRUTH: `send_bus_envelope` (~L519), `build_request`/`unsigned_value` helpers, domain-qualified `ALICE`/`BOB` constants. The fixed `famp send` must produce this envelope shape. Must stay green.
- `crates/famp/tests/fixtures/cross_machine/*` — the fixtures to regenerate (D-08).

### Spec / invariants
- `docs/GATEWAY-SETUP.md` — the file to correct (D-07).
- INV-10 / RFC-8785 JCS / `FAMP-sig-v1\0` domain prefix; spec authority v0.5.2 (`CLAUDE.md`).
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- The wire-proof injector (`crates/famp-gateway/tests/wire_proof_inject.rs`, throwaway) and the E2E's `send_bus_envelope` show the EXACT envelope shape the fixed `famp send` must produce — `from`/`to` domain-qualified, typed `RequestBody`, sign-then-strip. Reuse the shape; then DELETE the injector.
- `build_client_config` + the `probe_tls.rs` example were used to diagnose the TLS findings — pattern for the shipping-surface test's TLS assertions.

### Established Patterns
- Sign-then-strip / BUS-11: typed envelopes ride the local bus UNSIGNED; the gateway signs at egress. This is how D-03 avoids local crypto.
- `.planning/` is gitignored → run executors NON-isolated on main (sequential). Rust-touching executors run `just lint` (not plain clippy) + `just ci`.

### Integration Points
- `build_envelope_value` is the single chokepoint for all client send paths (CLI + MCP) — one change (D-01) covers all three surfaces.
- Ingress `verify_inbound_any` keys off the envelope `from` — the from-domain (D-02/D-05) must match the peer's pinned label.
</code_context>

<specifics>
## Specific Ideas

- The final gate is a LIVE two-machine re-run with the real `famp send` (no injector), mirroring the 2026-07-28 dogfood. Ben can spin up a fresh server-side agent (or reuse zed) when the fix is ready.
- Prefer the smallest change that satisfies the invariants; the reports converge on C5 for the `to` half — the `from` half (D-05) is the real design work of this phase.
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. (Relay / open-internet / cross-person trust / replay-cache / capability plane remain v1.1/v2.0 per REQUIREMENTS.md and are explicitly out of scope here.)
</deferred>

---

*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Context gathered: 2026-07-28*
