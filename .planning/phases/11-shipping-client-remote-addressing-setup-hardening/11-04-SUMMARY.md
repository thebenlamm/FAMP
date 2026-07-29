---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 04
subsystem: testing
tags: [tls, rustls-platform-verifier, cert-fixtures, e2e, gateway, falsification-control]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 03's mode-branched remote `famp send` (request/commit/deliver+terminal); plan 07's broker from-binding + egress own-domain enforcement; plan 01's un-swallowed transport error chain"
provides:
  - "cross_machine fixture certs regenerated to CA:FALSE(critical)+extendedKeyUsage=serverAuth, satisfying both Apple SecTrust (macOS) and webpki (Linux)"
  - "a dedicated system-trust (CA -> leaf delegation) TLS falsification-control test isolating Apple SecTrust's real EKU enforcement from the E2E's leaf-pinned-as-its-own-anchor `--trust-cert` shortcut"
  - "crates/famp-gateway/tests/common/gateway_harness.rs -- the two-host broker+gateway+TOFU-trust harness, mechanically extracted for reuse by any future gateway e2e test"
  - "crates/famp-gateway/tests/e2e_shipping_surface.rs -- permanent `just ci` regression net driving the real `famp send` CLI cross-host (happy path, full-cycle terminal FSM, observable negative)"
affects: [11-05, 11-06, any future gateway e2e test that needs the two-host harness]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CA -> leaf delegation TLS test chain (openssl-generated at test runtime) to isolate real chain-validation policy checks from the leaf-pinned-as-its-own-anchor `--trust-cert` shortcut that masks them"
    - "#[path] module extraction for shared integration-test harnesses across separate test binaries (mirrors the existing common/child_guard.rs convention)"
    - "own-domain-mismatch via `famp send --domain <bogus>` as the sanctioned way to produce a deliberately-invalid `from` through the real shipping CLI, rather than hand-constructing envelope JSON"

key-files:
  created:
    - crates/famp-gateway/tests/common/gateway_harness.rs
    - crates/famp-gateway/tests/e2e_shipping_surface.rs
  modified:
    - crates/famp/tests/fixtures/cross_machine/alice.crt
    - crates/famp/tests/fixtures/cross_machine/alice.key
    - crates/famp/tests/fixtures/cross_machine/bob.crt
    - crates/famp/tests/fixtures/cross_machine/bob.key
    - crates/famp/tests/fixtures/cross_machine/README.md
    - crates/famp-gateway/tests/e2e_cross_host_delivery.rs
    - crates/famp-transport-http/src/tls.rs

key-decisions:
  - "The falsification control (Task 1) observed the PRE-regen fixtures PASS on macOS via the E2E's actual `--trust-cert` config, not fail as the plan's 'expected finding-#5 mode' assumed -- confirming RESEARCH Open Q2's alternate hypothesis. Per the plan's own explicit branch for this outcome, added a dedicated system-trust (CA->leaf) test to `tls.rs` rather than trusting a same-config post-regen green."
  - "The full-cycle terminal test in Task 3 uses 3 legs (request/commit/deliver-terminal), not the 4-leg (...+ack) cycle e2e_cross_host_delivery.rs hand-builds -- the shipping `famp send` CLI (plan 03) has no 'ack' mode; terminal state is reached from the deliver envelope's own terminal_status header per famp-fsm::TaskFsm::step, so 3 legs already proves the claim."
  - "The D-09 negative test drives `famp send --domain local.bus` (a real CLI flag) rather than hand-constructing an envelope with from=agent:local.bus/... -- exercises the actual shipping-client code path the invariant protects, and is simpler than injecting raw JSON."

patterns-established:
  - "common/gateway_harness.rs is now the canonical two-host gateway e2e harness -- any new gateway integration test should #[path]-include it rather than re-copying Side/spawn_gateway/poll_* helpers."

requirements-completed: [TEST-03]

