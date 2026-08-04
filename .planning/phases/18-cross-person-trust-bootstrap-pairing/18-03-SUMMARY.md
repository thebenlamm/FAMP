---
phase: 18-cross-person-trust-bootstrap-pairing
plan: 03
subsystem: security
tags: [pairing, consent, error-taxonomy, artifact, observe-before-pin, cli, falsification]

requires:
  - phase: 18-cross-person-trust-bootstrap-pairing
    provides: "Plan 01's InviteRecord/InviteState/InviteStore/StoreLock/pairing_ingress route, and Plan 02's InviteStore::decide state machine, MAX_ATTEMPTS attempt budget, and famp pair revoke"
provides:
  - "famp::pairing::consent::CONSENT_WARNING — the single authored QUAR-15 wording, rendered into docs/QUARANTINE.md, drift caught by consent_warning_matches_quarantine_doc"
  - "PairingError's seven final jargon-free, next-action-bearing messages plus StoreBusy, wired end-to-end through reject_reason_to_pairing_error"
  - "famp pair invite's single artifact: opening line, install step, consent warning, redeem step, prompt note, five-word code alone on the final line; --confirm-installed required (exit 2, zero bytes, no record without it)"
  - "famp pair status: structural observe-before-pin (REDEEMED BY: <principal> key_id=<key_id> written+flushed before any keyring mutation), a one-sentence done-signal, the gateway-restart notice, and a zero-Redeemed-records success path with a one-line nothing-to-do message"
  - "famp pair redeem: a one-sentence success done-signal; malformed code rejected client-side before any network call; HTTP reject reasons and transport failures mapped through PairingError with no invented remaining-tries number"
  - "docs/PAIRING.md — the mechanism reference (not the follower walkthrough)"
  - "crates/famp/tests/pair_cli.rs — 15 integration tests covering PAIR-04/05/07/08 at the CLI level"
affects: [19-auto-wake-gate, 20-human-acceptance-gate]

actuals:
  tokens: 17452
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "observe/pin split: pin_redeemed_record writes+flushes the identity line in one write_all call, then calls a separate pin() function that owns 100% of the module's filesystem mutation — making observe-before-pin structural, not incidental, and letting a test snapshot the keyring file between the two steps via a wrapping Write adapter"
    - "Self-built in-process mock endpoint for cross-crate-unreachable server logic: famp cannot depend on famp-gateway (the dependency runs the other way), so crates/famp/tests/pair_cli.rs stands up its own minimal axum responder for the two tests that need a real redemption round-trip, rather than reimplementing famp-gateway's ingest_redemption or adding a dependency-graph cycle"

key-files:
  created:
    - crates/famp/tests/pair_cli.rs
    - docs/PAIRING.md
  modified:
    - crates/famp/src/pairing/consent.rs
    - crates/famp/src/pairing/mod.rs
    - crates/famp/src/cli/pair/invite.rs
    - crates/famp/src/cli/pair/redeem.rs
    - crates/famp/src/cli/pair/status.rs
    - docs/QUARANTINE.md

key-decisions:
  - "status.rs's identity line format is the fixed, greppable `REDEEMED BY: <principal>  key_id=<key_id>` (two spaces before key_id) rather than the Plan-01-inherited free-text 'Redeemed by X (key_id Y)' — chosen so a test (and any future tooling) can match on a stable prefix."
  - "The restart notice's final clause was adapted to name pinning ('to pick up the newly pinned key') rather than byte-copied from rotate.rs's rotation-specific clause ('to pick up the rotated key') — matches the existing per-surface adaptation precedent already established by peer/revoke.rs, peer/retire.rs, and peer/import_revocation.rs, each of which independently reword only the final clause of the same NOTE."
  - "RedemptionReject carries no remaining-tries field on the wire today, so redeem.rs's wrong-code path always uses PairingError::WrongCode's message unchanged — the 'never invent a number' requirement is satisfied by there being no field to invent from, not by an interpolation branch that never fires. Adding that field would be a wire-schema change outside this task's file scope (crates/famp/src/pairing/mod.rs is not in Task 3's <files> list) and was not made."
  - "The two-home CLI-level mutual-pin test and the process-level done-signal test use a hand-rolled minimal axum responder (crates/famp/tests/pair_cli.rs::MockInviter) instead of reusing famp_gateway::pairing_ingress::ingest_redemption, because famp-gateway depends on famp (not the reverse) — the real ingest logic is unreachable from this crate's own test binary without an architecture change. The mock skips code/attempt validation (already exhaustively covered by pairing_ingress.rs and pairing_e2e.rs in the gateway crate) and exists only to prove this crate's own CLI wiring."

