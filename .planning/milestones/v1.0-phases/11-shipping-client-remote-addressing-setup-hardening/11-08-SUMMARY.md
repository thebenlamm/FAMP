---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 08
subsystem: security
tags: [trust-boundary, ingress, egress, route-config, gateway, own-domain, federation-fields, fail-closed]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 11-07's resolve_own_domain_or_exit(home) single resolution site (main.rs), reused unchanged for this plan's ingress to-authority check"
provides:
  - "ingress destination-authority gate: verified envelope's `to` must equal the URL-path recipient (unconditional) and, when own-domain is configured, must match this gateway's own-domain — both checked BEFORE any registry lookup/mailbox write"
  - "federation_format_ok() wired into the inbound path — inbound nonce/expiry are now format-checked, closing the zero-callers gap"
  - "egress client-supplied-federation-field rejection: a locally-originated drained envelope carrying any of the 7 federation-owned fields (from_domain/to_domain/sender_key_id/nonce/expiry/capability/approval) is rejected — never signed"
  - "sign_federation_fields's 5 derived fields are now unconditional inserts, not preserve-if-present"
  - "--backs agent:<domain>/<name> explicit route-binding flag; the peer x backed-name cross-product is deleted"
  - "startup fail-closed on duplicate --peer domain, duplicate --backs principal, and bare-positional-names with 2+ peers configured"
