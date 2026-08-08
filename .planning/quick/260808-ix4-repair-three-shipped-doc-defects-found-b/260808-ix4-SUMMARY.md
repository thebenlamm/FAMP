---
phase: quick-260808-ix4
plan: 01
subsystem: docs
tags: [famp-gateway, famp-pair, doc-accuracy-gate, cli-usage-drift, rust-tests]

requires:
  - phase: 20-human-acceptance-gate
    provides: 20-PREATTEMPT-DEFECTS.md (D1, D2, D3 findings from code-reading + throwaway-host smoke test)
provides:
  - "docs/FOLLOWER-SETUP.md section 1: PATH-presence install check that actually exits 0"
  - "docs/FOLLOWER-SETUP.md section 2: real inbound-reachability requirement, sourced from redeem.rs, with a RELAY-SETUP.md pointer"
  - "docs/GATEWAY-SETUP.md: accurate relay claim, section 4 flag surface matching the shipping USAGE const"
  - "crates/famp/tests/follower_setup_doc_accuracy.rs + crates/famp-gateway/tests/follower_setup_gateway_commands.rs: compiled gate that executes guide commands and asserts exit status, closing the D3 hole"
affects: [phase-20-human-acceptance-gate, docs-relay-setup]

actuals:
  tokens: 5160
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Doc-accuracy gate now has an execution half, not just a flag-spelling half: pure classifier fn over &str, fed either the real doc or an in-memory mutated copy, so the red path never touches the working tree"
    - "Split test-per-crate-ownership: a test that invokes famp-gateway as a subprocess lives in crates/famp-gateway/tests/, never crates/famp/tests/, because cargo does not build a sibling crate's bin for that target"

key-files:
  created:
    - crates/famp-gateway/tests/follower_setup_gateway_commands.rs
  modified:
    - docs/FOLLOWER-SETUP.md
    - docs/GATEWAY-SETUP.md
    - README.md
    - crates/famp/tests/follower_setup_doc_accuracy.rs
    - crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs

key-decisions:
  - "D1 replacement command: `command -v famp-gateway`, chosen because famp-gateway's parse_args (crates/famp-gateway/src/main.rs ~line 291) rejects any unrecognized --flag with 'unrecognized argument ... — no such flag' and exits 1, and there is no --help/--version arm — the only zero-exit invocation of this binary is a PATH presence check."
  - "D2 section 2 rewrite states the reachability requirement directly (sourced from crates/famp/src/cli/pair/redeem.rs's direct HTTPS dial) instead of delegating to GATEWAY-SETUP.md's production procedure, which mandates a shared VPN and out-of-band key exchange -- both forbidden by this guide's own preamble."
  - "docs/RELAY-SETUP.md referenced by path only, never read or created -- it is being authored concurrently by another agent."

requirements-completed: []  # DOC-07 stays open -- this is prep work for the still-unrun clean-host rehearsal, not its completion. Per-deliverable `requirement: DOC-07` traceability below is retained.

coverage:
  - id: D1
    description: "docs/FOLLOWER-SETUP.md section 1 documents no command that exits non-zero; the famp-gateway+--help literal appears nowhere in the file"
    requirement: DOC-07
    verification:
      - kind: unit
        ref: "crates/famp/tests/follower_setup_doc_accuracy.rs#section1_commands_execute_or_are_classified_non_hermetic"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/tests/follower_setup_gateway_commands.rs#documented_path_presence_check_executes_and_exits_zero"
        status: pass
    human_judgment: false
  - id: D2
    description: "FOLLOWER-SETUP.md section 2 states the real inbound-reachability requirement and links RELAY-SETUP.md instead of routing to GATEWAY-SETUP.md's forbidden-topology procedure; GATEWAY-SETUP.md no longer asserts no public relay exists and its flag surface matches the shipping USAGE const"
    requirement: DOC-07
    verification:
      - kind: unit
        ref: "crates/famp/tests/follower_setup_doc_accuracy.rs#follower_setup_is_ordered_and_matches_shipping_surfaces"
        status: pass
      - kind: unit
        ref: "crates/famp/tests/gateway_setup_doc_accuracy.rs#gateway_setup_doc_accuracy"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs#gateway_usage_doc_accuracy"
        status: pass
    human_judgment: false
  - id: D3
    description: "compiled gate fails if FOLLOWER-SETUP.md ever again documents a famp-gateway invocation that exits non-zero; red path demonstrated live (mutated copy trips, real doc does not)"
    requirement: DOC-07
    verification:
      - kind: unit
        ref: "crates/famp/tests/follower_setup_doc_accuracy.rs#section1_red_path_trips_on_prerepair_help_invocation"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/tests/follower_setup_gateway_commands.rs#red_path_trips_on_prerepair_invocation"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/tests/follower_setup_gateway_commands.rs#help_flag_invocation_fails_and_is_not_documented"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-08-08
