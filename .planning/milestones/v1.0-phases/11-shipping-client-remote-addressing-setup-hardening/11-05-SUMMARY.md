---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 05
subsystem: docs
tags: [gateway-setup, doc-accuracy, tls, own-domain, remote-addressing, semantic-gate]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 02's own-domain config surface (--domain / FAMP_OWN_DOMAIN / $FAMP_HOME/own-domain)"
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 03's fixed famp send --to agent:<domain>/<name> remote-addressing"
  - phase: 10-test-reactivation-setup-docs
    provides: "the 8 Gate A dogfood findings (10-HUMAN-UAT.md) and the original flag-grep gateway_setup_doc_accuracy.rs"
provides:
  - "corrected docs/GATEWAY-SETUP.md — all 8 Gate A dogfood findings fixed"
  - "own-domain config surface + famp send remote-addressing documented in the guide"
  - "extended crates/famp/tests/gateway_setup_doc_accuracy.rs with semantic (not just flag-grep) assertions"
affects: [10-HUMAN-UAT.md re-run readiness (guide can now be dogfooded again), 11-08 (gateway trust-boundary hardening — same file family)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Doc-accuracy gate normalizes whitespace (doc.split_whitespace().join(' ')) before matching multi-word anchor phrases, so markdown line-wrap can never split an assertion across a literal newline"
    - "Negative pin-label guard regex built from string parts at test runtime (prefix/middle/suffix concatenated), never written verbatim as a single literal, so the file itself doesn't reintroduce the banned shape"

key-files:
  created: []
  modified:
    - docs/GATEWAY-SETUP.md
    - crates/famp/tests/gateway_setup_doc_accuracy.rs

key-decisions:
  - "Renumbered the guide from 5 to 6 sections: inserted a new §5 'Configure your own-domain' before the renamed §6 'Connect / verify', since own-domain must be set before any remote famp send and belongs conceptually right before the send/verify walkthrough."
  - "Wrote the wiring-direction assertion against whitespace-normalized text rather than the raw doc string, after discovering the first draft's exact-substring check would have silently broken on markdown line-wrap (the phrase 'backs the remote principal `alice`' straddled a line break in the raw file)."
  - "Added #![allow(clippy::too_many_lines)] to the test file (154-line single test fn) rather than splitting into helpers — keeps every assertion co-located with the failure message that explains which Gate A finding it guards, mirroring the doc's own section-by-section structure."

patterns-established:
  - "Doc-accuracy semantic gates prove their own bite by construction: this plan's Task 2 re-inverted one directional statement live, confirmed the gate goes red, then reverted and confirmed green — documented below as the RED/GREEN pole evidence the plan's success criteria required."

requirements-completed: [DOC-05]

coverage:
  - id: D1
    description: "GATEWAY-SETUP.md §4 corrected: gateway backs the REMOTE principal it proxies (A backs bob, B backs alice), not the local one"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (directional wiring assertions)"
        status: pass
    human_judgment: false
  - id: D2
    description: "GATEWAY-SETUP.md §3 pin label corrected to the sender AGENT principal (agent:{domain}/{name}), never a gateway-suffixed label"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (sender-agent-principal + scoped negative regex assertions)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Guide states pin-before-launch / no-hot-reload, warns on the duplicate-pubkey keyring brick, and documents the ready line as printing AFTER keyring load"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (keyring-load-once phrase + ready-after-keyring byte-offset ordering assertions)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Guide replaces 'self-signed is fine' with the CA:FALSE+serverAuth cert recipe (verifies on macOS AND Linux) and documents a macOS host-firewall (socketfilterfw) pre-auth step"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (CA:FALSE / serverAuth / socketfilterfw token presence)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Guide documents the own-domain config surface (--domain / FAMP_OWN_DOMAIN / $FAMP_HOME/own-domain) and famp send --to agent:<domain>/<name> remote-addressing usage"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (FAMP_OWN_DOMAIN / own-domain / famp send --to agent: token presence)"
        status: pass
    human_judgment: false
  - id: D6
    description: "Doc-accuracy gate performs SEMANTIC checks (wiring direction, pin-label = agent principal, ordering, cert policy) beyond the pre-existing --help flag-grep, and demonstrably catches a reintroduced semantic inversion"
    requirement: "DOC-05"
    verification:
      - kind: integration
        ref: "cargo test -p famp --test gateway_setup_doc_accuracy (full suite green; manually re-inverted wiring direction to confirm RED, reverted to confirm GREEN — see Verification Performed)"
        status: pass
    human_judgment: false

# Metrics
duration: ~15min
completed: 2026-07-29
status: complete
---

# Phase 11 Plan 05: GATEWAY-SETUP.md correction + semantic doc-accuracy gate Summary

**Corrected `docs/GATEWAY-SETUP.md` for all 8 Gate A dogfood findings (inverted wiring, wrong pin label, keyring-brick/ordering, cert guidance, macOS firewall), documented the new own-domain config surface and `famp send --to agent:` remote usage, and extended `gateway_setup_doc_accuracy.rs` with semantic assertions proven to catch a reintroduced wiring inversion (RED) and pass on the corrected doc (GREEN).**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-07-28T21:15:00-04:00 (approx, first Read)
- **Completed:** 2026-07-28T21:25:06-04:00
- **Tasks:** 2 completed
- **Files modified:** 2

## Accomplishments
- Fixed all 8 Gate A dogfood findings in `docs/GATEWAY-SETUP.md`: reversed §4 wiring direction (A backs remote `bob`, B backs remote `alice`), corrected §3's pin label to the sender AGENT principal (`agent:hostA.example/alice` / `agent:hostB.example/bob`, never a `gateway`-suffixed label), added pin-before-launch / keyring-load-once / duplicate-pubkey-brick guidance, moved the documented `ready` signal to after keyring load, replaced "self-signed is fine" with the CA:FALSE+serverAuth cert recipe (verifies on both Apple SecTrust and webpki), and added a macOS `socketfilterfw` firewall pre-auth step.
- Added a new §5 documenting the own-domain config surface (precedence `--domain` > `FAMP_OWN_DOMAIN` > `$FAMP_HOME/own-domain`) and rewrote §6 (renumbered from §5) to drive the real shipping `famp send --to agent:<domain>/<name> --new-task ...` client instead of the old non-functional `/famp-send` recipe.
- Added a known-limitation note on bare-leaf-name routing ambiguity across local/remote holders, deferred to v1.1.
- Extended `gateway_setup_doc_accuracy.rs` with 13 new semantic assertions covering every finding above (positive presence checks for cert tokens, own-domain tokens, firewall step, agent-principal pin instruction, directional wiring; a scoped negative regex guard against a gateway-suffixed pin label; and a byte-offset ordering check that the `ready` mention appears after the keyring-load mention) — all operating on whitespace-normalized doc text so markdown line-wrap can't split an anchor phrase.
- **Proved the gate has teeth (plan success criterion):** manually re-inverted the §4 wiring statement back to the pre-fix (wrong) form, confirmed the test fails (RED), then reverted and confirmed it passes again (GREEN) — see Verification Performed.

## Task Commits

Each task was committed atomically:

1. **Task 1: correct GATEWAY-SETUP.md for all 8 findings + document own-domain + remote send** - `a9cdf20` (docs)
2. **Task 2: extend the doc-accuracy gate with semantic assertions** - `de6bde0` (test)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `docs/GATEWAY-SETUP.md` - all 8 Gate A findings corrected; new §5 own-domain config subsection; §6 (renamed from §5) drives real `famp send --to agent:` remote usage; known-limitation note on bare-leaf-name ambiguity
- `crates/famp/tests/gateway_setup_doc_accuracy.rs` - 13 new semantic assertions (cert policy, own-domain, firewall, pin-label authority + scoped negative guard, directional wiring, keyring/ready ordering) added after the pre-existing flag-grep block, which is unmodified

## Decisions Made
- Renumbered the guide to 6 sections (inserted own-domain config as its own §5 before the renamed §6 Connect/verify) rather than folding own-domain into an existing section — it's a genuinely separate operator action that must happen once, before any remote send.
- Wrote the directional-wiring and ordering assertions against `doc.split_whitespace().collect::<Vec<_>>().join(" ")` (whitespace-normalized) rather than the raw string, after the first draft's literal-substring check silently failed because markdown's automatic line-wrap split an anchor phrase (`backs the remote principal `alice``) across a newline in the raw file bytes.
- Built the negative pin-label regex from concatenated string parts at test runtime rather than as a single literal, per the plan's explicit instruction, so this test file itself never contains the banned shape verbatim.
- Added `#![allow(clippy::too_many_lines)]` to the test file instead of splitting the single test function into helpers, keeping every assertion's failure message next to the Gate A finding it documents (mirrors the doc's own section structure); `just lint` was clean with this allow in place.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `too_many_lines` clippy lint on the extended test function**
- **Found during:** Task 2, `just lint` run after extending the test
- **Issue:** The single `gateway_setup_doc_accuracy` test function grew to 154 lines (pedantic `clippy::too_many_lines` threshold is 100), blocking `just lint` / CI / the pre-push hook.
- **Fix:** Added a scoped `#![allow(clippy::too_many_lines)]` at the top of the file with an explanatory comment, following the codebase's existing pattern of narrowly-scoped pedantic-lint allows (mirrors `e2e_shipping_surface.rs`'s own `too_many_lines` allow).
- **Files modified:** `crates/famp/tests/gateway_setup_doc_accuracy.rs`
- **Verification:** `just lint` clean afterward.
- **Committed in:** `de6bde0` (Task 2 commit)

