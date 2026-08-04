---
phase: 18-cross-person-trust-bootstrap-pairing
verified: 2026-08-03T23:10:00Z
status: passed
score: 5/5 mechanically-closeable truths verified (PAIR-01,02,03,04,06,07,08 mechanically closed; PAIR-05 deliberately partial per plan design)
behavior_unverified: 0
overrides_applied: 1
overrides:
  - field: status
    from: human_needed
    to: passed
    authorized_by: Ben
    authorized_on: 2026-08-03
    scope: "Phase 18 only — does NOT close PAIR-05's comprehension half."
    rationale: >-
      The sole human_verification item below is not a Phase 18 test. It requires a
      genuine non-technical second person, which is the definition of Phase 20's
      Human Acceptance Gate. ROADMAP.md:258 pre-declared this before execution began:
      "Open on completion: PAIR-05's comprehension half ... closes only at Phase 20's
      UAT-02." Leaving Phase 18 blocked on it would park a mechanically-complete phase
      behind a UAT that cannot run until Phase 20, while Phase 19 proceeded against a
      phase showing incomplete.
      The item is NOT waived — it is retained verbatim below, carried in
      REQUIREMENTS.md as PAIR-05 [~] Partial, and closes at Phase 20 UAT-02.
      This override changes WHERE the item is tracked, not WHETHER it must pass.
# Retained verbatim under the override above. Deferred, NOT waived.
human_verification:
  - test: "PAIR-05 comprehension half — give the seven pairing failure messages to a genuine non-technical person and observe whether they can act on each without further explanation."
    expected: "A non-expert reads a failure message and knows what to do next without asking a follow-up question."
    why_human: "No automated test can measure human comprehension. This is explicitly and correctly deferred to Phase 20's UAT-02 by the plan itself (18-03-PLAN.md, 18-03-SUMMARY.md D10, REQUIREMENTS.md line 72). Recorded here only so the phase does not silently read as fully closed."
    deferred_to: "Phase 20 UAT-02"
    status: open
---

# Phase 18: Cross-Person Trust Bootstrap (Pairing) Verification Report