patterns-established:
  - "Falsification control recorded as a manual run (not automated CI), same convention Plan 02 established: temporarily invert the ordering under test, observe the RED/GREEN split, revert, re-observe both GREEN — full four-arm result below rather than merely asserted."

requirements-completed: [PAIR-04, PAIR-05, PAIR-07, PAIR-08]

coverage:
  - id: D1
    description: "Artifact encoding and ordering: consent warning before the code, code is the final line, no 16+ char base64url token (excluding https:// URLs), no invite id in the artifact"
    requirement: PAIR-04
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#artifact_code_offset_greater_than_consent_and_install_lines, #artifact_code_line_is_final_non_empty_line, #artifact_no_long_base64url_token_excluding_https, #artifact_does_not_contain_invite_id"
        status: pass
    human_judgment: false
  - id: D2
    description: "famp pair invite refuses to run without --confirm-installed: exits 2, writes zero bytes, creates no record"
    requirement: PAIR-08
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#process_level_invite_without_confirm_installed_exits_2"
        status: pass
    human_judgment: false
  - id: D3
    description: "Observe-before-pin is structural: the REDEEMED BY: identity line is written and flushed before any keyring byte changes, proven by a Write-wrapping snapshot taken at the moment the line is emitted, plus a recorded falsification control showing the test goes RED under a broken (pin-then-observe) ordering"
    requirement: PAIR-07
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#status_observe_before_pin_keyring_unchanged_at_write_time"
        status: pass
      - kind: manual_procedural
        ref: "Recorded falsification run, four outcomes below (this file)"
        status: pass
    human_judgment: true
    rationale: "The falsification run requires manually inverting the source ordering, observing, and reverting — it cannot be expressed as a single automated CI assertion without defeating its own purpose as an external check on the automated test."
  - id: D4
    description: "famp pair status with zero Redeemed records is a success (Ok) with a one-line nothing-to-do message, and leaves the keyring file untouched; a zero-record run is no longer indistinguishable from an empty writer"
    requirement: PAIR-07
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib pair::status#run_at_prints_nothing_redeemed_yet_when_no_redeemed_records_exist"
        status: pass
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#status_no_redeemed_records_is_ok_and_keyring_untouched"
        status: pass
    human_judgment: false
  - id: D5
    description: "Both redeem::run_at (success) and status::run_at (successful pin) print a one-sentence done-signal, not FSM JSON; the redeem success signal contains no '{' character and no second sentence boundary"
    requirement: PAIR-07
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#redeem_success_done_signal_is_single_sentence_no_brace, #two_home_mutual_pin_via_cli"
        status: pass
    human_judgment: false
  - id: D6
    description: "redeem-path failure mapping: a malformed code is rejected client-side (PairingError::CodeMalformed) before any network call; a transport failure maps to GatewayUnreachable with the --from URL interpolated; the seven PairingError messages are pairwise distinct, jargon-free, and each names an imperative next action"
    requirement: PAIR-05
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#redeem_malformed_code_rejected_before_network_call, #redeem_gateway_unreachable_message_interpolates_url"
        status: pass
      - kind: unit
        ref: "cargo test -p famp --lib pairing::tests#pair_errors_are_pairwise_distinct, #each_failure_message_names_an_imperative_next_action"
        status: pass
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#pair_errors_avoid_jargon"
        status: pass
    human_judgment: false
  - id: D7
    description: "The jargon-avoidance matcher was sanity-checked against a known-positive string (containing every jargon term) before trusting its zero-hit result on the seven real messages — a clean grep alone is not proof of absence"
    requirement: PAIR-05
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#pair_errors_avoid_jargon_sanity_check_catches_known_positive"
        status: pass
    human_judgment: false
  - id: D8
    description: "CONSENT_WARNING's exact bytes are present in docs/QUARANTINE.md — one authored source, drift caught by test, not by review"
    requirement: PAIR-04
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#consent_warning_matches_quarantine_doc"
        status: pass
    human_judgment: false
  - id: D9
    description: "PAIR-01's mutual-pin assertion restated at the CLI level: after invite -> redeem -> status runs across two separate tempdir homes, both peers.keyring files hold the other principal's key as Active"
    requirement: PAIR-07
    verification:
      - kind: integration
        ref: "cargo test -p famp --test pair_cli#two_home_mutual_pin_via_cli"
        status: pass
    human_judgment: false
  - id: D10
    description: "PAIR-05's comprehension half — whether a real non-expert can act on the seven messages — is NOT mechanically assertable and is explicitly NOT claimed closed here. It remains open for Phase 20's UAT-02."
    requirement: PAIR-05
    verification: []
    human_judgment: true
    rationale: "No test in this repo, or any repo, can measure human comprehension. This entry exists so the phase verifier does not mark PAIR-05 fully closed on the string-assertion tests alone."

