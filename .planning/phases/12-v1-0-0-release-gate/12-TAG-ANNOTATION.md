---
phase: 12-v1-0-0-release-gate
plan: 05
requirement: REL-05
tag-candidate-sha: 5edff41835b9c8e6daa59a51efce549460d88e5b
---

# 12-TAG-ANNOTATION — Drafted `v1.0.0` Tag Body

This is the drafted annotation for the `v1.0.0` tag, reviewed here before Ben's
Task 2 checkpoint decision. Nothing in this file has been used to create a tag.
`git tag -l 'v1.0.0'` returns empty at the time this file is committed.

## Proposed Annotation

```
FAMP v1.0.0 — Federation Profile

Delivered: the v1.0 Federation Profile milestone (Phases 7-12, 6 phases, 29
plans, 25 requirements). A shipping FAMP client (`famp send` / `famp_send`)
addresses a remote principal across two machines Ben controls, drives a
signed, Ed25519-verified cross-host envelope through a gateway on each side,
and reaches a terminal task-FSM state on both hosts. This closes design
review C's central objection to the v1.0.0-rc.1 candidate: the release's
primary feature is now reachable through the shipping client, not just
provable via a hand-written injector.

Key accomplishments:
- Gateway skeleton resolving the same-host `kill(pid,0)` liveness fork for
  cross-host-proxied principals (Phase 7).
- Signed cross-host envelope wire format under `FAMP-sig-v1\0` (INV-10),
  two-machine TOFU trust bootstrap via hand-copied peer export/import
  (Phase 8).
- Full bidirectional request -> commit -> deliver cross-host delivery cycle,
  proven end-to-end (Phase 9).
- Test reactivation triage (27 parked tests: 0 salvageable, all superseded)
  and the two-machine setup guide (Phase 10).
- Shipping-client remote addressing: `famp send --to agent:<domain>/<name>`
  mode-branches into typed, FSM-driving Request/Commit/Deliver envelopes
  (not the always-audit_log bare-name path); trust-boundary hardening
  (broker binds `from` to authenticated identity, gateway rejects
  foreign-domain and misaddressed envelopes, federation-owned fields have
  exactly one writer, ambiguous route config fails closed); live two-machine
  dogfood UAT-01 PASS (Phase 11).
- This release gate: documented send-confirmation semantics (REL-01), a
  two-reviewer independent adversarial source review of the shipped trust
  boundary with two confirmed-and-fixed defects and eight documented-accept
  dispositions (REL-02), release-record hygiene (REL-04), and this tag,
  created only after CI green was re-verified at its exact target commit
  (REL-03/REL-05).

Known gaps / deferred (not fixed in this release, each with a named home):
- Own-domain fails open (not closed) when left unconfigured at gateway
  startup, and the documented setup order in GATEWAY-SETUP.md configures it
  AFTER first start — accepted for v1.0's locked own-two-machines,
  single-peer scope; recommend reordering the setup doc and consider a
  mandatory-own-domain-at-startup requirement alongside INGRESS-01/PEER-01
  in v1.1 (REL-02 review, finding F-2).
- One signing key per gateway process, but ingress trust is pinned per full
  Principal — backing 2+ agents under one gateway is not supported today
  and fails closed (typed `DuplicatePubkey` startup error), not silently;
  not reachable through the documented one-agent-per-gateway v1.0 flow
  (REL-02 review, finding F-3).
- Egress is a decoupled, non-durable background drain loop: a relay failure
  after local mailbox acceptance is logged, not retried or requeued, and
  the local sender has no visibility into it. Mitigated for v1.0 by the
  send-confirmation boundary documented below (REL-01), not by a durability
  fix (REL-02 review, finding F-4).
- Freshness-window and replay-cache enforcement at the ingress boundary is
  not built (v1.1's INGRESS-01) — `federation_format_ok` checks
  well-formedness only, not expiry-past or nonce-uniqueness.

Design review C section 16 "Exact release ruling" — the nine-item checklist
gating this tag, each item satisfied with its own evidence citation:

1. The shipping client accepts a complete remote principal — satisfied
   (Phase 11). Evidence: 11-VERIFICATION.md truth row #1 (`famp send --to
   agent:<domain>/<name>` splits `Target`/envelope `to`, delivers into the
   remote mailbox).
2. The signed envelope contains globally qualified `from` and `to` —
   satisfied (Phase 11). Evidence: 11-VERIFICATION.md truth row #1
   (`build_envelope_value`/`build_remote_envelope_value`) and truth row #2
   (typed, FSM-driving, sign-then-strip envelopes).
3. `from` is bound to broker-authenticated identity — satisfied (Phase 11).
   Evidence: 11-VERIFICATION.md SEC-01 row
   (`crates/famp-bus/src/broker/handle.rs::send`,
   `is_self_authored(envelope, Some(&effective_identity))` gate before
   mailbox write).
4. No `local.bus` authority crosses federation — satisfied (Phase 11).
   Evidence: 11-VERIFICATION.md truth row #1 (bus target stays bare leaf,
   only the signed envelope carries domain-qualified authority) and SEC-02
   row (ingress authoritative only for own domain + addressed mailbox).
5. The remote gateway verifies and delivers the same signed bytes —
   satisfied (Phase 11). Evidence: 11-VERIFICATION.md truth row #6
   (`e2e_shipping_surface.rs` drives the real `famp send`, cross-platform
   fixtures regenerated) and SEC-02 row (`federation_format_ok` wired into
   `inbox_handler`).
6. Existing canonicalization and E2E gates remain green — satisfied (this
   phase, REL-03). Evidence: 12-CI-ATTESTATION.md — commit 5edff41,
   `famp-canonical RFC 8785 conformance gate` and both `test (*)` jobs
   (which include `e2e_cross_host_delivery`/`e2e_shipping_surface`) all
   `success`, 11/11 check-runs green, re-queried immediately before this
   tag was created.
7. Ambiguous route configuration fails closed — satisfied (Phase 11,
   reconfirmed Phase 12). Evidence: 11-VERIFICATION.md SEC-04 row (`--backs`
   flag, duplicate/ambiguous config rejects at parse/startup); reconfirmed
   by 12-02-SUMMARY.md's route-config fail-closed fix
   (`crates/famp-gateway/tests/route_config_fail_closed.rs`).
8. The documentation states what `send` confirms — satisfied (this phase,
   REL-01). Evidence: 12-01-SUMMARY.md — docs/GATEWAY-SETUP.md §6, `famp
   send --help`, and README all state the fire-and-forget send-confirmation
   boundary, pinned by `gateway_setup_doc_accuracy.rs`.
9. Zed's independent source verdict is reconciled — satisfied (this phase,
   REL-02). Evidence: 12-02-SUMMARY.md / 12-REL-02-REVIEW.md — two-reviewer
   (self + codex) independent adversarial pass over the shipped trust
   boundary, 10 findings triaged, 2 real defects fixed with regression
   tests, 8 documented-accept with source-grounded rationale.

This discharges the closing line of the v1.0.0-rc.1 annotation: "v1.0.0
proper still requires design review C's section 16 nine-item checklist."
That checklist is closed as of this tag, at commit 5edff41835b9c8e6daa59a51efce549460d88e5b.

[LIMITATION STATEMENT — see "Limitation Statement Decision" below; option
(B), if selected, is inserted here as its own paragraph before the closing
line.]

Reachability scope, unchanged from rc.1 and locked at roadmap time:
own-two-machines only (direct network or a VPN Ben already runs). No public
relay, no cross-person trust, no signed peer directory, no
freshness/replay-cache enforcement, no capability/approval/tool-admission
plane, no conformance vector pack — all deferred to v1.1/v2.0 or to the
separate Gate B trigger.

Wire protocol unchanged: BUS_PROTO_VERSION remains 1.
```

