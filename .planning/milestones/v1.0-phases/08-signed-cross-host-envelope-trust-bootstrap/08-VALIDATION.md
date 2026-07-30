---
phase: 8
slug: signed-cross-host-envelope-trust-bootstrap
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-23
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`); workspace uses `cargo-nextest` for CI but per-crate `cargo test --lib`/`--test` is the reliable path (nextest `--list` hangs — see project memory) |
| **Config file** | none — workspace `Cargo.toml` + per-crate `[lints]` mirror `[workspace.lints]` |
| **Quick run command** | `cargo test -p famp-envelope --lib && cargo test -p famp-gateway --lib` |
| **Full suite command** | `just ci` (fmt + `just lint` clippy -D + workspace tests) |
| **Estimated runtime** | ~30–90 seconds for touched crates; full `just ci` longer |

---

## Sampling Rate

- **After every task commit:** Run the touched crate's `cargo test -p <crate> --lib` (+ `--test <name>` if an integration test was added)
- **After every plan wave:** Run `just lint` (clippy pedantic/nursery gate) + `cargo test` across touched crates
- **Before `/gsd-verify-work`:** `just ci` must be green (fmt + lint + tests)
- **Max feedback latency:** ~90 seconds

---

## Per-Task Verification Map

> Populated by the planner/executor as tasks are defined. Every phase requirement (WIRE-01, WIRE-02, TRUST-01, TRUST-02) must map to at least one automated Rust test. Byte-exactness (WIRE-02) and rejection paths (WIRE-01/TRUST-02) are unit-testable in-process; TRUST-01 is a single-machine export→import round-trip test.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | — | — | WIRE-01 | — | unsigned / bad-sig envelope rejected before bus write | unit | `cargo test -p famp-gateway --lib verify_inbound` | ❌ W0 | ⬜ pending |
| TBD | — | — | WIRE-02 | — | federation-field envelope round-trips JCS byte-exact; local envelope byte-identical | unit | `cargo test -p famp-envelope --lib` | ❌ W0 | ⬜ pending |
| TBD | — | — | TRUST-01 | — | export blob → import → key pinned; matching-key envelope verifies | unit/integration | `cargo test -p famp --test peer_roundtrip` | ❌ W0 | ⬜ pending |
| TBD | — | — | TRUST-02 | — | unpinned-key envelope rejected, no state created | unit | `cargo test -p famp-gateway --lib unpinned` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test scaffolds for `verify_inbound` reject paths (`famp-gateway`) — `invalid_signature` + `unpinned_key`
- [ ] Byte-exact round-trip test scaffold for the extended envelope (`famp-envelope`) — including the local-envelope-unchanged assertion
- [ ] Single-machine `famp peer export`→`import` round-trip test scaffold (`famp` CLI crate); use `ChildGuard` if it spawns broker/register children

*Existing infrastructure (cargo test, `just lint`, KAT-tested `famp-crypto`) covers the framework layer — no framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real two-machine out-of-band key exchange (copy/paste over Signal) | TRUST-01 | Genuinely cross-host + human transport; the phase gate uses a single-machine round-trip proxy instead | Deferred to Phase 10 setup-guide walkthrough; Phase 8 proves the mechanism in-process |

*All Phase 8 gate behaviors have automated in-process verification; the live two-machine path is a Phase 9/10 concern.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