**Phase Goal:** Two people with no prior shared secret and no assumed cryptography background
complete mutual key pinning by exchanging a short code over any human channel (Signal, voice,
text), with a wrong or expired code hard-aborting rather than silently degrading — replacing
v1.0's paste-a-blob TOFU pattern.
**Verified:** 2026-08-03 (session date reported as 2026-08-03/2026-08-04 in phase artifacts)
**Status:** human_needed (mechanically complete; one requirement, PAIR-05, is deliberately and
correctly recorded as partial pending Phase 20's human UAT — this is a known, planned deferral,
not a gap this phase introduced)
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Mutual key pinning: both keyrings hold the other's key Active after one code exchange (PAIR-01) | VERIFIED | `cargo test -p famp-gateway --test pairing_e2e#happy_path_pins_both_sides_mutually` (5 passed) and `cargo test -p famp --test pair_cli#two_home_mutual_pin_via_cli` (15 passed) both assert via `Keyring::load_from_file`, not string-matching. Independently re-run, both green. |
| 2 | Wrong/expired code hard-aborts, bounded attempts, correct code after exhaustion still refused (PAIR-02) | VERIFIED | `pairing_ingress.rs::correct_code_after_attempts_exhausted_is_still_refused` — a CORRECT code presented with `attempts==5` is rejected `attempts_exhausted`, record stays `Pending` (read directly, code inspected). 5th wrong guess → `attempts==5`; 6th → `attempts_exhausted`, no live comparison. Independently re-run: 8/8 passed. |
| 3 | Single-use + 24h window, survives restart in both directions, `revoke` kill switch (PAIR-03) | VERIFIED | `replay_of_consumed_code_after_reload_is_rejected` drops all handles and reloads `InviteStore` fresh from disk before replaying — genuine persistence proof, not in-memory state (code inspected: `PairingIngressState` holds only a path, loads fresh every call). `famp pair revoke --id`/`--all-pending` exists, tested 7 ways, durable via the same persisted record. |
| 4 | No raw key blob pasted, no fingerprint compared (PAIR-04) | VERIFIED | `artifact_no_long_base64url_token_excluding_https` performs a real `split_whitespace` scan over every token in the artifact (not a spot-check), rejecting any 16+-char base64url token once `https://` URLs are excluded. `artifact_does_not_contain_invite_id` confirmed separately. |
| 5 | Failure messages name the failed step + next action, non-expert language — MECHANICAL HALF (PAIR-05) | PARTIAL (as designed) | Seven pairwise-distinct, jargon-free messages verified (`pair_errors_avoid_jargon`, sanity-checked against a known-positive fixture per this repo's own "clean grep is not proof of absence" lesson). Comprehension half explicitly and correctly left open for Phase 20 UAT-02 — confirmed in REQUIREMENTS.md line 72/216, ROADMAP.md line 258, both SUMMARYs. Not a gap; a documented, deliberate scope boundary. |
| 6 | Code enters only via stdin, never argv (PAIR-06) | VERIFIED | `PairRedeemArgs` (crates/famp/src/cli/pair/redeem.rs:34-43) has no positional field and no code-bearing flag — structural, confirmed by reading the struct. `redeem_argv_refuses_positional_code` confirms clap rejects `famp pair redeem hunter two three four five` with "unexpected argument" before any code is read. |
| 7 | Inviter sees WHO redeemed before the pin becomes durable; asymmetric done-signals (PAIR-07) | VERIFIED | `status.rs::pin_redeemed_record` calls `out.write_all(observe_line(...))` + `out.flush()` strictly before calling `pin()` (the ONLY function in the module that touches the keyring) — confirmed by reading the source. `status_observe_before_pin_keyring_unchanged_at_write_time` uses a `SnapshotOnFirstWrite` wrapper that snapshots the keyring file bytes on the FIRST `write()` call (i.e., at the moment the identity line lands), not at end-of-run — confirmed this is a real emission-time snapshot, not an end-of-run read. A recorded falsification control (18-03-SUMMARY.md) shows the test goes RED under a manually-inverted (pin-before-observe) ordering and GREEN under the correct ordering. |
| 8 | One artifact, consent before code, code on the final line, clock doesn't start before install confirmed (PAIR-08) | VERIFIED | `artifact_code_offset_greater_than_consent_and_install_lines` uses real `str::find`/`rfind` byte offsets (not heuristics) to prove the code line's offset exceeds both the consent-warning offset and the last install-line offset. `--confirm-installed` is a required bool flag; without it `run_at` returns `CliError::Exit(2)`, writes zero bytes, creates no record — confirmed by reading `invite.rs:90-101`. |

**Score:** 7/8 requirement-level truths fully mechanically VERIFIED; 1/8 (PAIR-05) correctly and
deliberately PARTIAL per the plan's own design, with its comprehension half routed to Phase 20.
0 behavior-unverified items beyond the one documented human-verification item below.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp/src/pairing/bip39-english.txt` | 2048-word vendored list | VERIFIED | 2048 lines; measured SHA-256 (`shasum -a 256`, independently re-computed) = `2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda`, matches the pinned `WORDLIST_SHA256` constant exactly. |
| `crates/famp/src/pairing/wordlist.rs` | draw/parse/digest, uniform draw | VERIFIED | `gen_range(0..WORDLIST_LEN)` (rejection-sampling, no modulo bias) confirmed present; `digests_equal` is a genuine fixed-length XOR-accumulate over all 32 bytes, no early return, confirmed by reading the loop. |
| `crates/famp/src/pairing/invite.rs` | InviteStore/StoreLock/save_atomic/decide | VERIFIED | 762 lines, `decide`/`burn_attempt`/`consume`/`revoke` all present and match plan's exact branch-order spec. |
| `crates/famp/src/pairing/consent.rs` | CONSENT_WARNING single-authored | VERIFIED | Only defined once in `consent.rs`; referenced (not re-typed) in `invite.rs`; `docs/QUARANTINE.md` carries the literal bytes; drift caught by `consent_warning_matches_quarantine_doc` (`include_str!` + `.contains()`). |
| `crates/famp/src/cli/pair/{mod,invite,redeem,status,revoke}.rs` | full CLI surface | VERIFIED | All 5 files present, substantial (100-378 lines each), all wired into `PairSubcommand` and the top-level `Commands::Pair` dispatcher. |
| `crates/famp-gateway/src/pairing_ingress.rs` | dedicated unauthenticated route | VERIFIED | 327 lines; own `Router`, own `PairingIngressState`, never touches `GatewayIngressState`/`ingress_guard`/`inbox_handler`/`ingest_inbound` (confirmed by reading imports and state type). |
| `crates/famp/tests/pair_cli.rs` | 15 integration tests | VERIFIED | 655 lines, 15/15 passing on independent re-run. |
| `crates/famp-gateway/tests/{pairing_e2e,pairing_ingress}.rs` | 5 + 8 tests | VERIFIED | Both independently re-run: 5/5 and 8/8 passing. |
| `docs/PAIRING.md`, `docs/QUARANTINE.md` (consent section) | mechanism reference + consent doc | VERIFIED | Both exist; `docs/PAIRING.md` opens with the DOC-06/REACH-04 scope disclaimer verbatim; contains `famp daemon restart`. `docs/QUARANTINE.md` has `## Consent warning (QUAR-15)` heading and the exact CONSENT_WARNING bytes. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `famp pair status` | `Keyring::rotate_to` → `save_to_file` | single trust-write site | VERIFIED | `pin()` in `status.rs` is the only function in the module touching the keyring; called strictly after the observe line is written+flushed. |
| `build_gateway_router(..., pairing_router)` | `Router::merge` before `RequestBodyLimitLayer` | ordering | VERIFIED | `ingress.rs:155` merges `pairing_router` into the base router; the `.layer(RequestBodyLimitLayer)` call is at `ingress.rs:162`, strictly after. |
| `ingest_redemption` | `StoreLock` → `InviteStore::load` (re-read under lock) → mutate → `save_atomic` → THEN response | persist-before-reply | VERIFIED | Code inspected directly; matches doc comment precisely. Falsification control recorded in 18-02-SUMMARY.md (RED under reverted ordering, GREEN under correct ordering, both re-confirmed reverted-back GREEN). |
| `CONSENT_WARNING` | `invite::run_at`'s artifact AND `docs/QUARANTINE.md` | one authored source, drift test | VERIFIED | Confirmed single definition site; drift test passing on independent re-run. |

### Phase-17 Non-Regression (explicitly scrutinized per plan's own acceptance criteria)

| Check | Command | Result |
|-------|---------|--------|
| Full `famp-gateway` crate suite | `cargo test -p famp-gateway` (independently re-run in this verification session) | **exit 0**, all tests passed including `e2e_cross_host_delivery`, `e2e_relay_bidirectional`, `e2e_shipping_surface`, `gateway_usage_doc_accuracy`, `inbound_destination_validation` (6/6), `liveness`, `no_cross_talk`, and (separately re-confirmed) `route_config_fail_closed` (6/6), `own_domain_startup_fatal` (1/1), `revocation` (11/11). Zero failures observed. |
| `ingress_guard.rs` whole-file pin | `shasum -a 256` | Still matches the Phase-17 pinned digest `8df625f...` — file genuinely untouched. |
| Layer 0 freeze | `git log --since=2026-08-03 -- crates/famp-{envelope,canonical,crypto,core,fsm}` | Empty — no commits since phase start touched any Layer 0 crate. |

### Behavioral Spot-Checks / Adversarial Scrutiny Items

| Scrutiny point | Finding |
|---|---|
| Observe-before-pin snapshot timing (PAIR-07) | Genuinely emission-time, not end-of-run. `SnapshotOnFirstWrite::write()` snapshots on the FIRST call, and `write_all(observe_line)` is the module's first write, called before `pin()`. Seeded with a non-empty pre-run keyring specifically to avoid a trivial-pass-on-empty-file false positive (code comment confirms this deliberate design). |
| Artifact byte-offset ordering (PAIR-08) | Real `str::find`/`rfind` offset comparisons, not string presence checks. |
| No-blob scan (PAIR-04) | Real `split_whitespace()` loop over every token, not a spot check. |
| Attempt-budget boundary (PAIR-02) | Correct-code-after-exhaustion is explicitly tested (`correct_code_after_attempts_exhausted_is_still_refused`), record verified still `Pending`. |
| Single-use across restart (PAIR-03) | Verified via a genuine disk-based reload — `PairingIngressState` holds only a path and `InviteStore::load` re-reads on every call; the "drop and reload" pattern is real, not simulated in memory. |
| Consent-before-code drift test | `consent_warning_matches_quarantine_doc` uses `include_str!` at compile time against the actual committed doc file — genuinely fails on drift, confirmed by reading the assertion. |
| Jargon-avoidance test false-negative risk | This phase explicitly applied the repo's own "clean grep is not proof of absence" lesson: `pair_errors_avoid_jargon_sanity_check_catches_known_positive` proves the matcher itself catches a known-positive fixture before trusting the zero-hit result on real messages. |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| PAIR-01 | 18-01 | Mutual key pinning via short code | SATISFIED | e2e + CLI-level mutual-pin tests, re-run green |
| PAIR-02 | 18-02 | Wrong code hard-aborts, bounded attempts | SATISFIED | boundary tests re-run green, code inspected |
| PAIR-03 | 18-02 | Single-use, 24h window, restart-durable, revoke | SATISFIED | restart/concurrency tests re-run green, `revoke` present and tested |
| PAIR-04 | 18-03 | No raw blob, no fingerprint comparison | SATISFIED | real token-scan test re-run green |
| PAIR-05 | 18-03 | Plain-language, actionable failure messages | PARTIAL (by design) | mechanical half satisfied and tested; comprehension half correctly deferred to Phase 20 UAT-02, consistent everywhere (REQUIREMENTS.md, ROADMAP.md, both SUMMARYs) |
| PAIR-06 | 18-01 | Stdin-only code entry | SATISFIED | structural (no field exists) + clap-rejection test re-run green |
| PAIR-07 | 18-03 | Observe-before-pin, asymmetric done-signals | SATISFIED | emission-time snapshot test + falsification control, re-run green |
| PAIR-08 | 18-03 | One artifact, consent before code, install-confirmed gate | SATISFIED | byte-offset tests + exit-2 structural gate, re-run green |

No orphaned requirements found — REQUIREMENTS.md's traceability table (lines 212-219) maps all
eight PAIR-* IDs to Phase 18 and each plan's frontmatter `requirements:` field covers exactly
this set with no gaps.

### Anti-Patterns Found

None. Debt-marker scan (`TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER`) across all phase-18-owned
files returned exactly one hit — `URL_PLACEHOLDER` in `invite.rs`, a legitimate named constant for
a user-fill-in value in the artifact (explicitly specified by the plan's action text: "a
clearly-marked placeholder plus a stderr note"), not a debt marker. No empty implementations, no
hardcoded-empty stub patterns, no "not yet implemented" strings found in any phase-18 source or
doc file.

`#[allow(...)]` audit across all phase-18 files: all instances are either `unwrap_used`/
`expect_used` inside `#[cfg(test)]` test modules (standard repo convention) or the one diagnosed
(not blanket) `future_not_send` allow on `StdinLock`, documented and justified in 18-01-SUMMARY.md
with four cited repo precedents. No safety-relevant lint was weakened.

### Human Verification Required

#### 1. PAIR-05 comprehension half

**Test:** Give the seven pairing failure messages (malformed code, wrong code, expired, already
redeemed, attempts exhausted, gateway unreachable, same-machine refusal) to a genuine
non-technical person and observe whether they know what to do next without further explanation.
**Expected:** A non-expert can act on each message unaided.
**Why human:** No automated test can measure human comprehension. This is not a gap in this
phase's execution — it is explicitly, correctly, and consistently deferred to Phase 20's UAT-02
across REQUIREMENTS.md, ROADMAP.md, and both plan SUMMARYs. It is surfaced here only so the phase
does not read as unconditionally "passed" while one requirement remains intentionally open.

### Gaps Summary

No gaps found. Every mechanically-closeable truth in this phase's must-haves, across all three
plans, was independently re-verified against the actual codebase (not SUMMARY claims): all named
test files were re-run fresh in this verification session and matched the counts the orchestrator
reported (38 lib + 15 pair_cli + 5 pairing_e2e + 8 pairing_ingress + 1 cli_help_invariant = 67
tests, all passing); `just lint` and `cargo fmt --all --check` both re-run clean; the full
`famp-gateway` crate suite was independently re-run to completion with zero failures, closing out
the one item this verification could not take on faith from the SUMMARY (a full-crate regression
run had been reported as "unverified" due to a 10-minute local timeout in both prior plan
SUMMARYs — this verification session ran it to completion and it passed). The one open item
(PAIR-05's comprehension half) is a deliberate, correctly-recorded scope boundary belonging to
Phase 20, not a defect of this phase's execution.

---

*Verified: 2026-08-03*
*Verifier: Claude (gsd-verifier)*