**2. [Rule 1 - Bug] Markdown line-wrap silently broke a literal-substring assertion**
- **Found during:** Task 2, first `cargo test` run of the new gate against the corrected doc
- **Issue:** The raw doc file wraps `backs the remote principal` and the following `` `alice` `` across a line break with no trailing space, so an exact-substring match against the un-normalized doc string failed even though the doc content was correct.
- **Fix:** Normalized the doc text via `split_whitespace().join(" ")` before running all semantic (non-flag-grep) assertions, and fixed one incidental markdown artifact in the doc itself (`**after**` splitting the intended anchor phrase — changed to plain `after`).
- **Files modified:** `crates/famp/tests/gateway_setup_doc_accuracy.rs`, `docs/GATEWAY-SETUP.md`
- **Verification:** `cargo test -p famp --test gateway_setup_doc_accuracy` green.
- **Committed in:** `de6bde0` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — bugs surfaced while building/running the new gate itself, not scope changes to the doc's substantive corrections)
**Impact on plan:** Both fixes were necessary for the gate to compile/pass cleanly; no scope creep beyond what Task 2 already required.

## Issues Encountered
None beyond the two auto-fixed items above.

## RED/GREEN Pole Evidence (plan success criterion)

Per the plan's explicit instruction to prove the gate "actually FAILS on a reintroduced semantic inversion":

