---
phase: 16-distribution
plan: 03
subsystem: testing
tags: [dist, cargo-dist, installer, checksum, sha256, falsification, shell]

requires:
  - phase: 16-distribution
    provides: "16-01 committed the three dist-generated shell installers under crates/famp/tests/fixtures/installers/ — famp-installer.sh is the subject under test here"
  - phase: 16-distribution
    provides: "16-02 added `just check-installer-drift`, which regenerates those fixtures and fails if they diverge from what dist 0.32 emits today — this is what keeps the tested artifact honest over time"
provides:
  - "crates/famp/tests/installer_checksum_gate.rs — the DIST-03 falsification pair"
  - "Mechanical proof that the shipped curl installer fails CLOSED on a checksum-corrupted artifact"
  - "A stripped-installer inversion proving the rejection is attributable to checksum verification specifically, not to some incidental failure mode"
affects: [16-04, 16-05, distribution, release-pipeline, security-claims]

actuals:
  tokens: 6825
  tasks: 2
  commits: 2

tech-stack:
  added: []
  patterns:
    - "Hand-rolled std::net::TcpListener fixture server (no new dev-dependency)"
    - "Falsification-with-control test pair, with a line-exact assertion that the control copy differs in exactly one line"

key-files:
  created:
    - crates/famp/tests/installer_checksum_gate.rs
  modified: []

key-decisions:
  - "The falsification is done by generating a stripped COPY of the committed installer at test time, never by editing the committed fixture — editing it would trip 16-02's check-installer-drift gate."
  - "assert_only_verify_call_changed() asserts the stripped copy differs from the working copy in EXACTLY one line, and that the changed line contains `verify_checksum`. This is the guard against the repo's recorded 'a mangled control fails everything and looks like real evidence' failure mode."
  - "The D-06 claim boundary is enforced in the test's own module doc: these tests prove byte-mismatch rejection, NOT that the release workflow was uncompromised. An attacker who can substitute the archive can substitute the checksum beside it. Signing is the named follow-up, not this gate."
  - ".config/nextest.toml was listed in the plan's files_modified but was NOT modified — the test completes in 0.24s and needs no slow-timeout entry. Recorded rather than manufactured."

patterns-established:
  - "Falsification pair naming: the CONTROL test states in its doc comment what MUST STILL PASS under the broken state; the FALSIFICATION test states what MUST FAIL. Both are named in the module header, not left implicit."
  - "Ephemeral-port hygiene: bind 127.0.0.1:0 and read local_addr(). A literal port in 32768-60999 is a known ubuntu-only CI flake in this repo."

requirements-completed: [DIST-03]

coverage:
  - id: D1
    description: "The shipped installer rejects an artifact whose bytes do not match its published checksum, exits non-zero, and leaves nothing installed (fail-CLOSED ordering — verify before write)"
    requirement: "DIST-03"
    verification:
      - kind: integration
        ref: "crates/famp/tests/installer_checksum_gate.rs#installer_rejects_a_corrupted_artifact_without_installing"
        status: pass
  - id: D2
    description: "The rejection is discriminating, not a gate that is red on everything — a matching artifact installs and exits 0"
    requirement: "DIST-03"
    verification:
      - kind: integration
        ref: "crates/famp/tests/installer_checksum_gate.rs#installer_accepts_a_matching_artifact"
        status: pass
  - id: D3
    description: "The rejection is provably caused by checksum verification specifically — the same corrupted artifact IS accepted by an installer copy with only the verify_checksum call removed"
    requirement: "DIST-03"
    verification:
      - kind: integration
        ref: "crates/famp/tests/installer_checksum_gate.rs — stripped-copy inversion inside installer_rejects_a_corrupted_artifact_without_installing, gated by assert_only_verify_call_changed()"
        status: pass
---

# 16-03: DIST-03 Checksum Falsification Pair

## What shipped

`crates/famp/tests/installer_checksum_gate.rs` (628 lines) — an integration test that
exercises the real, committed `famp-installer.sh` against fabricated artifacts served by a
hermetic local HTTP responder, proving DIST-03 mechanically rather than by assertion.

## The falsification pair, named explicitly