## Limitation Statement Decision

**§16's proposed wording, verbatim (from `12-RESEARCH.md` § REL-05, quoting
`DESIGN-REVIEW-C-final.pdf` p.18-19):**

> "`famp send` demonstrates signed, fire-and-forget federated envelope
> delivery. It does not initiate or complete the task FSM; federated task
> initiation is not exposed through the v1.0 client interface."

**Why it is now false.** I re-verified this against the shipped code
directly (not just cited the prior finding). `crates/famp/src/cli/send/mod.rs`
`build_remote_envelope_value` mode-branches on the send flags: `--new-task`
emits a typed `RequestBody`, `--task` emits a typed `CommitBody`, `--task
--terminal` emits a typed `DeliverBody` with `terminal_status` set, and all
three are sign-then-strip (no bare `audit_log` fallback on the remote path).
This is exactly what `11-VERIFICATION.md` truth row #2 and Phase 11's ADDR-02
requirement describe, and it is not a theoretical capability: `11-HUMAN-UAT.md`
§4 records a real task (`019fab97-d3e0-7d63-92ba-39f1ce171b83`) driven
end-to-end by the real `famp send` CLI (no injector) reaching `COMPLETED` on
BOTH `bens-macbook-air` and `home-devbox` over Tailscale, `sig_verified: true`
on the federated path. §16's proposed sentence was written before Phase 11
shipped ADDR-02 and describes a gap that no longer exists. Shipping it
unedited would be a factually false claim in the release's own tag body —
exactly the failure mode `12-RESEARCH.md`'s "Pitfall 3" warns about.

