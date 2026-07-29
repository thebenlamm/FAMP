---
phase: 11-shipping-client-remote-addressing-setup-hardening
verified: 2026-07-29T03:30:00Z
status: passed
score: 7/7 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 11: Shipping-Client Remote Addressing + Setup Hardening Verification Report

**Phase Goal:** A real shipping client (`famp send` / `famp_send`), not a hand-written injector, can address a remote principal and drive a signed cross-host delivery to a terminal task state; the two-machine setup guide is correct and followable on both macOS and Linux; and the 8 Gate A dogfood defects are fixed with regression coverage.

**Verified:** 2026-07-29T03:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria, 7 requirements)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp send --to agent:<domain>/<name>` emits domain-qualified `from`/`to`, bus target stays bare leaf, delivers into remote mailbox (ADDR-01) | ✓ VERIFIED | Source: `crates/famp/src/cli/send/mod.rs` `build_envelope_value`/`build_remote_envelope_value` splits `Target::Agent{name: leaf}` from envelope `to=principal.to_string()`. Independently re-run: `cargo test -p famp send` — all remote-target/split-addressing/local-regression tests pass. Live: `11-HUMAN-UAT.md` — delivered into `bob`'s real mailbox on a physically separate machine. |
| 2 | Remote sends emit typed, FSM-driving unsigned envelope (sign-then-strip); bare-name stays `audit_log` (ADDR-02) | ✓ VERIFIED | `build_remote_envelope_value` mode-branches `--new-task`→`RequestBody`, `--task`→`CommitBody`, `--task --terminal`→`DeliverBody`+`terminal_status`, all sign-then-strip (no `signature` key). Independently re-run: `remote_new_task_emits_typed_request_no_signature`, `remote_task_non_terminal_emits_typed_commit_with_commits_causality`, `remote_task_terminal_emits_typed_deliver_with_terminal_status`, `local_bare_name_send_unchanged_by_mode_branching` all pass (`cargo test -p famp send`). |
| 3 | A defined single-source own-domain resolver exists for stamping `from` (ADDR-03) | ✓ VERIFIED | `crates/famp/src/cli/own_domain.rs::resolve_own_domain` — one env-read site (`grep -rn FAMP_OWN_DOMAIN crates/famp/src` returns only `own_domain.rs`), precedence `--domain` > env > file, validated via probe-`Principal` parse. Independently re-run: `cargo test -p famp own_domain` — `resolves_precedence_trims_and_rejects`, `remote_send_with_no_own_domain_source_returns_typed_error`, `resolve_export_principal_couples_to_own_domain` all pass. |
| 4 | Transport egress logs the full error source chain, not opaque `"reqwest failure"` (OBS-01) | ✓ VERIFIED | `crates/famp-transport-http/src/error.rs`: `#[error("reqwest failure: {0}")]` interpolates the source. `RelayError::Transport(#[from] HttpTransportError)` in `egress.rs` preserves the chain (no `.to_string()` flatten). Independently re-run: `cargo test -p famp-transport-http --lib error` (`reqwest_failed_display_contains_source_text`, `invalid_url_display_contains_source_text`) and `cargo test -p famp-gateway --lib` (egress OBS-01 test in the 18-test green run) both pass. |
| 5 | `docs/GATEWAY-SETUP.md` corrected for all 8 Gate A findings (DOC-05) | ✓ VERIFIED | Doc contains `CA:FALSE`/`serverAuth` cert recipe, `socketfilterfw` firewall pre-auth, own-domain config surface (`FAMP_OWN_DOMAIN`), §3 pins under `agent:{domain}/{name}` (sender agent principal, no `/gateway` label), §4 states each host backs the REMOTE principal, ready-after-keyring-load. Independently re-run: `cargo test -p famp --test gateway_setup_doc_accuracy` — pass (includes the extended semantic/directional/ordering assertions from plan 05 task 2). |
| 6 | Cross-platform fixtures regenerated, macOS CI leg exercised, shipping-surface e2e replaces injector, negative test yields typed error (TEST-03) | ✓ VERIFIED | `crates/famp/tests/fixtures/cross_machine/{alice,bob}.crt` verified CA:FALSE+serverAuth (openssl inspection in 11-04-SUMMARY, not independently re-run but corroborated by the fact `e2e_cross_host_delivery` — the fixture consumer — passes locally on macOS). `crates/famp-gateway/tests/e2e_shipping_surface.rs` drives the real `famp send` (not `send_bus_envelope`) for happy-path + full-cycle terminal + observable negative. Independently re-run: `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery` — both green (`shipping_send_happy_path_full_cycle_and_observable_negative` and `gw01_gw02_gw03_two_process_cross_host_delivery`). |
| 7 | Gate A two-machine dogfood re-run with fixed `famp send` (no injector), terminal FSM on both sides — final human gate (UAT-01) | ✓ VERIFIED | `11-HUMAN-UAT.md`, verdict PASS, `status: passed`, `verdict: PASS` in frontmatter. Task `019fab97-d3e0-7d63-92ba-39f1ce171b83` reached COMPLETED on both `bens-macbook-air` and `home-devbox` over Tailscale, driven entirely by the shipping `famp send` CLI (no injector), `sig_verified: true` on the federated path, bidirectional delivery confirmed. This is a `checkpoint:human-verify` gate (`autonomous: false`) and was run live per the phase context — the orchestrator independently corroborated it (per the task's `already_established_evidence`). |

**Score:** 7/7 truths verified (0 present-but-behavior-unverified)

### Trust-boundary hardening that "rides with" the phase (SEC-01..04, not in ROADMAP's official 7 but part of the phase's stated scope and self-added to REQUIREMENTS.md mid-phase)

These four items were added to `REQUIREMENTS.md` during the phase (commit `0b5a34c`, from a third external design review) and are NOT in ROADMAP.md's Phase 11 "Requirements:" line (which lists only the 7 above) or in the orchestrator's "Phase requirement IDs" list for this verification. I verified them anyway since the phase objective text explicitly includes "the trust-boundary hardening... that ride with it."

| Item | Status | Evidence |
|------|--------|----------|
| SEC-01 — broker binds envelope `from` to authenticated identity; egress rejects foreign-domain `from` when own-domain configured | ✓ VERIFIED | `crates/famp-bus/src/broker/handle.rs::send` — `is_self_authored(envelope, Some(&effective_identity))` gate before any mailbox write, covers both agent + channel sends. `egress.rs::relay_one` `FromDomainMismatch` check before `sign_federation_fields`. Independently re-run: `cargo test -p famp-bus` (green) + `cargo test -p famp-gateway --lib` `relay_one_rejects_foreign_from_domain_when_own_domain_configured` (green). |
| SEC-02 — ingress authoritative only for own domain + addressed mailbox; `federation_format_ok` wired in | ✓ VERIFIED | `crates/famp-gateway/src/ingress.rs::inbox_handler` — `MisaddressedRecipient` (to != URL-path recipient, unconditional) and `ForeignDomain` (to.authority() != own-domain, when configured) checks before registry lookup; `envelope_federation_format_ok()` called. Independently re-run: `cargo test -p famp-gateway --test inbound_destination_validation` — 3/3 pass, asserting mailbox untouched on reject (not just status code). |
| SEC-03 — gateway sole writer of 7 federation-owned fields | ✓ VERIFIED | `egress.rs` `FEDERATION_OWNED_FIELDS` const (7 fields) scanned before signing; `sign_federation_fields`'s 5 derived-field inserts are unconditional (`grep -n or_insert_with` returns nothing inside that function). Independently re-run: `relay_one_rejects_each_client_supplied_federation_field`, `relay_one_names_all_present_client_supplied_fields_at_once` — pass. |
| SEC-04 — route config explicit, fails closed on ambiguity | ✓ VERIFIED | `main.rs` `--backs agent:<domain>/<name>` flag; cross-product loop deleted (only the unrelated egress-spawn loop over `backed_names` remains); duplicate `--peer`/`--backs` and bare-names-with-2+-peers all reject at parse/startup. Independently re-run: `cargo test -p famp-gateway --test route_config_fail_closed` — 5/5 pass. |

All four independently confirmed at source + test level. `cargo test -p famp-gateway --lib` (18 tests, includes all egress/ingress/verify adversarial cases) and the two named control e2e tests (`e2e_cross_host_delivery`, `e2e_shipping_surface`) all green in this session's own re-run, not merely SUMMARY-claimed.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-transport-http/src/error.rs` | Display includes source | ✓ VERIFIED | `#[error("reqwest failure: {0}")]`, test passes |
| `crates/famp-gateway/src/egress.rs` | `RelayError::Transport(#[from] HttpTransportError)` + own-domain + field-ownership checks | ✓ VERIFIED | All three checks present, tests pass |
| `crates/famp/src/cli/own_domain.rs` | New resolver module, single env-read site | ✓ VERIFIED | Exists, tests pass, sole `FAMP_OWN_DOMAIN` reader |
| `crates/famp/src/cli/send/mod.rs` | `--domain` flag, conditional remote path, mode-branched class | ✓ VERIFIED | All present, tests pass |
| `crates/famp-bus/src/broker/handle.rs` | `from.name()==effective_identity` gate | ✓ VERIFIED | Present before mailbox insertion, tests pass |
| `crates/famp-gateway/src/ingress.rs` | to-authority + to==path-recipient + `federation_format_ok` gates | ✓ VERIFIED | All present, tests pass |
| `crates/famp-gateway/src/main.rs` | `--backs`, cross-product removed, dup-config rejection, ready-after-init | ✓ VERIFIED | Confirmed at L305-387 (ready line after home/own-domain/signing-key/keyring/transport/route-map) |
| `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` | CA:FALSE+serverAuth regen | ✓ VERIFIED | Consumer test (`e2e_cross_host_delivery`) green on this macOS host |
| `crates/famp-gateway/tests/e2e_shipping_surface.rs` | Happy + full-cycle terminal + negative | ✓ VERIFIED | Exists, single named test passes |
| `crates/famp-gateway/tests/common/gateway_harness.rs` | Extracted shared harness | ✓ VERIFIED | Present; `e2e_cross_host_delivery` (control) still green after extraction |
| `docs/GATEWAY-SETUP.md` | Corrected for 8 findings | ✓ VERIFIED | Content confirmed + accuracy gate passes |
| `crates/famp/tests/gateway_setup_doc_accuracy.rs` | Extended semantic assertions | ✓ VERIFIED | Test passes |
| `crates/famp-gateway/tests/inbound_destination_validation.rs`, `route_config_fail_closed.rs` | New SEC-02/04 integration tests | ✓ VERIFIED | Both files exist, all 8 tests pass |
| `.planning/phases/11-.../11-HUMAN-UAT.md` | Dogfood record | ✓ VERIFIED | PASS verdict recorded |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `own_domain::resolve_own_domain` | `famp send` `from` authority | `build_remote_envelope_value` calls `resolve_own_domain(args.domain.as_deref(), home)` | ✓ WIRED |
| `own_domain::resolve_own_domain` | `famp peer export` label authority | `export.rs` calls `own_domain::resolve_own_domain(None, home)` | ✓ WIRED |
| broker `effective_identity` | envelope `from.name()` | `handle.rs::send` `is_self_authored` gate | ✓ WIRED |
| gateway `own_domain` (main.rs single resolution) | egress `from.authority()` check | `run_egress`→`relay_one` | ✓ WIRED |
| gateway `own_domain` (same value) | ingress `to.authority()` check | `run_ingress`→`GatewayIngressState.own_domain` | ✓ WIRED (confirmed same resolution site feeds both, per `grep -rn own_domain crates/famp-gateway/src/`) |
| fixed `famp send` remote path | gateway egress → HTTPS → ingress → remote mailbox → terminal FSM | `e2e_shipping_surface.rs` + live `11-HUMAN-UAT.md` | ✓ WIRED, both source-test and live-dogfood confirmed |

### Behavioral Spot-Checks / Independently Re-Run Tests (this session, not SUMMARY-sourced)

| Test | Command | Result |
|------|---------|--------|
| own-domain precedence + errors | `cargo test -p famp own_domain` | ✓ PASS (3 tests) |
| famp-bus adversarial from-binding | `cargo test -p famp-bus` | ✓ PASS |
| e2e control + shipping surface | `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery` | ✓ PASS (2/2) |
| gateway_setup_doc_accuracy | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✓ PASS |
| transport-http error chain | `cargo test -p famp-transport-http --lib error` | ✓ PASS (2 named tests) |
| gateway lib (egress/ingress/verify adversarial) | `cargo test -p famp-gateway --lib` | ✓ PASS (18/18) |
| SEC-02 ingress destination validation | `cargo test -p famp-gateway --test inbound_destination_validation` | ✓ PASS (3/3) |
| SEC-04 route config fail-closed | `cargo test -p famp-gateway --test route_config_fail_closed` | ✓ PASS (5/5) |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` | ✓ PASS (clean) |

(Full-workspace `cargo test --workspace` / `just ci` were NOT run — per the environment notes, these hang/timeout on this machine. Targeted crate/test-binary runs above are the load-bearing evidence, matching the phase's own documented environment workaround.)

### Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
|-------------|-------------|--------|----------|
| ADDR-01 | 11-03 | ✓ SATISFIED | See truth #1 |
| ADDR-02 | 11-03 | ✓ SATISFIED | See truth #2 |
| ADDR-03 | 11-02 | ✓ SATISFIED | See truth #3 |
| OBS-01 | 11-01 | ✓ SATISFIED | See truth #4 |
| DOC-05 | 11-05 | ✓ SATISFIED | See truth #5 |
| TEST-03 | 11-04 | ✓ SATISFIED | See truth #6 |
| UAT-01 | 11-06 | ✓ SATISFIED | See truth #7 |
| SEC-01..04 | 11-07, 11-08 | ✓ SATISFIED (out-of-roadmap addition, verified anyway) | See trust-boundary table |

No orphaned requirements: all 7 official Phase 11 requirement IDs (ADDR-01/02/03, OBS-01, DOC-05, TEST-03, UAT-01) appear in exactly one plan's frontmatter `requirements:` field, matching ROADMAP.md's Phase 11 "Requirements:" line and REQUIREMENTS.md.

### Anti-Patterns Found

None (blocker-level). No `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER` markers in any Phase-11-modified file (`error.rs`, `egress.rs`, `ingress.rs`, `main.rs`, `own_domain.rs`, `send/mod.rs`, `handle.rs`, `GATEWAY-SETUP.md`).

**Documentation-only WARNING (not code, not phase-blocking):** `.planning/REQUIREMENTS.md`'s Traceability table (bottom of file) still shows `SEC-02 | Phase 11 | In Progress`, `SEC-03 | Phase 11 | In Progress`, `SEC-04 | Phase 11 | In Progress` while the checklist items directly above (`- [x] SEC-02/03/04...`) are checked complete. Source-level and test evidence in this report confirms SEC-02/03/04 ARE functionally complete — this is a stale traceability-table row, not an unfinished feature. Recommend updating those three rows to `Complete` in a follow-up doc commit. Separately, plan 11-07's commit message referenced an undefined `ADDR-04` (per plan 11-08's own note) which was never created and remains an unresolved dangling reference — low-impact, informational only.

### Human Verification Required

None. UAT-01 (the phase's one `checkpoint:human-verify` gate) was already run and recorded PASS in `11-HUMAN-UAT.md`, independently corroborated per the task's established evidence, and its live artifacts (task IDs, mailbox listings, `sig_verified: true`) are consistent with the source-level implementation verified above.

### Gaps Summary

No gaps found against the 7 official Phase 11 requirements or the additional SEC-01..04 trust-boundary hardening. All must-have truths, artifacts, and key links verified at the source level with independently re-run tests (not merely trusted from SUMMARY.md claims), plus a live two-machine UAT PASS record. The only finding is a cosmetic staleness in REQUIREMENTS.md's traceability table (SEC-02/03/04 rows say "In Progress" despite the functionally-complete checklist items and passing tests) — flagged as a WARNING for a follow-up doc fix, not blocking phase completion.

---

*Verified: 2026-07-29T03:30:00Z*
*Verifier: Claude (gsd-verifier)*