status: complete
---

# Quick Task 260808-ix4: Repair Three Shipped-Doc Defects Summary

**Replaced FOLLOWER-SETUP.md's broken `famp-gateway --help` install check with `command -v famp-gateway`, rewrote its reachability guidance to state the real inbound-HTTPS requirement instead of delegating to a forbidden-topology doc, corrected GATEWAY-SETUP.md's false "no public relay" claim and stale flag surface, and closed the doc-accuracy gate's execution hole with two new test files that actually run the guide's commands and assert exit status.**

## Performance

- **Duration:** ~25 min
- **Tasks:** 3/3 completed
- **Files modified:** 5 modified, 1 created
- **Commits:** 3 task commits (no separate metadata commit per plan's `.planning/` gitignore note -- see below)

## Accomplishments

- **D1 fixed.** `docs/FOLLOWER-SETUP.md` section 1's fenced block now runs `command -v famp-gateway` instead of `famp-gateway --help`. Source evidence: `crates/famp-gateway/src/main.rs` line 215-217 defines `const USAGE` with no `--help`/`--version` arm, and lines 291-295's catch-all match arm returns `Err(format!("famp-gateway: unrecognized argument '{other}' — no such flag\n{USAGE}"))` for any unrecognized `--`-prefixed token, confirmed live in the D1 finding's smoke test (`famp-gateway --help` exit=1 on a fresh Linux host). `command -v famp-gateway` is the only zero-exit probe this binary supports.
- **D2 fixed.** Section 2 now states directly (with a `redeem.rs` citation) that the inviter's gateway URL must be inbound-reachable before pairing, since `famp pair redeem --from <url>` (`crates/famp/src/cli/pair/redeem.rs`) POSTs to `<url>/famp/v1/pair/redeem` with no relay branch. It links `[Relay Setup](RELAY-SETUP.md)` for sections 5/6's bidirectional transport need, and scopes `GATEWAY-SETUP.md` to certs/firewall/flags/own-domain only, explicitly instructing the reader to skip its out-of-band key exchange (without naming the forbidden `famp peer export`/`import` literals).
- **D2 (gateway half) fixed.** `docs/GATEWAY-SETUP.md`'s opening paragraph no longer claims "There is no public relay" — it now states the guide covers the direct topology, acknowledges `famp-relay` + `--relay-fetch` with a `RELAY-SETUP.md` pointer, and records that pairing is always a direct dial. Section 4's usage block and flag table now include `--backs`, `--relay-fetch`, and `--pairing-store`, matching the shipping `USAGE` const verbatim (`crates/famp-gateway/src/main.rs` lines 215-217). README's paraphrase of the same falsehood is gone.
- **D3 fixed.** Two new test functions (`section1_commands_execute_or_are_classified_non_hermetic` in `crates/famp/tests/follower_setup_doc_accuracy.rs`, plus the whole of `crates/famp-gateway/tests/follower_setup_gateway_commands.rs`) actually execute the guide's section 1 commands and assert exit status, rather than only checking flag spellings. An unclassified section 1 command fails the suite by name.

## Task Commits

1. **Task 1: Repair FOLLOWER-SETUP.md sections 1 and 2 (D1 doc half, D2 follower half)** - `de5aa1f` (fix)
2. **Task 2: Correct the false relay claim and the stale flag surface in GATEWAY-SETUP.md (D2 gateway half)** - `624ef80` (fix)
3. **Task 3: Close the gate hole -- execute the guide's verification commands and assert exit status (D3)** - `185dd9a` (test)

No separate plan-metadata commit: `.planning/` is gitignored in this repo (per project memory), so this SUMMARY.md is not committed to git history.

## Files Created/Modified

- `docs/FOLLOWER-SETUP.md` - D1 install-check swap + prose; D2 section 2 fully rewritten
- `docs/GATEWAY-SETUP.md` - D2 opening-paragraph relay claim corrected; section 4 usage block + flag table extended
- `README.md` - "no public relay" clause deleted from Quick Start paragraph
- `crates/famp/tests/follower_setup_doc_accuracy.rs` - D3 famp-binary half: `section1_lines`/`classify_section1_commands` pure helpers, `EXECUTED_VERIFICATIONS`/`NON_HERMETIC` consts, green-path + red-path tests
- `crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs` - `GATEWAY_FLAGS` const extended with `--backs`, `--relay-fetch`, `--pairing-store`
- `crates/famp-gateway/tests/follower_setup_gateway_commands.rs` (new) - D3 famp-gateway-binary half: PATH-presence check execution, `--help` failure proof, in-memory red path

## Decisions Made

- D1 replacement command is `command -v famp-gateway`, per the plan's pre-verified F1 fact (no zero-exit invocation of `famp-gateway` exists otherwise).
- D2 section 2 states the reachability requirement directly rather than delegating to `GATEWAY-SETUP.md`, because that doc's production procedure mandates a shared VPN and out-of-band key exchange, both forbidden by `FOLLOWER-SETUP.md`'s own preamble.
- `docs/RELAY-SETUP.md` referenced by path only in both guides -- never read, created, or modified, per the plan's explicit out-of-scope instruction (another agent is authoring it concurrently).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `just lint` failed on a `clippy::const_is_empty` false positive**
- **Found during:** Task 3 verification (`just lint`)
- **Issue:** `assert!(!EXECUTED_VERIFICATIONS.is_empty(), ...)` on a single-entry `const` array triggers clippy's `const_is_empty` lint (`-D warnings` makes it a build failure) because clippy can statically prove the assertion is currently always true. The assertion is intentional future-proofing (guards against the classification list becoming vacuous), not dead code.
- **Fix:** Added `#[allow(clippy::const_is_empty)]` on the test function with a comment explaining the assertion guards a future state, not a currently-reachable one.
- **Files modified:** `crates/famp/tests/follower_setup_doc_accuracy.rs`
- **Verification:** `just lint` passes clean afterward; `cargo test -p famp --test follower_setup_doc_accuracy` still green.
- **Committed in:** `185dd9a` (Task 3 commit)

**2. [Rule 3 - Blocking] `cargo fmt --all -- --check` failed on my own new code**
- **Found during:** Task 3 verification
- **Issue:** Two spots in the newly-added helper/test code did not match rustfmt's canonical formatting (a wrapped `if` condition and a misaligned trailing comment).
- **Fix:** Ran `rustfmt crates/famp/tests/follower_setup_doc_accuracy.rs` -- scoped to the single file this task owns, not `cargo fmt --all` (which would have reformatted the concurrently-edited `docs/RELAY-SETUP.md`-adjacent work of another agent).
- **Files modified:** `crates/famp/tests/follower_setup_doc_accuracy.rs`
- **Verification:** `cargo fmt --all -- --check` passes clean afterward across the whole tree.
- **Committed in:** `185dd9a` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 3 -- blocking lint/fmt issues in newly-authored test code)
**Impact on plan:** Both fixes are mechanical and scoped entirely to files this plan owns. No scope creep, no behavior change.

