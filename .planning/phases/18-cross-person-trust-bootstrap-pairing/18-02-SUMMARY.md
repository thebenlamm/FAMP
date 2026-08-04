---
phase: 18-cross-person-trust-bootstrap-pairing
plan: 02
subsystem: security
tags: [pairing, attempt-limit, ttl, single-use, revoke, axum, falsification]

requires:
  - phase: 18-cross-person-trust-bootstrap-pairing
    provides: "Plan 01's InviteRecord/InviteState/InviteStore/StoreLock/save_atomic, the pairing_ingress route and PairingIngressState, and the famp pair CLI surface"
provides:
  - "InviteStore::decide — a pure, read-only six-outcome classifier (Accept/WrongCode/Expired/AlreadyRedeemed/AttemptsExhausted/NoPendingInvite) that is the single authoritative state machine for attempt limits, TTL, and single-use"
  - "MAX_ATTEMPTS=5 server-side attempt budget enforced in InviteStore::decide against a persisted per-record counter"
  - "ingest_redemption endpoint enforcement: persist-before-reply ordering, lock-and-recheck concurrency control, zero-mutation rejections except the wrong-code attempt burn"
  - "famp pair revoke --id <id> | --all-pending — the operator kill switch, durable across a gateway restart"
affects: [18-03, 19-auto-wake-gate, 20-human-acceptance-gate]

actuals:
  tokens: 16512
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Checked-before-mutate via a pure &self classifier (decide) plus separate &mut self mutators (burn_attempt/consume/revoke), mirroring Keyring::rotate_to's discipline"
    - "Lock-and-recheck: an unlocked cheap read decides the branch, then the mutating branch RE-ACQUIRES the lock and RE-DECIDES under it before mutating, so a concurrent winner is never double-honored"
    - "Persist-before-reply: save_atomic under StoreLock runs strictly before the signed HTTP response is constructed, proven by a recorded falsification control rather than asserted"

key-files:
  created:
    - crates/famp-gateway/tests/pairing_ingress.rs
    - crates/famp/src/cli/pair/revoke.rs
  modified:
    - crates/famp/src/pairing/invite.rs
    - crates/famp/src/pairing/mod.rs
    - crates/famp-gateway/src/pairing_ingress.rs
    - crates/famp-gateway/tests/pairing_e2e.rs
    - crates/famp/src/cli/pair/mod.rs

key-decisions:
  - "decide's branch order puts AttemptsExhausted ahead of Accept for a matching record (T-18-12) — the correct code presented after the budget is spent is refused, never pinned, exactly as PAIR-02 requires."
  - "burn_attempt increments every live (non-expired) Pending record, not just 'the' record — a wrong code has no matched record to attribute the attempt to, so the whole live Pending set absorbs it. In practice there is usually exactly one Pending invite outstanding."
  - "HTTP status codes were extended beyond Plan 01's set: already_redeemed -> 409 Conflict, attempts_exhausted -> 429 Too Many Requests, expired -> 404 (grouped with no_pending_invite/code_mismatch per T-18-09's oracle-avoidance intent). Not specified in the plan text; chosen for operator legibility since the reason string in the JSON body already differentiates these cases regardless of status code."
  - "Deviation: pairing_e2e.rs's literal invite-creation timestamps bumped from 2026-08-03 to 2030-08-03, and wrong_code_leaves_store_byte_identical_and_pending renamed/rewritten as wrong_code_burns_one_attempt_and_stays_pending — see Deviations below."

patterns-established:
  - "Falsification control as a recorded manual run, not an automated CI step: revert the ordering under test, observe the expected RED/GREEN split, revert back, observe both GREEN — all four outcomes written into the SUMMARY verbatim rather than asserted."

requirements-completed: [PAIR-02, PAIR-03]

