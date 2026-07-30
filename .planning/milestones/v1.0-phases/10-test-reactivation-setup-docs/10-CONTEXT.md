# Phase 10: Test Reactivation + Setup Docs - Context

**Gathered:** 2026-07-27
**Status:** Ready for planning

> **[--auto]** Discussion ran in autonomous mode. Every gray area below was
> auto-resolved to the recommended option and logged inline. Ben should skim
> `<decisions>` before `/gsd-plan-phase 10` — any decision he wants changed is a
> one-line edit here, then re-plan.

<domain>
## Phase Boundary

Close the v1.0 Federation Profile — Gateway Core milestone by making the
cross-host machinery **durable and reproducible by someone other than the
author**. Three things become true:

1. The ~27 parked federation tests in `crates/famp/tests/_deferred_v1/` are
   **triaged** — each is either reactivated green in CI or removed with a
   documented rationale (TEST-01).
2. A **live two-process end-to-end test** exercising the full signed cross-host
   `request → commit → deliver → ack` cycle runs on **every `just ci`** — not
   behind a manual or `#[ignore]`'d path (TEST-02).
3. A **setup guide** lets a developer stand up the gateway between two machines
   unassisted — bind address, out-of-band key exchange, connect/verify (DOC-04).

**This is a milestone-closing, own-machines-first phase.** No new protocol
surface; it hardens + documents what Phases 7–9 built. On completion the roadmap
tags `v1.0.0` (the tag itself is a `/gsd-complete-milestone` action, not a task
in this phase).

