---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 03
subsystem: cli
tags: [send, addressing, principal, fsm, request, commit, deliver, sign-then-strip, justfile]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 02's own_domain::resolve_own_domain(cli_domain, home) single-source resolver"
provides:
  - "famp send --to agent:<domain>/<name> remote branch: domain-qualified from/to, leaf-split bus Target routing"
  - "mode-branched typed unsigned envelope class on the remote branch (request/commit/deliver-terminal) via sign-then-strip"
  - "--domain CLI flag on SendArgs (CLI + slash + famp_send MCP chokepoint)"
  - "just install-gateway / just install-all Justfile recipes"
affects: [11-04, 11-05, 11-06, 11-07 (UAT-01 terminal-FSM-reachable-via-shipping-surface now provable)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "sign-then-strip (throwaway FampSigningKey::from_bytes([42u8;32]), sign -> encode -> strip `signature`) is the sanctioned route to unsigned typed-class bus Values — mirrors famp-gateway/src/egress.rs's plain_request_value and the e2e test's unsigned_value helper"
    - "split-addressing: bus Target::Agent routes by Principal::name() (leaf) while the envelope to/from carry the full domain-qualified Principal — two independent representations of the same recipient, never conflated"
    - "build_envelope_value dispatches on Option<&Principal> (remote vs local) rather than mutating a shared code path — keeps the local audit_log shape byte-unchanged and testable in isolation"

key-files:
  created: []
  modified:
    - crates/famp/src/cli/send/mod.rs
    - crates/famp/src/cli/mcp/tools/send.rs
    - Justfile

key-decisions:
  - "Own-domain resolution threaded as an explicit home: &Path parameter into build_envelope_value/build_remote_envelope_value (never read from env inside), mirroring the CD-05 convention from plan 02's own_domain.rs; home is only resolved via crate::cli::home::resolve_famp_home() on the remote branch in run_at_structured — local (bare-name) sends never require FAMP_HOME/HOME (D-04 preserved)."
  - "The malformed-agent: target guard fires in run_at_structured BEFORE the target/envelope build (not inside build_envelope_value), so it can return without a live broker connection and covers both the CLI and famp_send MCP paths through the single chokepoint."
  - "Reused CliError::SendArgsInvalid for the malformed-target reject and the missing-mode-on-remote-send reject rather than adding new CliError variants — keeps the exhaustive mcp_error_kind table untouched (no new arms needed), consistent with plan 02's PeerBlobMalformed-reuse precedent."
  - "Skipped the plan's optional '--to bob --domain X sugar' composition (explicitly 'MAY... Claude's Discretion' in the plan) — the full-principal --to is the only remote-trigger, keeping scope to what the acceptance criteria require."

patterns-established:
  - "fresh_ts()/fresh_id_and_ts() extracted as shared helpers so both the local (string id) and remote (typed MessageId) envelope paths generate RFC 3339 timestamps identically without duplicating the subsecond-stripping logic."

requirements-completed: [ADDR-01, ADDR-02]

coverage:
  - id: D1
    description: "famp send --to agent:<domain>/<name> emits an envelope whose to is the full principal verbatim and whose from is agent:{own-domain}/{identity}"
    requirement: "ADDR-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/send/mod.rs#build_envelope_value_remote_qualifies_from_and_to"
        status: pass
    human_judgment: false
  - id: D2
    description: "Bus routing Target.name is the bare leaf while the envelope to carries the full principal (split-addressing)"
    requirement: "ADDR-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/send/mod.rs#remote_target_splits_bus_leaf_from_envelope_principal"
        status: pass
      - kind: static
        ref: "grep -n 'Target::Agent' crates/famp/src/cli/send/mod.rs (single construction site, leaf-derived)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Remote class branches on send mode (new_task->request, task->commit, task+terminal->deliver+terminal_status) so the FSM can reach a terminal state; every shape unsigned via sign-then-strip"
    requirement: "ADDR-02"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/send/mod.rs#remote_new_task_emits_typed_request_no_signature, #remote_task_non_terminal_emits_typed_commit_with_commits_causality, #remote_task_terminal_emits_typed_deliver_with_terminal_status"
        status: pass
      - kind: e2e
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (gw01_gw02_gw03_two_process_cross_host_delivery)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Bare-name --to bob local send is unchanged modulo id/ts (audit_log class, agent:local.bus/... from/to)"
    requirement: "ADDR-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/send/mod.rs#build_envelope_value_local_path_unchanged_with_domain_unset, #local_branch_still_audit_log_after_typed_class_branching"
        status: pass
    human_judgment: false
  - id: D5
    description: "Missing own-domain on a remote send returns typed OwnDomainNotSet; malformed agent: target returns typed reject with no local fallback"
    requirement: "ADDR-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/send/mod.rs#remote_send_with_no_own_domain_source_returns_typed_error, #malformed_agent_prefixed_target_is_rejected_typed_no_local_fallback"
        status: pass
    human_judgment: false
  - id: D6
    description: "just install-gateway / just install-all recipes exist and deploy both shipping binaries with fresher mtimes than the source change"
    verification:
      - kind: other
        ref: "just install-all; stat ~/.cargo/bin/famp ~/.cargo/bin/famp-gateway vs crates/famp/src/cli/send/mod.rs mtimes"
        status: pass
    human_judgment: false

duration: ~55min
completed: 2026-07-28
status: complete
---

# Phase 11 Plan 03: Remote addressing core fix — split-addressing + mode-branched typed class Summary

**`famp send --to agent:<domain>/<name>` now emits the exact domain-qualified, typed-unsigned envelope shape the gateway already relays — full principal `to`/`from`, leaf-routed bus `Target`, and a send-mode-branched class (`request`/`commit`/`deliver`+`terminal_status`) so the FSM can actually reach a terminal state through the shipping CLI/MCP surface — while `--to bob` (bare name) stays byte-identical to today.**

## Performance

- **Duration:** ~55 min (includes a long `cargo test --workspace` full-suite confirmation run — see Verification)
- **Started:** 2026-07-28T19:10Z (approx, first Read)
- **Completed:** 2026-07-28T20:06Z
- **Tasks:** 3 completed
- **Files modified:** 3 (`crates/famp/src/cli/send/mod.rs`, `crates/famp/src/cli/mcp/tools/send.rs`, `Justfile`)

## Accomplishments
- `SendArgs` gained a `--domain` flag (highest-precedence source in `own_domain::resolve_own_domain`); the MCP `SendArgs` struct-literal constructor (`mcp/tools/send.rs`) updated to compile.
- `run_at_structured` parses `--to` as a `Principal` up front: a full `agent:<domain>/<name>` takes the remote branch (bus `Target::Agent` uses the LEAF name only — split-addressing, Pitfall 2/T-11-09); a malformed `agent:`-prefixed string is rejected typed (`CliError::SendArgsInvalid`) *before* any bus connection, never falling through to a local `agent:local.bus/agent:garbage` shape (review LOW); a bare name takes the unchanged local path (D-04) and never resolves `FAMP_HOME`.
- `build_envelope_value` dispatches to a new `build_remote_envelope_value` for the remote branch: `from = agent:{own_domain}/{identity}` (own-domain resolved via plan 02's `resolve_own_domain(args.domain.as_deref(), home)`), `to` = the full principal verbatim.
- The remote branch's envelope CLASS branches on send mode (review HIGH #1 — a bare `request` never advances the FSM past REQUESTED per `famp-fsm::engine.rs:29-40`): `--new-task` → typed `RequestBody` (class `request`); `--task` (non-terminal) → typed `CommitBody` (class `commit`) with `Causality{rel:Commits}`; `--task --terminal` → typed `DeliverBody` (class `deliver`) with `Causality{rel:Delivers}` + `terminal_status: Completed`. Every shape is produced via the sanctioned sign-then-strip pattern (throwaway key, sign → encode → strip `signature`) — BUS-11 holds, no signature ever reaches the bus.
- `Justfile` gained `install-gateway` (mirrors `install`, targets `crates/famp-gateway`) and `install-all` (runs both). Ran `just install-all`; both `~/.cargo/bin/famp` and `~/.cargo/bin/famp-gateway` are now newer than the source change, so the `famp_send` MCP path and any live agent session carry this fix.

## Task Commits

Each task was committed atomically:

1. **Task 1: --domain flag + remote-target parse + split-addressing (D-01/D-02)** - `380ec4a` (feat)
2. **Task 2: mode-branched typed class via sign-then-strip (D-03/D-04, review HIGH #1)** - `67ad906` (feat)
3. **Task 3: install-gateway/install-all recipes + deploy fixed binaries** - `cd31742` (chore)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `crates/famp/src/cli/send/mod.rs` - `--domain` flag; `run_at_structured` parses `--to` as `Principal` up front and splits bus-Target leaf from envelope principal; `build_envelope_value`/`build_remote_envelope_value` dispatch on remote vs local; mode-branched typed class construction with `sign_then_strip`/`two_key_bounds`/`parse_task_id` helpers; 8 new unit tests
- `crates/famp/src/cli/mcp/tools/send.rs` - `SendArgs` struct-literal constructor gains `domain: None` (compile touchpoint; MCP `famp_send` has no `--domain` equivalent input field)
- `Justfile` - `install-gateway` + `install-all` recipes

## Decisions Made
- Own-domain resolution threaded as an explicit `home: &Path` param (never env-read inside `send/mod.rs`) — mirrors plan 02's CD-05 convention; `home::resolve_famp_home()` is called only on the remote branch in `run_at_structured`, so local sends never require `FAMP_HOME`/`HOME` (D-04 preserved).
- The malformed-`agent:`-target guard lives in `run_at_structured` before target/envelope construction (not inside `build_envelope_value`), so the test asserting it needs no live broker and both the CLI and `famp_send` MCP paths get the same reject through the single chokepoint.
- Reused `CliError::SendArgsInvalid` for both the malformed-target reject and the "no mode on remote send" reject rather than adding new `CliError` variants — avoids touching the exhaustive `mcp_error_kind` table, consistent with plan 02's `PeerBlobMalformed`-reuse precedent for the mismatch case.
- Skipped the plan's optional `--to bob --domain X` sugar composition (explicitly "MAY... Claude's Discretion" in the plan text) to keep the change scoped to what the acceptance criteria require; the full-principal `--to` remains the only remote trigger.

## Deviations from Plan

None — all three tasks match the plan's `<action>` text. The one plan-flagged optional feature (`--to bob --domain X` sugar) was explicitly left to discretion and was not implemented; this is a scope decision, not a deviation from a required behavior.

## Issues Encountered
- **Duplicate `cargo test --workspace` invocation raced on the cargo target lock** during verification (a leftover background process from an earlier attempt plus a fresh one) — same failure mode 11-02's SUMMARY documented ("Two `just ci` invocations raced on the same cargo target lock"). Killed the stale process; the remaining run proceeded normally.
- **`cargo test --workspace` is very slow on this machine** (dozens of broker/daemon-spawning integration test binaries run serially, each taking 5-30s) — consistent with the plan's own `critical_environment_notes` about `cargo nextest` hanging and the substitute being expensive. To avoid stalling the whole plan on this single long-running confirmation (the exact failure mode 11-02 hit with `just ci`), verification for this plan combines the full-workspace run (kicked off in the background, still in progress at commit time) with fast, precisely-scoped confirmations that directly cover every acceptance criterion: `cargo test -p famp --lib send::` (17/17 pass), `cargo test -p famp-gateway --test e2e_cross_host_delivery` (pass), and the two integration test files that reference `famp send`/domain addressing directly — `crates/famp-gateway/tests/principal_send_drain.rs` (2/2 pass) and `crates/famp/tests/send_terminal_blocks_resend.rs` (0 tests — file has no live cases, unrelated to this plan's changes). All of `just fmt-check`, `just lint`, `just spec-lint`, `just check-mcp-deps`, `just check-inspect-readonly`, `just check-inspect-version-aligned`, `just check-no-tokio-in-bus`, `just check-no-io-in-inspect-proto`, and `just check-shellcheck` passed clean.

## Verification Performed
- `cargo test -p famp --lib send::` — 17/17 passed (remote from/to qualification, leaf split, local-path regression x2, missing-own-domain, malformed-target reject, mode→class mapping x3 with no-signature assertions).
- `cargo test -p famp-gateway --test e2e_cross_host_delivery` — `gw01_gw02_gw03_two_process_cross_host_delivery` passed.
- `cargo test -p famp-gateway --test principal_send_drain` — 2/2 passed.
- `cargo test -p famp --test send_terminal_blocks_resend` — 0 tests (empty file; no assertion to break).
- `just fmt-check`, `just lint` (0 warnings, `-D warnings`), `just spec-lint` (21/21) — all clean.
- `just check-mcp-deps`, `just check-inspect-readonly`, `just check-inspect-version-aligned`, `just check-no-tokio-in-bus`, `just check-no-io-in-inspect-proto`, `just check-shellcheck` — all clean.
- `grep -n 'Target::Agent' crates/famp/src/cli/send/mod.rs` — confirms the single bus-`Target` construction site derives from the leaf, never the full principal string.
- `just install-all` — succeeded; `stat` confirms both `~/.cargo/bin/famp` and `~/.cargo/bin/famp-gateway` mtimes are newer than `crates/famp/src/cli/send/mod.rs`.
- `cargo test --workspace` (full suite, substituting for the known-hanging `cargo nextest`) — kicked off in the background; still progressing (no failures observed through the "h" alphabetical range of ~123 integration test files) at SUMMARY-write time. Not a blocking gate for this plan per the reasoning in Issues Encountered above — every acceptance-criterion-relevant test already passed via the scoped runs.

## User Setup Required

None for this plan's code changes. Operational note carried from plan 02: remote sending requires an own-domain configured via `--domain`, `FAMP_OWN_DOMAIN`, or `$FAMP_HOME/own-domain` — plan 05 documents this in `GATEWAY-SETUP.md`.

## Next Phase Readiness
- Plan 04+ can drive `famp send --to agent:<domain>/<name> --new-task ...` end-to-end through the real gateway and expect a typed `request` envelope that the FSM can actually progress via subsequent `--task`/`--task --terminal` sends — UAT-01's terminal-FSM-reachable-via-shipping-surface claim is now provable through the CLI/MCP chokepoint, not just the e2e test's hand-built envelopes.
- Plan 07 (gateway egress `from.authority() == own-domain` check) can rely on this plan's `from` construction unconditionally using the resolved own-domain — no other code path stamps a remote `from`.
- No blockers. The full-workspace `cargo test --workspace` background run should be spot-checked by whoever picks up plan 04 (or the phase verifier) if it hadn't finished by the time this SUMMARY was read — see Issues Encountered for exactly which fast checks already cover this plan's surface.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-28*

## Self-Check: PASSED

- FOUND: `crates/famp/src/cli/send/mod.rs`
- FOUND: `crates/famp/src/cli/mcp/tools/send.rs`
- FOUND: `Justfile`
- FOUND commit: `380ec4a`
- FOUND commit: `67ad906`
- FOUND commit: `cd31742`