## Red-Path Demonstration (evidence, not assertion)

Per the plan's `<red_path_requirement>`, both red-path tests were manually broken and observed to fail on the real (repaired) doc before being restored to their correct mutation-based form.

This is the on-disk version of the demonstration, matching the plan's exact must-have wording: "with the pre-repair section 1 line restored, the new tests fail while the pre-existing accuracy test still passes." The fenced block's `command -v famp-gateway` line (docs/FOLLOWER-SETUP.md line 18) was edited back to `famp-gateway --help` on disk, all three tests below were run against that single broken state, and the file was restored via `git checkout -- docs/FOLLOWER-SETUP.md`.

**Must-fail #1 -- `section1_commands_execute_or_are_classified_non_hermetic` (famp crate):**
```
thread 'section1_commands_execute_or_are_classified_non_hermetic' panicked at
crates/famp/tests/follower_setup_doc_accuracy.rs:221:32:
unclassified section 1 command: famp-gateway --help
test result: FAILED. 0 passed; 1 failed; ...
```

**Must-fail #2 -- `help_flag_invocation_fails_and_is_not_documented` (famp-gateway crate):**
```
thread 'help_flag_invocation_fails_and_is_not_documented' panicked at
crates/famp-gateway/tests/follower_setup_gateway_commands.rs:89:5:
D1 regression: docs/FOLLOWER-SETUP.md documents `famp-gateway --help`, which
the binary invocation above just proved exits non-zero
test result: FAILED. 0 passed; 1 failed; ...
```