coverage:
  - id: D1
    description: "Attempt-limit three-way boundary: 5th wrong guess rejected leaving attempts=5, 6th wrong guess rejected as attempts_exhausted without a live comparison, correct code after exhaustion also refused and never pinned"
    requirement: PAIR-02
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib pairing::invite#decide_attempts_exhausted_at_max_accept_one_below"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test pairing_ingress#wrong_code_burns_one_attempt_each_up_to_max_then_exhausted, #correct_code_after_attempts_exhausted_is_still_refused"
        status: pass
    human_judgment: false
  - id: D2
    description: "Concurrency: two ingest_redemption futures racing the same valid code yield exactly one Accept and one already_redeemed, via re-decide under StoreLock"
    requirement: PAIR-03
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test pairing_ingress#concurrent_redemptions_of_same_code_yield_exactly_one_success"
        status: pass
    human_judgment: false
  - id: D3
    description: "Restart durability: a consumed invite replayed after a fresh InviteStore::load from disk is rejected already_redeemed"
    requirement: PAIR-03
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test pairing_ingress#replay_of_consumed_code_after_reload_is_rejected"
        status: pass
    human_judgment: false
  - id: D4
    description: "Persist-before-reply ordering is load-bearing, proven by a falsification control (not merely asserted): reverting the ordering flips replay_of_consumed_code_after_reload_is_rejected to RED while correct_code_first_use_succeeds stays GREEN; reverting back restores both to GREEN"
    requirement: PAIR-03
    verification:
      - kind: manual_procedural
        ref: "Recorded falsification run, four outcomes below (this file)"
        status: pass
    human_judgment: true
    rationale: "The falsification run itself requires a human/agent to manually mutate source, observe, and revert — it cannot be expressed as a single automated CI assertion without defeating its own purpose as an external check on the automated tests."
  - id: D5
    description: "Every rejection path except wrong-code leaves pairing.json byte-identical, proven by before/after byte reads"
    requirement: PAIR-02
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test pairing_ingress#non_wrong_code_rejections_leave_store_byte_identical"
        status: pass
    human_judgment: false
  - id: D6
    description: "famp pair revoke --id / --all-pending exists, is durable, and a revoked invite is never redeemable regardless of digest equality"
    requirement: PAIR-03
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib pair::revoke (7 tests, including cli_rejects_id_and_all_pending_together and revoked_invite_is_rejected_as_no_pending_invite_by_decide)"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-08-04
status: complete
---

# Phase 18 Plan 02: Attempt Budget, TTL, Single-Use, and Revoke Summary

**A pure `InviteStore::decide` state machine backs a persist-before-reply pairing endpoint that survives concurrency and restarts, plus `famp pair revoke` as the durable kill switch — closing PAIR-02 and PAIR-03.**

## Performance

- **Duration:** 55 min
- **Tasks:** 3
- **Files modified/created:** 7

## Accomplishments

- **`InviteStore::decide`** (`crates/famp/src/pairing/invite.rs`): a pure, read-only classifier over six outcomes (`Accept`, `WrongCode`, `Expired`, `AlreadyRedeemed`, `AttemptsExhausted`, `NoPendingInvite`). `AttemptsExhausted` is checked ahead of `Accept` (T-18-12) so a correct code presented after the budget is spent is never honored. A `Revoked` record is filtered out before any digest comparison runs, so it never matches regardless of digest equality. Three separate `&mut self` mutators (`burn_attempt`, `consume`, `revoke`/`revoke_all_pending`) perform no validation of their own — `decide` already ran.
- **Endpoint enforcement** (`crates/famp-gateway/src/pairing_ingress.rs`): `ingest_redemption_at` delegates its decision half to `decide`. The wrong-code path is the ONLY rejection that mutates (burns the attempt budget under `StoreLock`, re-loading first). The accept path verifies the signature (expensive) before acquiring the lock, then re-decides a SECOND time under the lock — turning two concurrent valid redemptions into exactly one success — and persists strictly before the signed response is built.
- **`famp pair revoke`** (`crates/famp/src/cli/pair/revoke.rs`): `--id <id>` or `--all-pending`, clap-enforced mutual exclusion with exactly one required. Durable — the revocation lives in the same persisted `pairing.json` record every other write goes through.
- **Falsification control run and recorded** (see below): the persist-before-reply ordering is proven load-bearing, not merely asserted.

## Task Commits

1. **Task 1: `InviteStore::decide` state machine** — `20e393f` (feat) — 17 unit tests in `crates/famp/src/pairing/invite.rs`
2. **Task 2: endpoint enforcement + falsification proof** — `32c91fb` (feat) — 8 tests in new `crates/famp-gateway/tests/pairing_ingress.rs`, `pairing_e2e.rs` updated
3. **Task 3: `famp pair revoke`** — `170662a` (feat) — 7 tests in `crates/famp/src/cli/pair/revoke.rs`

## Files Created/Modified

- `crates/famp/src/pairing/invite.rs` — `MAX_ATTEMPTS`, `InviteRecord::is_expired`, `RedemptionDecision`, `InviteStore::decide`/`burn_attempt`/`consume`/`revoke`/`revoke_all_pending`, 17 unit tests
- `crates/famp/src/pairing/mod.rs` — `PairingError` extended with `Expired`/`AlreadyRedeemed`/`AttemptsExhausted`/`WrongCode`/`UnknownInvite` (placeholder wording, `TODO(18-03)` markers)
- `crates/famp-gateway/src/pairing_ingress.rs` — `ingest_redemption_at` rewritten to delegate to `decide`; `burn_and_reject_wrong_code`/`accept_and_consume` helpers extracted to stay under the function-length lint; `reject_status` extended with `expired`/`already_redeemed`/`attempts_exhausted`
- `crates/famp-gateway/tests/pairing_ingress.rs` (new) — 8 tests
- `crates/famp-gateway/tests/pairing_e2e.rs` — timestamp bump + one test renamed/rewritten (see Deviations)
- `crates/famp/src/cli/pair/revoke.rs` (new) — `PairRevokeArgs`, `run`/`run_at`, 7 tests
- `crates/famp/src/cli/pair/mod.rs` — `Revoke` variant + dispatch arm

