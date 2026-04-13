---
phase: 1
slug: minimal-signed-envelope
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-13
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for the `famp-envelope` crate. Derived from `01-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (primary) + `cargo test` (for `compile_fail` doctests) |
| **Config file** | `crates/famp-envelope/Cargo.toml` + workspace `.config/nextest.toml` |
| **Quick run command** | `cargo nextest run -p famp-envelope` |
| **Full suite command** | `cargo nextest run -p famp-envelope && cargo test -p famp-envelope --doc` |
| **Estimated runtime** | ~15 seconds (unit + proptest default budget) |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp-envelope`
- **After every plan wave:** Run `cargo nextest run -p famp-envelope && cargo test -p famp-envelope --doc && cargo clippy -p famp-envelope -- -D warnings`
- **Before `/gsd:verify-work`:** Full suite + RFC 8785 vector 0 fixture match green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

*Populated by gsd-planner once tasks are numbered. Seed rows from research (requirement → test type):*

| Requirement | Test Type | Automated Command | Notes |
|-------------|-----------|-------------------|-------|
| ENV-01 (envelope shape) | fixture + proptest | `cargo nextest run -p famp-envelope envelope::shape` | Round-trip every body variant |
| ENV-02 (deny_unknown_fields everywhere) | fixture | `cargo nextest run -p famp-envelope envelope::unknown_field` | Reject at every depth |
| ENV-03 (canonical round-trip) | proptest + insta | `cargo nextest run -p famp-envelope canonical::roundtrip` | Byte-exact |
| ENV-06 (request body) | fixture | `cargo nextest run -p famp-envelope body::request` | |
| ENV-07 (deliver body) | fixture | `cargo nextest run -p famp-envelope body::deliver` | |
| ENV-09 narrowed (commit w/o cap-snapshot) | fixture (negative) | `cargo nextest run -p famp-envelope body::commit_no_cap` | Unknown field `capability_snapshot` rejected |
| ENV-10 (INV-10, signed-only) | type + fixture | `cargo test -p famp-envelope --doc unsigned_unreachable` | `compile_fail` doctest + decode-time rejection |
| ENV-12 cancel-only (no supersede/close) | fixture (negative) | `cargo nextest run -p famp-envelope control::cancel_only` | Unknown variant `supersede` rejected |
| ENV-14 (ack body) | fixture + §7.1c vector 0 | `cargo nextest run -p famp-envelope vector_zero` | Byte-exact against committed hex |
| ENV-15 (error taxonomy) | fixture | `cargo nextest run -p famp-envelope errors::` | 11 `EnvelopeDecodeError` variants mapped to `ProtocolErrorKind` |

*Task IDs filled by planner in the final per-task table — every task row must list `read_first`, `acceptance_criteria`, and one of the commands above.*

---

## Wave 0 Requirements

- [ ] `crates/famp-envelope/Cargo.toml` created, workspace member registered
- [ ] `crates/famp-envelope/src/lib.rs` stub with `EnvelopeDecodeError` enum skeleton
- [ ] `crates/famp-envelope/tests/vectors/` directory with §7.1c worked-example fixture committed verbatim (324-byte canonical, 336-byte signing input, 64-byte signature — copied from spec v0.5.1 fork)
- [ ] `crates/famp-envelope/tests/fixtures/` directory with one JSON per body variant (request, commit, deliver, ack, control-cancel) + negative fixtures (missing-sig, unknown-field, supersede-attempt)
- [ ] `insta` snapshot directory seeded with placeholder `.snap` files
- [ ] `proptest` added as dev-dep, default config (256 cases) in `proptest-regressions/` checked in

*Framework already installed via workspace (nextest, insta, proptest from v0.6). No new global installs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Domain-separation prefix correctness vs v0.5.1 spec fork | ENV-10 / INV-10 | Prefix lives in `famp-crypto` (v0.6); envelope code never touches it. One-time human read of shipped code + spec §7.1a on wave completion. | `rg DOMAIN_PREFIX crates/famp-crypto` and diff against spec section; document hash in VERIFICATION.md |

*Everything else has automated verification. No `#[ignore]` tests.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify command or a Wave 0 dependency entry
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers §7.1c vector 0 fixture and `compile_fail` doctest skeleton
- [ ] No watch-mode flags; all runs are one-shot
- [ ] Feedback latency < 20s on M-class hardware
- [ ] `nyquist_compliant: true` set in frontmatter before `/gsd:execute-phase`

**Approval:** pending