coverage:
  - id: D1
    description: "cross_machine fixture certs regenerated to CA:FALSE(critical)+extendedKeyUsage=serverAuth, loopback SANs; README corrected (was: false Ed25519 claim + deleted _gen_fixture_certs reference)"
    requirement: "TEST-03"
    verification:
      - kind: other
        ref: "openssl x509 -in alice.crt -text -noout | grep -A1 'Basic Constraints'/'Extended Key Usage' -- confirmed CA:FALSE + TLS Web Server Authentication on both alice.crt and bob.crt"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (post-regen)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Falsification control: pre-regen fixture behavior on macOS observed BEFORE regeneration, both poles of the branch-appropriate control named and confirmed green under real assertions"
    requirement: "TEST-03"
    verification:
      - kind: other
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery run against PRE-regen fixtures on macOS -- observed PASS (not the plan's assumed fail mode)"
        status: pass
      - kind: unit
        ref: "crates/famp-transport-http/src/tls.rs#tls::system_trust_eku_control::ca_delegated_leaf_enforces_eku_on_apple_sectrust -- must-fail pole (no-EKU CA-issued leaf) and must-pass pole (serverAuth-EKU CA-issued leaf) both asserted and observed correct"
        status: pass
    human_judgment: false
  - id: D3
    description: "Two-host gateway harness mechanically extracted to common/gateway_harness.rs; e2e_cross_host_delivery.rs stays behaviorally identical and green after the move"
    requirement: "TEST-03"
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (post-extraction)"
        status: pass
    human_judgment: false
  - id: D4
    description: "e2e_shipping_surface.rs: happy path (domain-qualified from asserted byte-equal), full-cycle terminal (both sides converge on COMPLETED), and observable negative (gateway-A stderr unpinned_key/403 + non-delivery on B)"
    requirement: "TEST-03"
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_shipping_surface -- shipping_send_happy_path_full_cycle_and_observable_negative"
        status: pass
    human_judgment: false

duration: ~55min (interactive; wall-clock spans a mid-session coordinator correction, see Issues Encountered)
completed: 2026-07-29
status: complete
---

# Phase 11 Plan 04: Cert regen + falsification control + shipping-surface e2e Summary

**Regenerated `cross_machine` TLS fixtures to CA:FALSE+serverAuth, ran a real macOS falsification control that found the E2E's `--trust-cert` config PASSES even on the OLD no-EKU fixtures (masking Apple SecTrust's real EKU check) and added a dedicated CA->leaf delegation test to prove the recipe actually matters, extracted the two-host gateway test harness for reuse, and shipped `e2e_shipping_surface.rs` — a permanent `just ci` regression net driving the real `famp send` CLI cross-host through happy-path, full-cycle-terminal-FSM, and an observable negative (mismatched-authority `from` rejected with `unpinned_key`/403).**

## Performance

- **Duration:** ~55 min of active tool-use across the session (Task 1 ~20:19-20:33, Task 2 ~20:33-20:38, Task 3 ~20:38-20:45, all 2026-07-28 local); the wall-clock session also included a mid-session interruption where a `cargo test --workspace` verification run was left backgrounded without proper monitoring — the coordinator intervened, supplied verified gate results directly, and the actual code/test work (all three task commits) was already complete and unaffected.
- **Started:** 2026-07-28T20:19Z (approx, first Read)
- **Completed:** 2026-07-29T01:15Z (SUMMARY write)
- **Tasks:** 3 completed
- **Files modified:** 8 (2 created, 6 modified)