duration: 70min
completed: 2026-08-04
status: complete
---

# Phase 18 Plan 03: Consent Warning, Error Taxonomy, One Artifact, Asymmetric Done-Signals Summary

**Pairing now speaks plain language end to end: one consent-gated artifact with the code at the bottom, seven jargon-free failure messages, and a structurally observe-before-pin `famp pair status` — closing PAIR-04, PAIR-07, and PAIR-08 mechanically and PAIR-05's mechanical half, with its comprehension half explicitly deferred to Phase 20's UAT-02.**

## Performance

- **Duration:** ~70 min for Task 3 (this execution). The full plan (Tasks 1-3, across three prior sessions) totaled 3 tasks, 3 commits.
- **Tasks:** 3 (Task 1 and 2 committed by prior sessions: `1feb06d`, `49ab2a5`; Task 3 committed this session: `8de93d6`)
- **Files created/modified:** 8 (`consent.rs`, `pairing/mod.rs`, `invite.rs`, `redeem.rs`, `status.rs`, `pair_cli.rs` (new), `docs/PAIRING.md` (new), `docs/QUARANTINE.md`)

## Accomplishments

- **`famp::pairing::consent::CONSENT_WARNING`** (Task 1): the single authored QUAR-15 sentence — `"Pairing with someone means their agent's messages will be read by your agent, which can run commands on your machine. Pair only with someone you would let type into your terminal."` — rendered verbatim into `docs/QUARANTINE.md` under a `## Consent warning (QUAR-15)` heading, drift caught by `consent_warning_matches_quarantine_doc`.
- **Seven jargon-free `PairingError` messages** (Task 1), each naming the failed step and an imperative next action: malformed code, wrong code, expired, already redeemed, attempts exhausted, gateway unreachable, same-machine refusal. `pair_errors_avoid_jargon` asserts none contains "public key", "fingerprint", "Ed25519", "keyring", or "base64"; `pair_errors_avoid_jargon_sanity_check_catches_known_positive` proves the matcher itself actually catches those terms on a fixture string, per this plan's falsification-discipline requirement (a clean grep alone is not proof of absence).
- **One invite artifact** (Task 2): `invite::build_artifact` returns a single string in PAIR-08 order (opening -> install -> `CONSENT_WARNING` -> redeem step -> prompt note -> code last), written in exactly one `write_all` call. `--confirm-installed` is required; without it `run_at` returns `CliError::Exit(2)`, writes zero bytes, and creates no invite record.
- **Structural observe-before-pin** (Task 3): `status.rs`'s `pin_redeemed_record` now writes and flushes the fixed `REDEEMED BY: <principal>  key_id=<key_id>` line in one `write_all` call, THEN calls a separate `pin()` function that is the ONLY place in the module touching the keyring file. This makes the ordering a structural fact of the call sequence, not an artifact of code layout — verified by a test that wraps the writer to snapshot the keyring file the instant the identity line's bytes land, and confirmed load-bearing by a recorded falsification control (see below).
- **Asymmetric done-signals** (Task 3): `redeem::run_at` prints `"Paired with <principal>; nothing else is needed on this side."` on success — one sentence, no FSM JSON, no `{`. `status::run_at` prints `"Paired with <principal>."` after a confirmed pin, followed by the gateway-restart notice. A zero-`Redeemed`-records `status` run is now `Ok(())` with a one-line `"Nothing redeemed yet..."` message instead of silent empty output.
- **Full redeem-path failure mapping** (Task 3): a malformed code is rejected via `PairingError::CodeMalformed` before any network call is attempted; the five HTTP reject reasons route through `reject_reason_to_pairing_error`; a transport-level failure (connection refused, DNS, timeout) maps to `PairingError::GatewayUnreachable { url }` with the `--from` URL interpolated. `RedemptionReject` carries no remaining-tries field on the wire today, so the wrong-code message is always used unchanged — no number is ever invented.
- **`crates/famp/tests/pair_cli.rs`** (new, 15 tests, all passing): did not exist before this session (Task 2's plan text asked for it but the prior agent placed Task 2's cases inline in `invite.rs`'s `mod tests` instead). This file now covers Task 2's PAIR-04/08 cases at the integration level for the first time, plus all of Task 3's observe-before-pin, jargon, doc-sync, and two-home mutual-pin cases.
- **`docs/PAIRING.md`** (Task 1): the mechanism reference — three-step flow, why the code is typed at a prompt (never `argv`/shell history), the 24-hour window and five-attempt budget (both server-enforced), `famp pair revoke`, crash-orphaned lock-file recovery, the `famp daemon restart` pin-not-live-until-restart caveat, and REACH-04's loopback-only status. Explicitly states it is NOT the follower-facing walkthrough (that is Phase 20's DOC-06/DOC-07).