**Must-pass (control) -- `follower_setup_is_ordered_and_matches_shipping_surfaces`, the pre-existing spelling-level gate:**
```
test follower_setup_is_ordered_and_matches_shipping_surfaces ... ok
test result: ok. 1 passed; 0 failed; ...
```
Confirms the defect log's central claim in the same broken state the two new gates just caught: the pre-existing flag-spelling gate is blind to D1 (a spelling-correct, non-zero-exit command) and passes right through it.

Restored via `git checkout -- docs/FOLLOWER-SETUP.md`; `git status --short` showed the file byte-identical to its pre-experiment committed state (`git diff docs/FOLLOWER-SETUP.md` empty); full suite re-run green (`follower_setup_doc_accuracy`: 4/4 ok, `follower_setup_gateway_commands`: 3/3 ok, `gateway_usage_doc_accuracy`: 1/1 ok).

The in-memory red-path tests (`section1_red_path_trips_on_prerepair_help_invocation`, `red_path_trips_on_prerepair_invocation`) that run on every CI pass were also each verified to fail correctly when their internal mutation was temporarily disabled (an earlier check on the mechanism itself, superseded by the on-disk demonstration above as the authoritative evidence for the plan's must-have).

## Issues Encountered

None beyond the two auto-fixed lint/fmt issues documented above.

## Guide Freeze Now Invalid

The Phase 20 rehearsal candidate's frozen guide commit/digest is **INVALID** as of this plan's commits:

```text
guide_commit=f848c9e747ad769a162408249a8dd084f34e2350
guide_digest=43f793114a9e51cf2a94c86dea47077cc1b800c2b344d81fa0bcc04eb6e1a01c
```

`docs/FOLLOWER-SETUP.md` has changed (commit `de5aa1f`, D1+D2 repairs) since that freeze was taken. Re-freezing `guide_commit`/`guide_digest` and rewriting `20-REHEARSAL.md`'s candidate block is a **separate follow-up step**, explicitly out of scope for this plan and not performed here. `20-REHEARSAL.md` and `20-02-SUMMARY.md` were not touched.

## Next Phase Readiness

- All three defects (D1, D2, D3) repaired and verified; `docs/RELAY-SETUP.md` remains untouched (authored concurrently by another agent) and is referenced by path only from both guides.
- Before the DOC-07 clean-host attempt can resume: (1) re-run Plan 20-01's full test suite, (2) re-freeze `guide_commit`/`guide_digest` against the new `docs/FOLLOWER-SETUP.md` content, (3) rewrite `20-REHEARSAL.md`'s candidate block against the new freeze. None of that is done by this plan.
- Once `docs/RELAY-SETUP.md` lands from the concurrent agent, spot-check that its content matches what `FOLLOWER-SETUP.md` section 2 and `GATEWAY-SETUP.md`'s opening paragraph now promise (both link it but neither describes its mechanics, per this plan's explicit scope boundary).

---
*Task: quick-260808-ix4*
*Completed: 2026-08-08*

## Self-Check: PASSED

All 6 modified/created source files confirmed present on disk; all 3 task commit hashes (`de5aa1f`, `624ef80`, `185dd9a`) confirmed present in `git log --oneline --all`.
