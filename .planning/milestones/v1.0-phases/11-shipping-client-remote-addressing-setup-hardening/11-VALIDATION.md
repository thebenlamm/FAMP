---
phase: 11
slug: shipping-client-remote-addressing-setup-hardening
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-27
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo-nextest (Rust) |
| **Config file** | Cargo.toml workspace + justfile |
| **Quick run command** | `cargo test -p famp --lib` |
| **Full suite command** | `just ci` |
| **Estimated runtime** | ~{N} seconds |

---

## Sampling Rate

- **After every task commit:** Run the quick run command for the touched crate
- **After every plan wave:** Run `just ci`
- **Before `/gsd-verify-work`:** `just ci` must be green (Rust-touching plans also run `just lint`)
- **Max feedback latency:** {N} seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | OBS-01 | T-11-02 | reqwest-failure Display carries its `#[source]` | unit | `cargo test -p famp-transport-http error` | ❌ W0 (created in task) | ⬜ pending |
| 11-01-02 | 01 | 1 | OBS-01 | T-11-02 | egress relay log captures the full source chain | integration | `just ci` | ✅ | ⬜ pending |
| 11-02-01 | 02 | 1 | ADDR-03 | T-11-03/04 | single-source own-domain, validated, one env-read site | unit | `cargo test -p famp own_domain` | ❌ W0 (created in task) | ⬜ pending |
| 11-02-02 | 02 | 1 | ADDR-03 | T-11-03 | peer-export label authority == own-domain (from==pinned) | unit | `cargo test -p famp peer::export` | ✅ extend | ⬜ pending |
| 11-03-01 | 03 | 2 | ADDR-01 | T-11-06/09 | domain-qualified from/to + leaf bus Target (split-addr) | unit | `cargo test -p famp send` | ✅ extend (L512+) | ⬜ pending |
| 11-03-02 | 03 | 2 | ADDR-02 | T-11-07/08 | typed unsigned `request` via sign-then-strip; local unchanged | unit | `cargo test -p famp send` | ✅ extend | ⬜ pending |
| 11-03-03 | 03 | 2 | ADDR-01/02 | T-11-07 | fixed binary deployed (MCP path) | integration | `just ci` | ✅ | ⬜ pending |
| 11-04-01 | 04 | 3 | TEST-03 | T-11-10 | fixtures CA:FALSE+serverAuth verify on macOS+Linux | integration | `cargo test -p famp-gateway --test e2e_cross_host_delivery` | ✅ regen | ⬜ pending |
| 11-04-02 | 04 | 3 | TEST-03 | T-11-11 | shipping `famp send` cross-host + negative local.bus→typed error | integration | `cargo test -p famp-gateway --test e2e_shipping_surface` | ❌ W0 (created in task) | ⬜ pending |
| 11-05-01 | 05 | 3 | DOC-05 | T-11-13/14/15 | guide corrected for all 8 findings + own-domain/remote-send | doc-accuracy | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✅ extend | ⬜ pending |
| 11-05-02 | 05 | 3 | DOC-05 | T-11-13 | semantic gate catches inversion/ordering/cert-policy | doc-accuracy | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✅ extend | ⬜ pending |
| 11-06-01 | 06 | 4 | UAT-01 | T-11-17 | fresh binaries deployed before dogfood | integration | `just ci` | ✅ | ⬜ pending |
| 11-06-02 | 06 | 4 | UAT-01 | T-11-16 | live two-machine terminal FSM, real client, no injector | manual | human-verify (see Manual-Only) | ✅ 11-HUMAN-UAT.md | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

New test surfaces are created within their owning plan as part of the work (TDD `tdd="true"` tasks write the failing test first), not in a separate Wave 0 plan:

- [ ] `crates/famp-transport-http` error-Display unit test (OBS-01) — created in 11-01 Task 1.
- [ ] `crates/famp/src/cli/own_domain.rs` `#[cfg(test)]` serial resolver test (ADDR-03) — created in 11-02 Task 1.
- [ ] `crates/famp/src/cli/send/mod.rs` `#[cfg(test)]` remote-path cases (ADDR-01/02) — extend existing module at L512+.
- [ ] `crates/famp-gateway/tests/e2e_shipping_surface.rs` happy + negative (TEST-03) — created in 11-04 Task 2 (reuses the `e2e_cross_host_delivery` harness).
- [ ] Regenerate `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` (TEST-03) — 11-04 Task 1.
- [ ] Extend `crates/famp/tests/gateway_setup_doc_accuracy.rs` with semantic assertions (DOC-05) — 11-05 Task 2.

No standalone Wave 0 plan required — each MISSING test is the deliverable of its own TDD task.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live two-machine dogfood re-run (no injector), FSM reaches terminal | UAT-01 | Requires two physical hosts + real remote agent | Re-run the Gate A dogfood: `famp send --to agent:<peer-domain>/<name>` from machine A → verify delivery + terminal task state on machine B |

*If none: "All phase behaviors have automated verification."*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < {N}s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