1. **GREEN (corrected doc):** `cargo test -p famp --test gateway_setup_doc_accuracy` → `test result: ok. 1 passed`.
2. **RED (manually re-inverted):** temporarily changed §4's wiring text from "on A, the gateway backs the remote principal `bob` ... On B, the gateway backs the remote principal `alice`" back to the pre-fix (wrong) form "on A, the gateway backs the local principal `alice`. On B, the gateway backs the local principal `bob`." Re-ran the test → **failed** with:
   ```
   thread 'gateway_setup_doc_accuracy' panicked at crates/famp/tests/gateway_setup_doc_accuracy.rs:181:5:
   update the guide or the code: guide §4 must state that A's gateway backs the remote principal `bob` (finding #1 — wiring was inverted in the original guide)
   ```
3. **Reverted to GREEN:** restored the corrected wiring text; re-ran the test → `test result: ok. 1 passed` again. `git diff docs/GATEWAY-SETUP.md` against the Task 1 commit confirmed the only remaining delta was the intentional `**after**` → `after` markdown fix, not an accidental content change.

This confirms the gate is a real regression net for finding #1 (and, by construction, the other semantic assertions added alongside it), not a test that would pass regardless of doc content.

## User Setup Required

None for this plan. Operational note carried forward: a real two-machine dogfood re-run against this corrected guide (10-HUMAN-UAT.md) still requires a human (Ben) running it on real hardware — this plan only fixes the guide's text and strengthens the automated accuracy gate; it does not itself constitute the human-verified re-run.

## Next Phase Readiness
- `docs/GATEWAY-SETUP.md` is ready for a re-run of the 10-HUMAN-UAT.md dogfood scenario — all 8 findings are corrected and the guide now documents the shipping remote-send path (plan 03) instead of the non-functional `/famp-send` recipe the original dogfood hit.
- Plan 07 (which owns `crates/famp-gateway/src/main.rs`) still needs to make the CODE move — printing `ready` after keyring load — to match what this plan's doc now describes; this plan intentionally did not touch `main.rs` (per the plan's own note, avoiding a two-plan conflict on that file).
- No blockers for 11-06 or 11-08.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-29*