**Explicitly out of scope** (v1.1 / v2.0+, per PROJECT.md / REQUIREMENTS.md v2):
public-internet relay (RELAY-01), signed peer directory (DIR-01), replay-cache
enforcement (INGRESS-01), no-implicit-peering (PEER-01), inbound-taint
(TAINT-01), the FAMP-Sec plane (SEC-01..N). Also NOT in scope: a hosted/CI
two-*physical*-machine runner — TEST-02's CI artifact is the two-*process*
loopback E2E; the true two-machine run is Ben's DOC-04 human walkthrough
(the milestone's Gate A dogfood), not an automated CI job.

</domain>

<decisions>
## Implementation Decisions

### TEST-01 — deferred-test triage (retirement-dominant, honest)
- **D-01 [auto → recommended]:** **Triage each of the 27 into one of three
  buckets, and expect RETIRE to dominate.** Scout shows **18 of 27** tests
  exercise the **permanently-deleted v0.8 federation CLI** (`famp init` /
  `setup` / `listen` / `peer`, `run_on_listener`, direct `HttpTransport`) — a
  surface hard-deleted in v0.9 Phase 4 and **not coming back** (v1.0 replaced it
  with `famp-gateway`, not `famp listen`). Buckets:
  1. **Retire-with-rationale** — tests bound to the dead CLI whose behavior is
     either gone or now covered by the gateway path. Delete the file; record a
     one-line rationale per file in a triage ledger (recommend
     `crates/famp/tests/_deferred_v1/TRIAGE.md` or an updated `README.md`) so
     the removal is documented, not silent (TEST-01 requires documented
     rationale for removals).
  2. **Already-covered** — intent now proven by Phase 9's
     `e2e_cross_host_delivery.rs` (the signed cross-host cycle, TOFU bootstrap,
     reject semantics). Retire and point the ledger at the covering test.
  3. **Reactivate** — only tests encoding **still-live, not-otherwise-covered
     intent** (adversarial conversation shapes, `send_more_coming_requires_new_task`
     task-scoping, `send_principal_fallback`, `conversation_restart_safety`)
     that can be re-expressed against the **current** `famp-bus` / `famp-gateway`
     API. Rewrite against the live API and move into the active
     `crates/famp/tests/` glob. Researcher classifies each file concretely;
     planner locks the per-file disposition.
- **D-02 [auto → recommended]:** **Do NOT resurrect the deleted CLI to satisfy a
  test.** If a test's only value was exercising `famp listen`/`init`/`setup`, it
  retires — reintroducing that surface contradicts the v0.9 Phase 4 deletion and
  the `v0.8.1-federation-preserved` escape-hatch design. The bar for "reactivate"
  is: the behavior still exists on a shipping surface today.

### TEST-02 — CI-gated live E2E (promote, don't rebuild)
- **D-03 [auto → recommended]:** **Promote Phase 9's
  `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` as the TEST-02
  artifact — do not author a second E2E.** It already stands up two brokers +
  two gateways over loopback HTTPS and drives the full signed
  `request→commit→deliver→ack` cycle with terminal-FSM assertion. TEST-02's real
  work is **guaranteeing it runs green under `just ci`**.
- **D-04 [auto → recommended / CRUX]:** **The load-bearing question is whether it
  runs under `cargo nextest run --workspace`** (which is exactly what `just ci` →
  `just test` invokes), because FAMP has a known **`cargo nextest -p famp` hang
  in the test-binary `--list` phase** (auto-memory `nextest_list_hang`). The
  Phase 9 E2E was only ever run via **plain `cargo test`**. Researcher MUST
  verify `cargo nextest run --workspace` (or `-p famp-gateway`) actually lists +
  executes + passes this subprocess-spawning test. If nextest hangs or skips it:
  add a **`nextest.toml` test-group** serializing the gateway E2E (mirror the
  existing `inspect-subprocess` serialization pattern), OR pin the subprocess
  test group like the inspect tests already do — whichever makes it run green in
  CI without reintroducing the `--list` hang. Do NOT "fix" it by `#[ignore]` or
  a manual recipe — that defeats TEST-02's "not gated behind manual" clause.
- **D-05 [auto → recommended]:** **Keep the E2E hermetic + `ChildGuard`-clean so
  it's CI-safe.** It must bind ephemeral ports / sockets, isolate `FAMP_HOME` per
  process, use fixture certs (already does), and RAII-reap every broker/gateway
  child (memory `test_child_guard_convention`) so a CI failure never leaks
  processes or races another test. Confirm no reliance on a developer's
  `~/.famp/` daemon.

### DOC-04 — two-machine setup guide
- **D-06 [auto → recommended]:** **New standalone guide at `docs/GATEWAY-SETUP.md`,
  linked from README's federation/quickstart section.** Own-machines-first
  framing (laptop ↔ dev server, direct or a VPN Ben already runs, **no public
  relay**). Structure it as a copy-pasteable runbook derived from the *actual*
  shipping CLI surface (Phase 8 `famp peer export/import`; Phase 9
  `famp-gateway --listen <addr> --tls-cert --tls-key --peer <domain>=<url>
  --trust-cert`) — planner reads `crates/famp-gateway/src/main.rs` for exact flag
  spellings so the doc can't drift from the binary. Sections: prerequisites +
  daemon on each host → gateway identity → **out-of-band key exchange** (`peer
  export` on A → Signal/clipboard → `peer import` on B, and reverse) with TOFU
  pin + fingerprint eyeball-check → start each gateway → **connect/verify**
  (address B's agent by principal, confirm delivery + terminal state via `famp
  inspect tasks`).
- **D-07 [auto → recommended]:** **Doc accuracy is gated against the binary, and
  the true two-physical-machine walkthrough is a HUMAN-UAT acceptance Ben
  performs** — it is literally the milestone's Gate A dogfood (laptop ↔ dev
  server). Automated verification asserts the doc's commands/flags match `--help`
  / `main.rs` (grep-gate, mirroring the v0.11 Phase 6 accuracy-against-binary
  gate); the "developer follows it unassisted and reaches a working cross-host
  connection" clause is verified by Ben's real run, captured in a
  `10-HUMAN-UAT.md`. Do not claim DOC-04 done on the grep-gate alone.

### Claude's Discretion
- Triage ledger location/format (`_deferred_v1/TRIAGE.md` vs README update) — D-01.
- Exact nextest serialization mechanism for the gateway E2E (test-group vs
  `[[profile...test-groups]]` pin) — D-04, planner picks against the existing
  `nextest.toml` + the `inspect-subprocess` precedent.
- Guide filename (`docs/GATEWAY-SETUP.md` vs a README `## Cross-host setup`
  section) — D-06, planner picks against the existing docs layout.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase intent & requirements
- `.planning/ROADMAP.md` §"Phase 10: Test Reactivation + Setup Docs" — goal + 3
  success criteria (the acceptance contract) + the `v1.0.0` tag-on-completion note.
- `.planning/REQUIREMENTS.md` — TEST-01, TEST-02, DOC-04 exact text; the v2
  deferral list bounding what NOT to document/test.
- `.planning/PROJECT.md` §"Current Milestone" — own-machines-first, "no public
  relay," explicitly-NOT-v1.0 list.

### TEST-01 — the deferred corpus
- `crates/famp/tests/_deferred_v1/README.md` — the original freeze rationale +
  the (now-updatable) "reactivation criteria/path" note; scout found the
  reactivation trigger already fired (Phase 9 shipped the cross-host cycle).
- `crates/famp/tests/_deferred_v1/*.rs` — the 27 parked tests. 18 reference the
  deleted v0.8 CLI (`init/setup/listen/peer`, `run_on_listener`, `HttpTransport`);
  candidate-salvage set (still-live intent): `send_more_coming_requires_new_task.rs`,
  `send_principal_fallback.rs`, `conversation_restart_safety.rs` (researcher
  re-classifies each against the current API).
- v0.9 Phase 4 deletion commit `feat!(04): remove federation CLI surface …` +
  escape-hatch tag `v0.8.1-federation-preserved` — why the CLI is gone for good.

### TEST-02 — the E2E + CI
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` — the Phase 9 two-process
  loopback E2E to promote (single test `gw01_gw02_gw03_two_process_cross_host_delivery`).
- `justfile` — `test:` = `cargo nextest run --workspace`; `ci:` chains
  `fmt-check lint build … test …`. The E2E must run green *inside this chain*.
- `.config/nextest.toml` (or wherever the `inspect-subprocess` serialization
  lives) — the precedent for serializing subprocess-spawning tests; auto-memory
  `nextest_list_hang` (`cargo nextest -p famp` stalls in `--list`).
- `.planning/phases/09-…/09-05-SUMMARY.md` + `09-VERIFICATION.md` — what the E2E
  asserts and the GW-03 fix (`785b8c2`) that made terminal state observable.

### DOC-04 — the setup guide
- `crates/famp-gateway/src/main.rs` — exact cross-host CLI flags
  (`--listen`/`--tls-cert`/`--tls-key`/`--peer`/`--trust-cert`) the guide documents.
- `crates/famp/src/cli/peer/` — `famp peer export`/`import` surface (Phase 8, D-05).
- `README.md` §quickstart / `## Platform support` — where the guide links in;
  the v0.11 Phase 6 daemon-first onboarding + the accuracy-against-binary gate
  pattern (`docs/…` verified live) to mirror for DOC accuracy.
- `.planning/phases/08-…/08-CONTEXT.md` (TRUST-01 export/import) and
  `.planning/phases/09-…/09-CONTEXT.md` (D-02 peer-endpoint map, D-08 TLS) — the
  trust + reachability model the guide narrates.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 9's `e2e_cross_host_delivery.rs` is the TEST-02 artifact — promote, don't
  rebuild (D-03). It already uses fixture certs + `ChildGuard` + isolated homes.
- The `inspect-subprocess` nextest serialization is the ready-made pattern for
  making the gateway E2E CI-safe under nextest (D-04).
- Phase 8/9 CLI surface (`famp peer export/import`, `famp-gateway --listen …`)
  is the literal content of the DOC-04 runbook (D-06).
- v0.11 Phase 6's "accuracy-against-binary" doc gate is the model for keeping
  DOC-04 from drifting from `--help` (D-07).

### Established Patterns
- **Deleted surface stays deleted** — don't resurrect `famp listen`/`init` to
  green a test; the v0.9 Phase 4 deletion + preserved tag are deliberate (D-02).
- **Subprocess tests need `ChildGuard` + serialization** (memories
  `test_child_guard_convention`, `nextest_list_hang`) — the CI-gating work for
  TEST-02 lives here (D-04/D-05).
- **`.planning/` gitignored → executors run NON-isolated on main** (memory) —
  same as Phases 7–9; test/doc files under `crates/`/`docs/` commit normally.
- **`just lint` (not plain clippy) for any Rust touch; `just ci` is the CI-parity
  gate** — TEST-02 succeeds only when the E2E is green *inside* `just ci`.

### Integration Points
- Triage ledger (TEST-01) → documents every retire/reactivate decision.
- Reactivated tests → move from `_deferred_v1/` into the active
  `crates/famp/tests/` glob so nextest picks them up.
- Gateway E2E → runs inside `just ci`'s `cargo nextest run --workspace` (TEST-02).
- `docs/GATEWAY-SETUP.md` → linked from README; flags grep-gated against
  `main.rs` (DOC-04); Ben's two-machine run recorded in `10-HUMAN-UAT.md`.

</code_context>

<specifics>
## Specific Ideas

- Triage is **retirement-dominant and that's correct** — most parked tests
  encode a CLI FAMP deliberately deleted; the honest close is deleting them with
  a one-line rationale each, not contorting them onto the gateway. Salvage only
  genuine, still-uncovered intent.
- The single most important TEST-02 fact to nail down first: **does the Phase 9
  E2E execute under `cargo nextest run --workspace`?** Everything else in TEST-02
  is downstream of that one check. Prove it before planning the CI wiring.
- Write DOC-04 from the binary's real `--help`, not from memory — mirror the
  v0.11 accuracy-gate so the guide can't drift from the shipped flags.

</specifics>

<deferred>
## Deferred Ideas

- **Automated two-*physical*-machine CI runner** — out of scope; TEST-02's CI
  artifact is the two-process loopback E2E; the real two-machine run is Ben's
  DOC-04 human UAT (Gate A dogfood).
- **Public-internet relay / directory / cross-person trust / inbound-taint** —
  v1.1 (RELAY-01/DIR-01/PEER-01/TAINT-01); the setup guide stays own-machines-first.
- **FAMP-Sec plane** — v2.0+ (SEC-01..N).
- **`v1.0.0` tag + milestone archival** — the roadmap tags on completion, but the
  tag/`CHANGELOG`/archive is a `/gsd-complete-milestone` action, not a Phase 10
  task.
- **Conformance vector pack (Gate B)** — event-driven, unblocks only when a 2nd
  implementer commits to interop; not this phase.

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 10-Test Reactivation + Setup Docs*
*Context gathered: 2026-07-27*