**Two defensible options** (Task 2 is where the choice is made, not here):

- **(A) Ship NO limitation sentence.** The specific gap §16 described (no
  federated task-FSM initiation) is closed. Nothing on §16's own list
  remains to disclose. Cleanest possible release note — the nine-item
  checklist above already carries every disclosure §16 required.
- **(B) Ship a replacement built from REL-01's verified statement.** A
  successful `famp send` confirms only that the local broker accepted the
  envelope into the gateway-backed outbound mailbox on this host — not that
  the gateway has drained, signed, and relayed it, that the remote gateway
  verified it, that it reached the remote mailbox, or that the task FSM
  advanced on the far side. Egress is a decoupled background drain loop the
  CLI process never waits on. This is the exact wording already shipped in
  `docs/GATEWAY-SETUP.md` §6, `famp send --help`, and README (12-01,
  `gateway_setup_doc_accuracy.rs`-pinned), so the tag would restate an
  already-accurate, already-tested claim rather than invent new prose.

**Recommendation: (B),** with one caveat surfaced for Ben's judgment rather
than decided here — the fire-and-forget exit-code boundary is real and
operator-relevant even though the FSM gap §16 worried about is not; a reader
who only ever reads the tag (not the guide) benefits from the accurate
bounded limitation. The caveat: since 12-01 already shipped this exact
statement in three other user-facing surfaces (guide, `--help`, README), the
tag body arguably doesn't need to restate it at all — a reader following the
setup guide will already see it before they ever run `famp send`. Both (A)
and (B) are honest; the choice is about redundancy-for-emphasis versus
minimalism, not about correctness. Neither risks shipping stale text — that
risk only existed for the original, now-false §16 sentence, which is
excluded under every option.

## Scope Guard

Per `REQUIREMENTS.md` lines 93-123 (`## v2 Requirements (deferred)` and
`## Out of Scope`), the annotation must not be read as claiming any of:

1. **Public-internet relay / NAT traversal** (`RELAY-01`, v1.1) — the
   annotation's "Reachability scope" paragraph states own-two-machines only,
   direct network or a VPN Ben already runs. No claim of public-internet or
   relay reachability appears anywhere in the body.
2. **Cross-person trust bootstrap** (v1.1) — the annotation states trust is
   hand-copied keys between machines one person controls (unchanged from
   the rc.1 annotation's own framing); no claim of a cross-person trust flow.
3. **A signed peer directory** (`DIR-01`, v1.1) — not mentioned anywhere in
   the body; the "Reachability scope" paragraph explicitly lists "no signed
   peer directory" among the deferred items.
4. **Freshness / replay-cache enforcement** (`INGRESS-01`, v1.1) — the
   "Known gaps" section names this explicitly as unbuilt, and the
   "Reachability scope" paragraph repeats "no freshness/replay-cache
   enforcement" as an explicit deferral, not a shipped capability.
5. **The capability / approval / tool-admission plane** (`FSEC-01..N`,
   v2.0+, demand-gated) — not mentioned in the body except as an explicit
   deferral in the "Reachability scope" paragraph ("no
   capability/approval/tool-admission plane").
6. **The conformance vector pack** (Gate B, event-driven) — the annotation
   states explicitly that this ships on the separate Gate B trigger, not
   with this tag; no claim of vector-pack availability appears anywhere.

Confirmed by direct re-read of the drafted `## Proposed Annotation` body,
phrase by phrase, against this list: the only occurrences of "relay",
"replay", "directory", "capability", "vector pack", or "cross-person" in the
body are inside the explicit deferral/known-gaps sentences quoted above —
none appears as an affirmative claim of shipped capability.
</content>