## Task Commits

1. **Task 1: One authored consent warning, one plain-language error taxonomy** — `1feb06d` (feat) — prior session
2. **Task 2: The one artifact — install instructions, consent warning, code at the bottom** — `49ab2a5` (feat) — prior session
3. **Task 3: Asymmetric done-signals and observe-before-pin** — `8de93d6` (feat) — this session, 15 new tests in `crates/famp/tests/pair_cli.rs`

## Files Created/Modified

- `crates/famp/src/pairing/consent.rs` — `CONSENT_WARNING`, `CONSENT_WARNING_HEADING`
- `crates/famp/src/pairing/mod.rs` — `PairingError`'s seven final messages plus `StoreBusy`, `reject_reason_to_pairing_error`
- `crates/famp/src/cli/pair/invite.rs` — `--confirm-installed`, `build_artifact` (single-call, PAIR-08 order)
- `crates/famp/src/cli/pair/redeem.rs` — success done-signal, full failure mapping via `PairingError`/`reject_reason_to_pairing_error`
- `crates/famp/src/cli/pair/status.rs` — `observe_line`/`pin` split, `REDEEMED BY:` line, `NOTHING_REDEEMED_YET`, `RESTART_NOTICE`
- `crates/famp/tests/pair_cli.rs` (new) — 15 tests
- `docs/PAIRING.md` (new) — mechanism reference
- `docs/QUARANTINE.md` — `## Consent warning (QUAR-15)` section

## Decisions Made

See `key-decisions` in frontmatter above (identity-line format, restart-notice adaptation precedent, remaining-tries "no field to invent from" resolution, and the self-built mock-inviter rationale for the cross-crate-unreachable server logic).

## Falsification Control — Recorded Outcomes (PAIR-07, D3)