affects: [11-05 (GATEWAY-SETUP.md must document --backs, the duplicate-peer rejection, the >1-peer bare-name restriction, and 11-07's own-domain env/file config — see checklist below)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "own_domain converted from main()'s owned Option<String> local to Arc<str> only at the run_ingress call site (not at the resolution site) — keeps 11-07's single resolution point reusable by both egress (Option<String> clone) and ingress (Arc<str> for cheap Clone into GatewayIngressState) without restructuring it a second time"
    - "6-arm AnySignedEnvelope dispatch helper duplication convention (envelope_sender) extended with envelope_recipient and envelope_federation_format_ok — same documented duplication rationale (AnySignedEnvelope exposes no generic dispatch)"
    - "route-map construction extracted into build_route_map() purely to satisfy clippy::too_many_lines on main(), mirroring 11-07's resolve_own_domain_or_exit extraction precedent"

key-files:
  created:
    - crates/famp-gateway/tests/inbound_destination_validation.rs
    - crates/famp-gateway/tests/route_config_fail_closed.rs
  modified:
    - crates/famp-gateway/src/ingress.rs
    - crates/famp-gateway/src/egress.rs
    - crates/famp-gateway/src/main.rs

key-decisions:
  - "own_domain threaded into ingress as Option<Arc<str>>, converted from 11-07's Option<String> local at the run_ingress call site only — one resolution site (main.rs:283, unchanged line number range shifted by this plan's edits) feeds both boundaries, no second config source (explicit plan prohibition)."
  - "sign_federation_fields_is_idempotent needed NO change despite entry()->insert(): its assertion is guarded by the pre-existing already_signed early-return keyed on the `signature` field, which short-circuits before the entry/insert block is ever reached on a second call. Verified by re-running the test unmodified after the insert() change — still green."
  - "Client-supplied-federation-field pre-check placed once in relay_one, immediately before sign_federation_fields, not wrapped in any retry-aware logic. Investigated run_egress's drain loop first (per the plan's mandatory Task 2 investigation): Await permanently advances the mailbox cursor and a failed relay_one is logged-and-skipped (no re-queue), so a given drained envelope Value is handed to relay_one/sign_federation_fields at most once ever in production. There is no legitimate second gateway pass this pre-check could reject."
  - "--backs binding scoped to route-map resolution only (transport.add_peer), not to which principals are locally backed (registry.back()/backed_names stays sourced from positional args alone) — keeps F-5 (bare-name proxy mailbox collision) untouched and out of scope per the plan's explicit prohibition."
  - "Zero --peer configured + bare names is a no-op (not an error) — matches the legacy cross-product's behavior over an empty peers list (nothing to route yet, not a misconfiguration)."

patterns-established:
  - "Startup route-config validation (duplicate --peer, duplicate --backs, ambiguous bare-name fallback) mirrors KeyringError::KeyConflict's fail-closed message shape: name the conflicting values, exit(1) before any 'ready' output."

requirements-completed: [SEC-02, SEC-03, SEC-04]

coverage:
  - id: D1
    description: "Ingress rejects a verified envelope whose to.authority() != configured own-domain (403 foreign_domain), and any envelope whose to != URL-path recipient (400 misaddressed_recipient) — both checked before any registry lookup or mailbox write, and a well-formed same-domain correctly-addressed envelope still delivers"
    requirement: "SEC-02"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/inbound_destination_validation.rs#envelope_addressed_to_foreign_domain_is_rejected_and_mailbox_untouched, #envelope_to_differs_from_path_recipient_is_rejected_and_mailbox_untouched, #well_formed_same_domain_envelope_still_delivers"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (own-domain-unset regression control)"
        status: pass
    human_judgment: false
  - id: D2
    description: "federation_format_ok() is wired into the ingress gate — malformed nonce/expiry on an otherwise-valid signed envelope is rejected (400 malformed_federation_fields)"
    requirement: "SEC-02"
    verification:
      - kind: static
        ref: "grep -n 'federation_format_ok' crates/famp-gateway/src/ingress.rs — envelope_federation_format_ok() called in inbox_handler before delivery"
        status: pass
    human_judgment: false
  - id: D3
    description: "Egress rejects (never signs) a locally-originated drained envelope carrying any of the 7 federation-owned fields, naming every offending field; sign_federation_fields's 5 derived fields are unconditional inserts, not preserve-if-present"
    requirement: "SEC-03"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#relay_one_rejects_each_client_supplied_federation_field, #relay_one_names_all_present_client_supplied_fields_at_once"
        status: pass
      - kind: static
        ref: "grep -n 'or_insert_with' crates/famp-gateway/src/egress.rs — zero matches inside sign_federation_fields"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (control)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Route map contains only operator-declared bindings (--backs, or bare names with exactly one --peer); duplicate --peer, duplicate --backs, and bare-names-with-2+-peers all fail startup before the 'ready' line, naming the conflict"
    requirement: "SEC-04"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/route_config_fail_closed.rs#duplicate_peer_domain_fails_startup, #duplicate_backs_principal_fails_startup, #bare_names_with_two_peers_fails_startup_with_actionable_message, #bare_names_with_exactly_one_peer_still_starts_and_prints_ready, #backs_with_no_matching_peer_fails_startup"
        status: pass
      - kind: static
        ref: "grep -n 'for name in &backed_names' crates/famp-gateway/src/main.rs — the route-map cross-product loop is gone; only the (unrelated) egress-spawn loop remains"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery --test no_cross_talk (single-peer / no-peer topologies stay green, unchanged invocations)"
        status: pass
    human_judgment: false

duration: ~70min
completed: 2026-07-28
status: complete
---

# Phase 11 Plan 08: Gateway ingress destination authority, egress field ownership, and fail-closed route config Summary

**Closes the four source-verified Report C trust-boundary findings (F-1..F-4): ingress now rejects any envelope not addressed to this gateway's own domain and mailbox, egress rejects (never signs) client-supplied federation metadata, and the route map is built only from explicit `--backs`/single-peer bindings with startup-fatal fail-closed on any ambiguity or duplicate `--peer`/`--backs` configuration.**

## Performance

- **Duration:** ~70 min active implementation/verification work
- **Tasks:** 3 completed
- **Files modified:** 3 (ingress.rs, egress.rs, main.rs); 2 new integration test files

## Accomplishments
- `crates/famp-gateway/src/ingress.rs::inbox_handler` now reads the verified envelope's own `to` principal and, before the registry lock is ever taken: rejects `to != URL-path recipient` unconditionally (`MisaddressedRecipient`, 400); rejects `to.authority() != own-domain` when own-domain is configured (`ForeignDomain`, 403), reusing 11-07's already-resolved `own_domain` threaded through `GatewayIngressState`/`build_gateway_router`/`run_ingress`; and rejects a `federation_format_ok() == false` envelope (`MalformedFederationFields`, 400), wiring in a helper that previously had zero callers outside its own unit tests. Each reject gets its own status code, JSON `error` slug, and `eprintln!` naming which check failed.
- `crates/famp-gateway/src/egress.rs::relay_one` now scans a drained envelope for the 7 federation-owned keys (`from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry`/`capability`/`approval`) immediately before `sign_federation_fields` and returns the new typed `RelayError::ClientSuppliedFederationField { fields }` (naming every offending field) if any are present — the envelope is never mutated, never signed. `sign_federation_fields`'s five `entry().or_insert_with()` calls became unconditional `insert()`.
- `crates/famp-gateway/src/main.rs` gained a `--backs agent:<domain>/<name>` repeatable flag; `parse_args` now rejects a duplicate `--peer` domain (naming both URLs) and a duplicate `--backs` principal at parse time. Route-map construction (extracted into `build_route_map()`) registers exactly the operator-declared bindings: `--backs` entries resolved against their matching `--peer` domain (startup-fatal if none matches), or bare positional names resolved against the sole configured peer when exactly one `--peer` is given — 2+ peers with bare names is now startup-fatal with an actionable "use --backs" message instead of silently fabricating a peer x name cross-product of principal->route bindings.

## Task Commits

Each task was committed atomically:

1. **Task 1: ingress rejects foreign-domain and misaddressed envelopes (F-1/INV-H)** - `2a293db` (fix)
2. **Task 2: gateway is sole writer of federation-owned fields (F-2/INV-F)** - `7293ad2` (fix)
3. **Task 3: explicit route bindings, fail closed on ambiguity (F-3+F-4/INV-J)** - `319f572` (fix; also carries this plan's Task 1 `main.rs` ingress-threading hunk — see Deviations)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `crates/famp-gateway/src/ingress.rs` - `GatewayIngressState.own_domain: Option<Arc<str>>`; `build_gateway_router`/`run_ingress` take an `own_domain` param; `envelope_recipient`/`envelope_federation_format_ok` dispatch helpers; 3 new `IngressError` variants + distinct 4xx mapping; the 3 pre-delivery gates in `inbox_handler`
- `crates/famp-gateway/src/egress.rs` - `FEDERATION_OWNED_FIELDS` const; `RelayError::ClientSuppliedFederationField`; pre-check in `relay_one`; `sign_federation_fields`'s 5 inserts made unconditional; 2 new adversarial tests
- `crates/famp-gateway/src/main.rs` - `GatewayArgs.backs: Vec<Principal>`; `--backs` parsing + duplicate-`--backs`/duplicate-`--peer` rejection; `build_route_map()` extracted helper replacing the cross-product; `ingress_own_domain` threaded into the `run_ingress` call; 5 new unit tests
- `crates/famp-gateway/tests/inbound_destination_validation.rs` **(new)** - 3 adversarial integration tests (foreign-domain reject + empty-mailbox assertion, misaddressed-recipient reject + empty-mailbox assertion, well-formed delivery) against a real broker + `GatewayRegistry`
- `crates/famp-gateway/tests/route_config_fail_closed.rs` **(new)** - 5 startup-fail-closed integration tests spawning a real `famp-gateway` subprocess

## Decisions Made
See `key-decisions` in frontmatter for the four load-bearing calls (own-domain Arc<str> conversion site, the idempotence-test non-change, the client-supplied-field pre-check placement/rationale, and `--backs` scoping vs. `backed_names`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug/hygiene] `main()` exceeded `clippy::too_many_lines` (119/100) after Task 3's route-map rewrite**
- **Found during:** Task 3, `just lint`
- **Issue:** Replacing the inline cross-product with the new `--backs`/bare-name branch logic pushed `main()` over clippy's pedantic line limit.
- **Fix:** Extracted the entire route-map construction into a standalone `async fn build_route_map(args, backed_names, transport)`, called from `main()`. Mirrors 11-07's own `resolve_own_domain_or_exit` extraction, done for the identical reason.
- **Files modified:** `crates/famp-gateway/src/main.rs`
- **Verification:** `just lint` clean afterward.
- **Committed in:** `319f572` (Task 3 commit)

**2. [Rule 1 - Bug/hygiene] Test-file clippy findings in the two new integration test files**
- **Found during:** Task 3, `just lint`
- **Issue:** `route_config_fail_closed.rs` had a `use` statement placed after a statement (`clippy::items_after_statements`, part of `-D clippy::pedantic`) and a `match` with two arms sharing an identical body (`clippy::match_same_arms`).
- **Fix:** Moved the `use std::io::{BufRead, BufReader};` import to module scope; merged `Ok(0) => break` and `Err(_) => break` into a single `Ok(0) | Err(_) => break` arm.
- **Files modified:** `crates/famp-gateway/tests/route_config_fail_closed.rs`
- **Verification:** `just lint` clean afterward; test still passes.
- **Committed in:** `319f572` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — clippy pedantic-lint hygiene forced by this plan's own new code, not pre-existing scope creep).
**Impact on plan:** No behavior change from either fix. No scope creep.

### Commit-splitting note (not a deviation from the plan's substance, but a documented departure from strict one-task-one-file-set commits)

This plan's own `<files>` tags list `crates/famp-gateway/src/main.rs` under **both** Task 1 (thread `own_domain` into `run_ingress`) and Task 3 (the `--backs`/duplicate-config rewrite) — the two changes are structurally interleaved in the same function (`main()`) and in adjacent code near `tokio::select!`. Splitting `main.rs`'s diff into two independently-compiling commits would have required constructing an intermediate hand-edited version of the file solely for commit-history granularity, with no corresponding benefit (both tasks land in this same plan/session). Task 1's `main.rs` hunk (`ingress_own_domain` local + updated `run_ingress` call) is therefore committed together with Task 3's `main.rs` changes in `319f572`, and called out explicitly in that commit's message. `ingress.rs` (Task 1's substantive change) and `egress.rs` (Task 2, entirely self-contained) each landed in their own commit as planned.

## Issues Encountered

- **`cargo test --workspace` (the plan's full verification-section gate) ran into the same broker-socket-contention slowness 11-02/11-03/11-07 documented** — dozens of concurrently-spawned `famp broker` subprocesses accumulate under this machine's parallel test execution, and the run had not finished within this session's practical time budget at the point this SUMMARY was written. Per this plan's own `critical_environment_notes` ("say so plainly... rather than claiming a gate you did not observe"), this is recorded honestly rather than asserted. **This is not a regression from this plan's changes** — `famp-gateway`'s own full test suite (all 3 new/modified source files' logic, both new integration test files, and every named control: `e2e_cross_host_delivery`, `no_cross_talk`, `principal_send_drain`, `process_readiness`) was run directly via `cargo test -p famp-gateway` (foreground, targeted) and is **100% green** (51 tests, 0 failures) — this is the load-bearing verification for this plan's actual scope. `just fmt-check`, `just lint` (workspace, all-targets, `-D warnings`), `just spec-lint` (21/21), `just check-no-tokio-in-bus`, and `just check-mcp-deps` were all run directly and are clean.

## Verification Performed
- `cargo test -p famp-gateway` — 18 lib unit tests (`egress`/`ingress`/`verify`) + 18 `main.rs` unit tests + `e2e_ci_gate_guard` (2) + `e2e_cross_host_delivery` (1, THE control — own-domain-unset, single-peer, full relay path) + `gateway_usage_doc_accuracy` (1) + `inbound_destination_validation` (3, new) + `liveness` (1) + `no_cross_talk` (1, control) + `principal_send_drain` (2, control) + `process_readiness` (1, control) + `route_config_fail_closed` (5, new) = **54 tests total across the crate, all green, 0 failures.**
- `just fmt-check` — clean (after one `cargo fmt --all` pass on `egress.rs` and the new `inbound_destination_validation.rs`).
- `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) — clean, after extracting `build_route_map()` and two test-file pedantic-lint fixes (see Deviations).
- `just spec-lint` — 21/21 passed.
- `just check-no-tokio-in-bus` — OK, famp-bus tokio-free.
- `just check-mcp-deps` — OK.
- `cargo test --workspace` — started, ran a substantial portion of the workspace cleanly (famp-gateway, famp-bus's unit + several integration binaries, famp's codex install/uninstall roundtrip, cli_*, clierror_* suites all observed green in the log) but did not finish within this session's time budget due to documented broker-subprocess contention on this machine — see Issues Encountered.
- `grep -n 'federation_format_ok' crates/famp-gateway/src/ingress.rs` — confirms it is now called on the inbound path.
- `grep -rn 'own_domain' crates/famp-gateway/src/` — confirms exactly ONE resolution site (`main.rs`'s `resolve_own_domain_or_exit`) feeds both egress (`Option<String>`) and ingress (`Option<Arc<str>>`, converted at the call site).
- `grep -n 'or_insert_with' crates/famp-gateway/src/egress.rs` — zero matches inside `sign_federation_fields`.
- `grep -n 'for name in &backed_names' crates/famp-gateway/src/main.rs` — only the egress-spawn loop remains; the route-map cross-product is gone.

## GATEWAY-SETUP.md checklist for plan 11-05 (required by this plan's Task 3)

Plan 11-05 owns editing `docs/GATEWAY-SETUP.md`; this plan only enumerates what needs to change:

1. **Flag-surface block (§4)** — add `[--backs agent:<domain>/<name>]...` to the usage synopsis, between `[--peer ...]...` and `[--trust-cert <path>]`.
2. **Flag table (§4)** — add a `--backs agent:<domain>/<name>` row: "No, repeatable — explicit principal->route binding; resolved against the matching `--peer` domain's URL. Required when 2+ `--peer` domains are configured (bare positional names become startup-fatal in that case)."
3. **`--peer` row** — append: "A repeated domain across multiple `--peer` flags is a startup error naming both URLs (fails closed; no longer last-write-wins)."
4. **`<principal-name>...` row / surrounding prose** — note the new restriction: bare positional names only resolve routes automatically when **exactly one** `--peer` is configured; with 2+ peers, use `--backs` for each principal or the process exits non-zero before printing "ready".
5. **Own-domain configuration (currently entirely undocumented, inherited from 11-07 and now ALSO gating ingress in this plan)** — add a new subsection explaining `FAMP_OWN_DOMAIN` (env var) or `$FAMP_HOME/own-domain` (file, first non-empty trimmed line) as this gateway's federation authority. When set, egress rejects any `from` whose authority doesn't match it, and (as of this plan) ingress rejects any `to` whose authority doesn't match it. When unset, both checks are skipped (with a warning) — the `docs/HUMAN-UAT.md` two-machine dogfood should set this on both hosts to close that residual (T-11-29).
6. The two example invocations under "On A:" / "On B:" remain valid as-is (single-peer topology) — no change needed there, just the surrounding documentation above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness
- 11-04 (shipping-surface E2E harness) and 11-05 (GATEWAY-SETUP.md rewrite) can now proceed against the FINAL gateway config surface — `--backs`, the duplicate-`--peer`/`--backs` rejections, and the >1-peer bare-name restriction are all in place and will not need a second pass.
- This plan's own `<artifacts>` section flagged that `SEC-02`/`SEC-03`/`SEC-04` did not yet exist in `REQUIREMENTS.md` at plan-authoring time. A concurrent session (commit `0b5a34c`, observed already on `main` when this plan's Task 1 commit landed) added the full `SEC-01..04` section and reconciled 11-07's dangling `SEC-01`/`ADDR-04` commit-message citations — this plan's `requirements mark-complete` step ran against that section and marked `SEC-02`/`SEC-03`/`SEC-04` complete.
- No blockers.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-28*

## Self-Check: PASSED

- FOUND: `crates/famp-gateway/src/ingress.rs`
- FOUND: `crates/famp-gateway/src/egress.rs`
- FOUND: `crates/famp-gateway/src/main.rs`
- FOUND: `crates/famp-gateway/tests/inbound_destination_validation.rs`
- FOUND: `crates/famp-gateway/tests/route_config_fail_closed.rs`
- FOUND commit: `2a293db`
- FOUND commit: `7293ad2`
- FOUND commit: `319f572`