The repo's standing rule is that *a falsification run needs a control* — green under both the
working and the broken state carries zero information. Both halves are named in the test's own
module header:

- **MUST STILL PASS under the broken state** — `installer_accepts_a_matching_artifact`.
  This is the control. If checksum verification were removed from the installer entirely, this
  test would still pass. That is the point: it proves the harness is not vacuously green. Without
  it, an installer where *every* download 404'd would still look like a working checksum gate.

- **MUST FAIL under the broken state** — `installer_rejects_a_corrupted_artifact_without_installing`.
  This is the falsification. It asserts four things: a non-zero exit, an empty install root
  (fail-CLOSED ordering — the installer verifies *before* it writes, not after), a
  checksum-mismatch token in the installer's own stderr, and the inversion below.

## Why the rejection is provably load-bearing

A test that merely observes "corrupted artifact → non-zero exit" cannot distinguish checksum
verification from a 404, a tar failure, or a typo'd URL. So the test generates a **stripped copy**
of the installer at runtime via `strip_checksum_verification()`, which replaces exactly the
`verify_checksum` call with `true`, and feeds it the *same* corrupted artifact. That copy accepts
the artifact.

`assert_only_verify_call_changed()` then asserts the stripped copy has the same line count as the
working copy, differs at exactly one line index, and that the differing line contains
`verify_checksum`. This is the direct guard against this repo's recorded failure mode: a broken
revert-patch that fails every test looks identical to real evidence.

The copy is generated at test time. The committed fixture is never edited — editing it would trip
16-02's `just check-installer-drift`.

## Verification

Test run, locally, with both names appearing in output (`cargo test <filter>` exits 0 on zero
matches, so exit status alone is not evidence):

```
running 2 tests
test installer_rejects_a_corrupted_artifact_without_installing ... ok
test installer_accepts_a_matching_artifact ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

CI verified by SHA on `caebe4d` (not HEAD): workflow `ci` run 30785411852 — **success**;
workflow `smoke-test` run 30785411827 — **success**.

## Declared prohibitions — all hold

| Prohibition | Status | Evidence |
|---|---|---|
| No hardcoded port (ubuntu-only CI flake class) | enforced | binds `127.0.0.1:0`, reads `local_addr()`; grep for a literal 4–5 digit port returns nothing |
| No new dev-dependency for famp | enforced | `git diff efc92b2..HEAD -- crates/famp/Cargo.toml` is empty; fixture server is a hand-rolled `std::net::TcpListener` |
| Never writes to the real `~/.cargo/bin` | enforced | `CARGO_HOME` and `HOME` both redirected to per-test `TempDir`s |
| Control copy loses only the checksum path | enforced | `assert_only_verify_call_changed()` asserts exactly one differing line, containing `verify_checksum` |

## Deviations (Rule 3)

1. **Plan's artifact model was wrong about the mechanism.** `16-03-PLAN.md` describes a fabricated
   `.tar.xz` plus a separate `.sha256` as two downloaded artifacts. Reading `famp-installer.sh` in
   full shows that is not how dist 0.32's shell installer verifies. The test was written against the
   installer's actual verification path, and the divergence is recorded in the test's module doc.

2. **`.config/nextest.toml` listed in `files_modified` but not modified.** The test completes in
   0.24s and needs no slow-timeout entry; CI is green without one. Not manufacturing a config edit
   to satisfy the manifest.

## Claim boundary (D-06) — do not overstate

These tests prove the installer rejects an artifact whose **bytes do not match its published
checksum**. They do **not** prove the release workflow itself was uncompromised. An attacker who
can substitute the archive on the Release can substitute the checksum file beside it. Artifact
signing is the named follow-up, explicitly out of scope for this gate. No doc in this phase may
claim more than this — 16-04 enforces that.

## Self-Check: PASSED

- [x] Corrupted artifact → non-zero exit AND empty install root (fail-closed)
- [x] Matching artifact → installs, exits 0 (gate discriminates)
- [x] Stripped-copy inversion proves attribution to checksum verification
- [x] Control integrity asserted line-exactly, not by eyeball
- [x] No hardcoded port; no new dep; CARGO_HOME/HOME redirected
- [x] Test names confirmed present in real `cargo test` output
- [x] No tag created, no Release published