Per this plan's mandatory falsification-discipline requirement, `status.rs`'s `pin_redeemed_record` was manually mutated to run `pin()` BEFORE writing the `REDEEMED BY:` observe line (simulating the exact bug PAIR-07 exists to prevent — the inviter's trust store already mutated before the operator ever saw who it was mutating for), then the new ordering test and an unrelated pairing test were run, then the source was reverted and both re-run. All four outcomes below were observed directly, not inferred:

| State | `status_observe_before_pin_keyring_unchanged_at_write_time` | `pairing::tests::pair_errors_are_pairwise_distinct` (unrelated control) |
|---|---|---|
| **Broken ordering** (pin before observe) | **RED** — `assertion left == right failed`: the snapshot taken at write-time already contained the new pin (168 bytes) vs. the pre-run 69 bytes | **GREEN** — unaffected, as expected |
| **Correct ordering** (reverted, committed state) | **GREEN** | **GREEN** |

This confirms the test is a real, working falsification of the observe-before-pin ordering (it fails when the ordering breaks) and that the control test is not accidentally coupled to the same code path (it stays green regardless).

## Verification Performed

- `cargo test -p famp --lib pairing` — 38 passed, 0 failed (regression-clean; includes `pairing::consent`, `pairing::invite`, `pairing::wordlist`, `cli::pair::status`, `cli::pair::invite`, `cli::pair::revoke`)
- `cargo test -p famp --test pair_cli` — 15 passed, 0 failed
- `cargo test -p famp --test cli_help_invariant` — 1 passed
- `cargo test -p famp-gateway --test pairing_e2e --test pairing_ingress` — 5 + 8 = 13 passed, 0 failed (Plans 01/02 unregressed)
- `just lint` — exits 0, zero findings on the first full run (no fix-then-relint cycle needed)
- `cargo fmt --all -- --check` — silent after one `cargo fmt --all` pass (the initial diff was formatting-only, caught by the pre-commit hook and fixed before commit)
- `git status --porcelain | grep -E "famp-(envelope|canonical|crypto|core|fsm)/"` — empty; Layer 0 unmodified

**Total: 38 + 15 + 1 + 13 = 67 tests passing, 0 failures.**

## Issues Encountered

- **Full-workspace `timeout 600 cargo test --workspace --no-fail-fast` did not finish within 10 minutes.** The log (`/tmp/famp-w3.log`, 847 lines before the harness killed the command) shows every test that ran passed with zero failures, through `install_uninstall_roundtrip.rs`, which is exactly the pre-existing flaky group this repo's own memory already documents ("5 codex install/uninstall tests flake under `cargo test --workspace`" — they probe `target/debug/famp` while cargo relinks it, unrelated to this plan's files). Plan 02's own SUMMARY hit the identical situation (a different slow/unrelated test, same underlying cause: this workspace's full-suite run is documented as unusable locally — `cargo nextest hangs`, `just ci`/`just test` are UNUSABLE). Every command actually named in this plan's per-task `<verify>` blocks was run individually and passed (see Verification Performed above); the full-workspace run is flagged here as unverified rather than silently claimed green.
- A pre-commit `rustfmt` hook caught a formatting diff in the initial `pair_cli.rs` draft (line-wrap differences in a few multi-line `assert!`/import statements). Fixed with one `cargo fmt --all` pass before committing; re-verified `cargo fmt --all -- --check` silent and all 15 tests still passing after the reformat.

## Known Carry-Forward Gaps (stated explicitly, not silently dropped)

- **PAIR-05's comprehension half does NOT close in this plan.** Every mechanical assertion this plan can make about the seven failure messages (pairwise distinctness, imperative next-action presence, jargon absence) is made and passing. Whether a real non-expert can actually ACT on the wording is not something a unit test can measure. This closes only at Phase 20's UAT-02 — do not record PAIR-05 as fully satisfied on this plan's tests alone.
- **BIP-39 transcription-robustness is NOT asserted anywhere.** Per `18-RESEARCH.md`'s unverified assumption A1, no user-facing text in this plan (or `docs/PAIRING.md`) claims the wordlist is transcription-robust or minimum-edit-distance designed. Only the unique-4-char-prefix property (verified separately in Plan 01) is ever asserted.
- **A pin is durable immediately but not active until `famp daemon restart`** — same limitation as `famp peer rotate`/`famp peer revoke`. `status::run_at`'s `RESTART_NOTICE` and `docs/PAIRING.md` both state this; no surface claims live pickup.
- **The e2e fixtures (`pairing_e2e.rs`) use literal `2030-08-03` timestamps** — a deliberate placeholder Plan 02 introduced after the original `2026-08-03` literal's 24-hour window elapsed against the real wall clock. This WILL need bumping again eventually; the real fix (a clock-injectable fixture) is deferred, not implemented in this plan (out of Task 3's file scope: the fixture lives in `famp-gateway/tests/pairing_e2e.rs`, not touched by this plan).
- **REACH-04's genuinely-different-networks leg remains open** — `docs/PAIRING.md` states the inviter's gateway must be reachable from the redeemer's network, and that as of Phase 17 this is proven on loopback only; a NATed inviter is explicitly not a supported configuration today.

## Next Phase Readiness

- All four of this plan's requirements (PAIR-04, PAIR-05 mechanical half, PAIR-07, PAIR-08) are closed at the mechanism level; PAIR-05's comprehension half is explicitly carried to Phase 20.
- `famp::pairing::consent::CONSENT_WARNING` is ready for Phase 19's auto-wake gate to consume without re-typing the sentence.
- `docs/PAIRING.md` and `docs/QUARANTINE.md`'s consent section are both ready references for Phase 20's DOC-06/DOC-07 follower-facing walkthrough to link into, not duplicate.
- Layer 0 crates (`famp-envelope`, `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`) verified untouched.

---
*Phase: 18-cross-person-trust-bootstrap-pairing*
*Completed: 2026-08-04*