## Decisions Made

- `decide`'s branch order (Revoked filtered first, then digest match, then state/expiry/attempts in that order) is the single authoritative source of truth both the endpoint and the CLI's `decide`-based revoke-effectiveness test rely on.
- HTTP status codes for the new rejection reasons (`already_redeemed`→409, `attempts_exhausted`→429, `expired`→404) were chosen for operator legibility; the plan did not specify them and T-18-09's oracle-avoidance concern is already satisfied by `no_pending_invite`/`code_mismatch`/`expired` sharing 404.
- `burn_attempt` burns every live `Pending` record's counter on a wrong-code guess (not a single "closest match") — matches Task 1's literal `<behavior>` bullet; the only Pending invite in normal single-invite-outstanding operation absorbs it identically to a per-record design.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `pairing_e2e.rs`'s literal invite-creation timestamps had already elapsed against the real wall clock**
- **Found during:** Task 2, first full run of `cargo test -p famp-gateway --test pairing_e2e`
- **Issue:** Plan 01's fixtures used a literal `now` of `2026-08-03T00:00:00Z` for invite creation (expiring `2026-08-04T00:00:00Z`). `ingest_redemption`'s real internal clock read is `pub(crate)`-scoped and not test-injectable from an external test file (documented in `pairing_e2e.rs`'s own module doc). Since Task 2 wired real `is_expired` enforcement into the endpoint for the first time, and the actual real wall clock at execution time was `2026-08-04T00:56Z` — already past the fixture's 24-hour window — every `pairing_e2e.rs` test started failing `expired`.
- **Fix:** Bumped the five literal `2026-08-03` occurrences to `2030-08-03` (same relative offsets: created 00:00:00, redeem 00:05:00, status 00:10:00). Documented in the module doc comment as a known-recurring maintenance need, not a permanent fix.
- **Files modified:** `crates/famp-gateway/tests/pairing_e2e.rs`
- **Verification:** `cargo test -p famp-gateway --test pairing_e2e` — 5 passed, 0 failed
- **Committed in:** `32c91fb`

**2. [Rule 1 - Bug] `wrong_code_leaves_store_byte_identical_and_pending`'s core assertion was invalidated by this plan's own intended behavior change**
- **Found during:** Task 2, same test run as above
- **Issue:** Plan 01's test asserted the store was byte-identical after a wrong-code redemption, because Plan 01 shipped no attempt tracking. Plan 02 deliberately makes the attempt counter the wrong-code path's sole mutation (PAIR-02) — so the old assertion is now testing the wrong thing, not merely stale.
- **Fix:** Renamed to `wrong_code_burns_one_attempt_and_stays_pending`; replaced the byte-identical assertion with an explicit `attempts == 1` check plus a continued `Pending`-state check, and documented the behavior change in a doc comment pointing to `pairing_ingress.rs`'s coverage of the OTHER rejection paths (which still assert full byte identity).
- **Files modified:** `crates/famp-gateway/tests/pairing_e2e.rs`
- **Verification:** `cargo test -p famp-gateway --test pairing_e2e` — 5 passed, 0 failed
- **Committed in:** `32c91fb`

**3. [Rule 3 - Blocking] `ingest_redemption_at` exceeded the workspace's 100-line function-length lint after adding the Accept re-decide sequence**
- **Found during:** Task 2, `just lint`
- **Issue:** `clippy::too_many_lines` fired at 101/100 lines.
- **Fix:** Extracted the WrongCode-burn sequence into `burn_and_reject_wrong_code` and the Accept lock-and-consume sequence into `accept_and_consume`, both free functions taking explicit parameters (no lint config weakened, no `#[allow]` added).
- **Files modified:** `crates/famp-gateway/src/pairing_ingress.rs`
- **Verification:** `just lint` exits 0
- **Committed in:** `32c91fb`

**4. [Rule 1 - Bug] Two doc-comment-first-paragraph-too-long lint failures**
- **Found during:** Task 1 and Task 3, `just lint`
- **Issue:** `clippy::doc_lazy_continuation`-adjacent nursery lint on `MAX_ATTEMPTS`'s and `RedemptionDecision`'s doc comments, and separately on `revoke.rs`'s module doc.
- **Fix:** Split each into a short first sentence plus a blank line before the elaboration, per the rest of the file's existing convention.
- **Files modified:** `crates/famp/src/pairing/invite.rs`, `crates/famp/src/cli/pair/revoke.rs`
- **Verification:** `just lint` exits 0
- **Committed in:** `20e393f`, `170662a`

---

**Total deviations:** 5 auto-fixed (2 Rule 1 test-invalidation fixes, 1 Rule 1 lint fix batch across 2 commits, 1 Rule 3 function-length refactor)
**Impact on plan:** All necessary for correctness (the timestamp/behavior fixes) or to satisfy CLAUDE.md's lint-gate constraint without weakening any lint config. No scope creep.

## Falsification Control — Recorded Outcomes (PAIR-03, D4)

Per the plan's mandatory falsification protocol, `accept_and_consume` in `crates/famp-gateway/src/pairing_ingress.rs` was manually mutated to defer `save_atomic` until AFTER the signed response was fully built (simulating a process kill between "response ready" and "state persisted" — the exact bug T-18-03 exists to prevent), then run, then reverted and re-run. All four outcomes below were observed directly, not inferred:

| State | `replay_of_consumed_code_after_reload_is_rejected` | `correct_code_first_use_succeeds` |
|---|---|---|
| **Reverted ordering** (save_atomic deferred past response construction) | **RED** — panicked: `expected already_redeemed on replay, got Ok(Signed { ... })` | **GREEN** — first call still returns `Ok` (persistence timing does not affect first-use success) |
| **Correct ordering** (restored, `git commit` state) | **GREEN** | **GREEN** |

This confirms the restart test is a real, working falsification of the persist-before-reply ordering (it fails when the ordering breaks) and the control test is not accidentally coupled to the same fact (it stays green regardless, because the control's assertion is scoped to "the call returns `Ok`", deliberately NOT "the store was persisted" — an earlier draft of the control asserted persisted state too, which made both tests go red under the reverted ordering and defeated the control's purpose; this was caught and fixed during the falsification run itself, documented inline in the test file).

## Issues Encountered

- The initial version of `correct_code_first_use_succeeds` asserted both "returns Ok" AND "the store shows Redeemed" — under the reverted ordering both facts were still true synchronously (no true process kill is simulated in an in-process unit test), so the first falsification attempt showed BOTH tests going RED, which is uninformative (zero information per the plan's own falsification-needs-a-control guidance). Diagnosed and fixed by narrowing the control test to only the "succeeds" fact, re-run, and got the intended RED/GREEN split. See "Falsification Control" above.
- `assert_cmd::Command::cargo_bin("famp")` uses whatever binary is already on disk; running `cargo test -p famp --lib pair::revoke` alone (without a preceding `cargo build --bin famp`) picked up a stale pre-Revoke-variant binary and failed with "unrecognized subcommand 'revoke'". Fixed by running `cargo build -p famp --bin famp` before the process-level test.
- A full `timeout 600 cargo test --workspace --no-fail-fast` run (this plan's wave-merge verification command) did not finish within 600s — it was still running `famp-gateway`'s `e2e_cross_host_delivery` test, a slow/pre-existing network-touching e2e test unrelated to this plan's file scope (`e2e_two_daemons_rejects_unsigned`, also encountered mid-run, failed with a pre-existing `rustls` "No provider set" panic, also unrelated to pairing). Every command actually named in the plan's per-task `<verify>` blocks — `cargo test -p famp --lib pairing::invite`, `cargo test -p famp-gateway --test pairing_ingress`, `cargo test -p famp-gateway --test pairing_e2e`, `cargo test -p famp --lib pair::revoke`, `cargo test -p famp --lib pairing`, `just lint`, `cargo fmt --all -- --check` — was run individually and passed. The full-workspace run is flagged here as unverified rather than silently claimed green.

## Next Phase Readiness

- PAIR-02 and PAIR-03 are both closed at the mechanism level: attempt budget, TTL, single-use-across-restart, concurrency, and revoke all proven.
- `PairingError`'s new variants (`Expired`, `AlreadyRedeemed`, `AttemptsExhausted`, `WrongCode`, `UnknownInvite`) carry placeholder `Display` wording with `TODO(18-03)` markers — Plan 03 owns giving these real operator-facing messages.
- `crates/famp/tests/pair_cli.rs` (process-level CLI coverage) is still unwritten — explicitly Plan 03's file per this plan's `<read_first>` note ("Do not create `crates/famp/tests/pair_cli.rs` here").
- Layer 0 crates (`famp-envelope`, `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`) verified untouched after every task (`git status --porcelain | grep -E "famp-(envelope|canonical|crypto|core|fsm)/"` empty each time).

---
*Phase: 18-cross-person-trust-bootstrap-pairing*
*Completed: 2026-08-04*
