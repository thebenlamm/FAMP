# Phase 20: Human Acceptance Gate - Context

**Gathered:** 2026-08-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver the follower-facing setup guide, mechanically validate it on a genuinely fresh machine with no FAMP state or Rust toolchain, then run one real acceptance event in which a second person follows the guide without coaching, pairs across a different network without a shared VPN or hand-copied keys, and exchanges signed task traffic bidirectionally with Ben. The phase closes only when each receiver's own `famp inspect tasks` output shows the received task reached a terminal state. It does not redesign distribution, reachability, pairing, quarantine, or auto-wake, and it does not depend on Phase 21 push notifications.

</domain>

<decisions>
## Implementation Decisions

### Follower guide and rehearsal
- **D-01:** Write one linear follower-facing guide that starts from an unprepared supported machine, leads with the prebuilt-binary installer, and uses `famp pair` rather than the legacy `famp peer export/import` flow. Background and operator caveats may be linked, but the happy path must not require the follower to synthesize steps across multiple documents.
- **D-02:** The guide must keep roles explicit throughout (Ben/inviter and follower/redeemer), use copy-pasteable commands, explain each observable done-signal, place the consent warning at the actual pairing decision, and route failures through the shipped actionable messages rather than teaching cryptographic concepts.
- **D-03:** Before involving the second person, execute the exact follower path on a disposable clean environment that has neither prior FAMP state nor a Rust toolchain. The rehearsal must exercise downloaded release binaries, pairing, gateway readiness, bidirectional signed delivery, and receiver-side terminal task inspection. A preflight assertion must fail if FAMP state or Rust tooling is present.
- **D-04:** Semantic guide gates must assert role direction, pairing flow, install source, readiness signals, sender-exit non-proof, and receiver-owned terminal-state proof. Flag/string presence alone is insufficient because it would repeat the v1.0 inverted-wiring failure.

### Human event protocol
- **D-05:** Ben prepares only his own machine and sends the single pairing artifact plus the follower guide. The follower operates their own machine. Ben may observe and collect explicitly shared evidence but may not type commands, screen-control, edit the follower's state, hand-copy keys, or provide step-by-step recovery that is absent from the guide.
- **D-06:** Clarifying what an ordinary word means is allowed only if it does not reveal the next action. Any question about what command to run, what value to enter, or how to recover is recorded as a guide/comprehension failure; update the guide, reset the affected state, and rerun the acceptance event rather than coaching through it.
- **D-07:** The network topology is a hard precondition: two independently administered machines on different networks, no shared VPN/overlay, and no direct key-file or public-key-line exchange. The inviter URL must be publicly reachable by the follower before the invite's 24-hour clock begins.
- **D-08:** Use two distinct tasks, one initiated from each machine. For each direction, the receiving person captures their own `famp inspect tasks --id <task_id> --json` output showing a terminal state (`COMPLETED`, `FAILED`, or `CANCELLED`). Sender exit status, gateway logs, mailbox arrival alone, or a report relayed by Ben cannot substitute.

### Evidence and failure handling
- **D-09:** Preserve a redacted acceptance record containing machine/OS and binary versions, proof of no Rust/prior FAMP state for the rehearsal, network-independence attestation, timestamps, pairing done-signals, both task IDs/directions, and both receivers' terminal-state outputs. Never capture short codes, private keys, auth tokens, or unredacted home paths.
- **D-10:** Separate three outcomes: pass, product/guide failure, and invalid run. Product/guide failures include confusing instructions or actionable-message comprehension failure; invalid runs include coaching, stale FAMP state, Rust/source-build fallback, same-network/VPN use, copied keys, or missing receiver-owned evidence. Only a fully clean rerun may close an invalid run.
- **D-11:** The seven Phase 18 pairing failure messages receive comprehension evidence during the clean rehearsal and/or real event by naturally encountered failures plus a scripted, non-mutating review with the second person. The person must state the next action in their own words without explanation; this closes PAIR-05's human half without deliberately damaging the live pairing attempt.