## Accomplishments
- Regenerated `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` to the canonical D-08 recipe (RSA-2048, `basicConstraints=critical,CA:FALSE`, `keyUsage=critical,digitalSignature,keyEncipherment`, `extendedKeyUsage=serverAuth`, `127.0.0.1`/`localhost` SANs) and corrected the stale README (was claiming Ed25519 + a deleted `_gen_fixture_certs` generator; certs are actually RSA-2048 via `openssl`).
- Ran the falsification control (Task 1's mandatory first step) and got a genuinely informative, non-assumed result — see "Falsification Control" below.
- Added a dedicated system-trust CA->leaf delegation TLS test (`crates/famp-transport-http/src/tls.rs::system_trust_eku_control`, macOS-only) that isolates Apple SecTrust's real EKU enforcement from the `--trust-cert` leaf-pinning shortcut, with both control poles named and observed correct.
- Mechanically extracted the two-host broker+gateway+TOFU-trust test harness into `crates/famp-gateway/tests/common/gateway_harness.rs` (Task 2), keeping `e2e_cross_host_delivery.rs` behaviorally identical and green.
- Shipped `crates/famp-gateway/tests/e2e_shipping_surface.rs` (Task 3): a single test driving the FIXED `famp send` CLI (never the raw bus client, never the retired injector — confirmed absent from `git ls-files`) through a happy path, a full-cycle terminal-FSM proof, and an observable negative test.

## Task Commits

Each task was committed atomically:

1. **Task 1: falsification-control + regenerate cross_machine fixtures to CA:FALSE+serverAuth (D-08)** - `4c3abc6` (fix)
2. **Task 2: extract the two-host harness into tests/common/gateway_harness.rs (mechanical, no behavioral change)** - `470874c` (refactor)
3. **Task 3: shipping-surface e2e — happy + full-cycle terminal + observable negative (D-09)** - `921da5e` (feat)

**Plan metadata:** (this commit, following)

## Falsification Control (Task 1, mandatory)

Per the plan's explicit instruction, the control was run **before** any fixture regeneration:

```
cargo test -p famp-gateway --test e2e_cross_host_delivery   # PRE-regen fixtures (ECDSA P-256, no basicConstraints, no EKU)
-> gw01_gw02_gw03_two_process_cross_host_delivery ... ok
```

**This is the honest, observed result — not the plan's assumed outcome.** The plan's default expectation ("finding #5 mode") was that the pre-regen fixtures would FAIL on macOS with an Apple SecTrust `EkuError`. They did **not** — the test passed. This confirms RESEARCH Open Q2's alternate hypothesis: the E2E's `--trust-cert` client config (`Verifier::new_with_extra_roots`) trusts the peer's own self-signed leaf cert directly as an extra root, and when the presented leaf IS the pinned anchor, Apple SecTrust appears to short-circuit normal EKU/policy checks (a "this exact artifact is directly trusted" pinning shortcut) rather than doing real chain validation. A post-regen green under that same config would therefore prove nothing about EKU specifically — and indeed it doesn't (`gw01_gw02_gw03_two_process_cross_host_delivery` also passes post-regen).

Per the plan's own branch for a pre-regen-PASS outcome ("ADD a dedicated test exercising the SYSTEM-TRUST / no-extra-root client path... assert it FAILS on the OLD-shape cert and PASSES on the regenerated... recipe"), a new test was added: `crates/famp-transport-http/src/tls.rs::system_trust_eku_control::ca_delegated_leaf_enforces_eku_on_apple_sectrust` (`#[cfg(target_os = "macos")]`). Rather than testing a truly anchor-less config (which cannot ever "pass" for a self-signed cert — no informative must-pass pole is achievable there), it builds a genuine **CA -> leaf delegation chain** at test time via `openssl` (a self-signed CA trusted as the sole extra root, issuing two DIFFERENT leaves it signs): this reproduces the real chain-validation code path production TLS delegation would exercise, as opposed to the leaf-pinned-as-its-own-anchor shortcut. Both poles are named explicitly and were observed correct on this run:
- **MUST-FAIL pole:** a CA-issued leaf with no `extendedKeyUsage` — REJECTED. Confirmed.
- **MUST-PASS pole:** the same CA issuing a leaf WITH `extendedKeyUsage=serverAuth` (the D-08 recipe) — ACCEPTED. Confirmed.

Both poles green under real, distinct configurations (not the same config tested twice) is what makes this control informative, per the project's falsification-needs-a-control rule.

## Files Created/Modified
- `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` - regenerated to RSA-2048, CA:FALSE(critical), keyUsage, extendedKeyUsage=serverAuth, loopback SANs
- `crates/famp/tests/fixtures/cross_machine/README.md` - corrected (was: Ed25519 + deleted-generator claims)
- `crates/famp-transport-http/src/tls.rs` - new `system_trust_eku_control` test module (see Deviations)
- `crates/famp-gateway/tests/common/gateway_harness.rs` **(new)** - two-host broker+gateway+TOFU-trust harness, mechanically extracted from `e2e_cross_host_delivery.rs`
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` - moved helpers/consts removed, replaced with `#[path]` include + `use` of `gateway_harness`; behaviorally unchanged
- `crates/famp-gateway/tests/e2e_shipping_surface.rs` **(new)** - happy path + full-cycle terminal FSM + observable negative, driving the real `famp send` CLI

## Decisions Made
- **Falsification control branch:** since the pre-regen control PASSED (not failed) on macOS, followed the plan's explicit fallback instruction and added the CA->leaf delegation test rather than treating a post-regen green as proof.
- **Full-cycle terminal is 3 legs, not 4:** the shipping `famp send` CLI (plan 03) only mode-branches `request`/`commit`/`deliver+terminal_status` — there is no "ack" mode. Per `famp-fsm::TaskFsm::step` and the 11-03/11-07 SUMMARYs, terminal state is reached from the deliver envelope's own `terminal_status` header, so 3 legs already proves the terminal-FSM-reachable-via-shipping-surface claim (matches the plan's acceptance criteria wording, which only requires both sides to converge on `COMPLETED`).
- **D-09 negative test drives `famp send --domain local.bus`** (a real CLi flag) rather than hand-constructing a JSON envelope — exercises the actual shipping-client code path, and produces `from=agent:local.bus/alice` (mismatched against the pinned `agent:hosta.test/alice` label) through the real mode-branching logic.
- **Negative test asserts the `unpinned_key`/403 ingress slug** (gateway A's own-domain is left UNSET in this harness, matching `e2e_cross_host_delivery.rs`'s `spawn_gateway`, so plan 07's egress `FromDomainMismatch` pre-check is skipped and the envelope reaches gateway B's ingress as the plan's preferred deterministic branch requires).

## Deviations from Plan

### Auto-fixed / Plan-mandated additions

**1. [Plan's own explicit branch, not a deviation rule per se] `crates/famp-transport-http/src/tls.rs` modified — NOT in this plan's declared `files_modified`**
- **Found during:** Task 1, immediately after running the falsification control
- **What forced it:** Task 1's `<action>` text explicitly instructs: "If the pre-regen fixtures PASS on macOS... a post-regen green proves NOTHING. In that case ADD a dedicated test exercising the SYSTEM-TRUST / no-extra-root client path." The control observed exactly that outcome (see "Falsification Control" above) — the plan's own conditional logic, not scope creep, required a new test.
- **Why it belongs to D-08/D-09:** D-08 is specifically about the cross-platform (Apple SecTrust vs webpki) EKU/CA divergence being real and correctly mitigated by the recipe; without this test, the plan's own falsification-control requirement (stated in both the PLAN.md acceptance criteria and this executor's `<falsification_control_requirement>` directive) would be unsatisfied by a same-config pre/post comparison that carries zero information.
- **Files modified:** `crates/famp-transport-http/src/tls.rs` (+242 lines: one new `#[cfg(all(test, target_os = "macos"))] mod system_trust_eku_control` block — CA+leaf cert generation via `openssl` at test time, a bare axum HTTPS server, a raw system-trust reqwest client, and one test asserting both control poles).
- **Verification:** `cargo test -p famp-transport-http --lib system_trust_eku_control` passed (both poles); `cargo test -p famp-transport-http` (full crate, 20/20 unit + 4/4 integration) passed; `just lint` clean.
- **Committed in:** `4c3abc6` (Task 1 commit, alongside the fixture regen it validates).

**2. [Rule 3-adjacent — missing underlying capability] Full-cycle terminal test uses 3 legs instead of the plan's literal "request/commit/deliver-terminal/ack" wording**
- **Found during:** Task 3, reading `send/mod.rs::build_remote_envelope_value`
- **Issue:** The plan's Task 3 `<action>`/`<behavior>` text describes driving "request/commit/deliver-terminal/ack" via the shipping client. The shipping `famp send` CLI (as built in plan 03) only mode-branches three shapes — there is no "ack" mode.
- **Fix:** Implemented 3 legs (request, commit, deliver-terminal). Both sides still converge on `COMPLETED` via `poll_terminal_state`, satisfying the plan's stated acceptance criterion ("assert both sides converge on a terminal FSM state (COMPLETED)") — the "ack" leg was not required for that outcome per `famp-fsm::TaskFsm::step`'s documented terminal-transition semantics (11-03/11-07 SUMMARYs).
- **Files modified:** `crates/famp-gateway/tests/e2e_shipping_surface.rs`
- **Verification:** `shipping_send_happy_path_full_cycle_and_observable_negative` passes; `state_a == state_b == "COMPLETED"` asserted.
- **Committed in:** `921da5e` (Task 3 commit).

---

**Total deviations:** 2 (1 plan-mandated test addition outside declared `files_modified`, 1 scope adjustment forced by an upstream plan's actual implementation surface).
**Impact on plan:** Both are load-bearing for D-08/D-09's actual guarantees, not scope creep. No production behavior changed in either case — both are additive test-layer changes.

## Issues Encountered

- **`cargo test --workspace` verification was interrupted mid-session.** After all three task commits landed and were individually verified green (`cargo test -p famp-transport-http`, `cargo test -p famp-gateway --test e2e_cross_host_delivery --test e2e_shipping_surface`, `just lint` full-workspace — all clean), a `cargo test --workspace` full-suite confirmation run was launched. A first attempt was capped by a shell-level `timeout 900` wrapper that silently truncated the run at 15 minutes (the pipe's exit code reflected `tail`'s success, not the underlying `cargo test`/`timeout` outcome — a monitoring mistake, not a test failure). A second attempt was launched without the artificial cap; the coordinator then intervened directly with verified gate results before that second run's outcome was confirmed by this executor.
- **Per the coordinator's authoritative report, treat as ground truth:**
  - `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) → clean, exit 0.
  - `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery` → both green (`gw01_gw02_gw03_two_process_cross_host_delivery` 9.21s, `shipping_send_happy_path_full_cycle_and_observable_negative` 15.06s).
  - `cargo test --workspace` → **INCOMPLETE, not verified green.** The first attempt reached 40 test suites with 0 failures before being truncated by its own 900s timeout; it never got past the `famp` crate's `inspect_identities.rs` test file alphabetically, so `famp-bus`, `famp-canonical`, `famp-core`, `famp-crypto`, `famp-envelope`, `famp-fsm`, `famp-inspect-*`, `famp-keyring`, `famp-transport`, `famp-transport-http`'s own integration suite, doc-tests, and the remainder of `famp`'s and `famp-gateway`'s own test files were never reached in that run. **This plan does NOT claim the full workspace suite passed** — only the two crates this plan directly touches (`famp-transport-http`, `famp-gateway`) were independently confirmed green via targeted runs, matching the project's documented `cargo test --workspace` slowness (Memory: `project_nextest_list_hang`) and this plan's own `<critical_environment_notes>`.
- **Known non-regressions (per project Memory, not chased):** 5 codex install/uninstall tests can flake under `cargo test --workspace` due to `target/debug/famp` relinking races — not observed as failures in the 40 suites that did complete in this session's runs (`codex_install_uninstall_roundtrip.rs` 4/4 passed in the partial run). `http_happy_path` TimedOut (stale `target/`) also not observed — it passed (7.82s) in the partial run.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `crates/famp-gateway/tests/common/gateway_harness.rs` is now the canonical two-host gateway e2e harness; any future gateway integration test (11-05/11-06 or later) should `#[path]`-include it rather than re-copying `Side`/`spawn_gateway`/poll helpers.
- `e2e_shipping_surface.rs` is a permanent, additive `just ci` regression net (both `e2e_cross_host_delivery.rs` and `e2e_shipping_surface.rs` run in the existing `test` matrix job on both `ubuntu-latest` and `macos-latest` — no new CI job needed).
- **Open verification caveat carried forward:** the full `cargo test --workspace` suite was not confirmed green end-to-end in this session (see Issues Encountered). Whoever picks up 11-05/11-06 (or the phase verifier) should either re-run it to completion or rely on the per-crate targeted runs already performed here plus CI's own eventual full run.
- Falsification-control record (both poles) lives in this SUMMARY per the executor's `<falsification_control_requirement>` directive — no separate artifact needed.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-29*