### the agent's Discretion
- Exact guide filename and section layout, clean-environment technology, redaction format, evidence template layout, and test implementation are left to planning, provided they satisfy the locked evidence and no-coaching rules above.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Scope and acceptance contract
- `.planning/ROADMAP.md` — Phase 20 goal, dependency chain, four success criteria, and explicit exclusion of Phase 21.
- `.planning/REQUIREMENTS.md` — DOC-06, DOC-07, UAT-02, and PAIR-05's deferred human-comprehension half.
- `.planning/phases/18-cross-person-trust-bootstrap-pairing/18-VERIFICATION.md` — exact open PAIR-05 human item and mechanically verified pairing boundaries.

### Distribution and clean-machine installation
- `.planning/phases/16-distribution/16-CONTEXT.md` — locked distribution decisions, supported binary targets, installer integrity boundary, and curl-first documentation policy.
- `.planning/phases/16-distribution/16-VERIFICATION.md` — verified release/install capabilities and any limits the clean-box rehearsal must respect.
- `README.md` — current public installation entry points and release claims.

### Pairing, quarantine, and gateway operation
- `docs/PAIRING.md` — shipping `famp pair` mechanism, role asymmetry, invite limits, restart requirement, and explicit statement that it is not the follower walkthrough.
- `docs/QUARANTINE.md` — consent and remote-content handling boundary the follower must understand.
- `docs/GATEWAY-SETUP.md` — existing operator/mechanism guide, semantic wiring lessons, TLS/firewall constraints, and receiver-side task proof; it is reference material, not the new follower flow.
- `.planning/phases/19-auto-wake-gate/19-VERIFICATION.md` — verified Local-only auto-wake boundary and explicit remote Inbox availability.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/famp/tests/gateway_setup_doc_accuracy.rs`: existing semantic documentation gate with concrete direction, ordering, and send-confirmation assertions; extend its approach rather than relying on flag greps.
- `crates/famp/tests/pair_cli.rs`: process-level pairing artifact, done-signal, jargon, consent-order, and mutual-pin coverage that can anchor guide assertions.
- `crates/famp/tests/installer_checksum_gate.rs` and release installer fixtures: existing prebuilt-installer integrity harness for the clean-box rehearsal.
- `crates/famp-gateway/tests/e2e_relay_bidirectional.rs`: established bidirectional signed-relay test shape, useful for designing evidence without substituting automation for UAT.

### Established Patterns
- Receiver-owned proof: a sender-side zero exit means only local broker acceptance; terminal `famp inspect tasks` output on the receiving machine is the end-to-end criterion.
- Semantic documentation tests: assert concrete role direction and operation order, because presence-only tests previously passed an inverted guide.
- Pairing is asymmetric: redemption pins one side, `famp pair status` identifies the redeemer before pinning the inviter side, and the gateway must restart to activate the new pin.
- Remote traffic remains durable but cannot automatically wake an agent; the acceptance procedure must include explicit Inbox/task processing where required and must not depend on Phase 21.

### Integration Points
- New follower guide should become the DOC-06 public entry point and link to `docs/PAIRING.md`/`docs/GATEWAY-SETUP.md` only for deeper troubleshooting.
- Automated semantic gates belong with the existing `crates/famp/tests/*doc_accuracy.rs` and installer tests.
- The real-person event needs a checked-in evidence/procedure artifact and an explicit human checkpoint; it cannot be marked passed by a unit or loopback test.

</code_context>

<specifics>
## Specific Ideas

- Treat the human attempt as a scarce acceptance event: rehearse the exact path first, then freeze the guide used for the event.
- Use a compact evidence checklist with commands and redaction instructions so the follower can supply receiver-owned proof without exposing secrets.
- Make “unassisted” auditable by logging questions and classifying them before any recovery action.

</specifics>

<deferred>
## Deferred Ideas

- Push-based agent notification remains Phase 21; Phase 20 must succeed through the current explicit inspection/processing path.
- A signed peer directory and automated NAT traversal remain outside v1.1 scope.

</deferred>

---

*Phase: 20-human-acceptance-gate*
*Context gathered: 2026-08-05*
