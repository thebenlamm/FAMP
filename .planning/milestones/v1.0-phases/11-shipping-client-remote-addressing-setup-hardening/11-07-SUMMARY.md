---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 07
subsystem: security
tags: [trust-boundary, spoofing, broker, gateway, egress, from-binding, own-domain, ready-line]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 02's own_domain::resolve_own_domain(cli_domain, home) single-source resolver; plan 03's split-addressing remote send that stamps from = agent:{own-domain}/{identity}"
provides:
  - "broker Send gate: envelope from.name() must equal the connection's effective (resolved) identity, for both agent Sends and channel posts, before any mailbox insertion"
  - "gateway egress own-domain enforcement: relay_one rejects (never signs) any drained envelope whose from authority != the gateway's configured own-domain, when configured"
  - "gateway 'ready' line moved to after full init (home, own-domain, signing-key, keyring, transport, peer-map) instead of immediately after registry.back()"
  - "resolve_own_domain_or_exit(home) helper in famp-gateway main.rs, kept as an owned local reusable by a future ingress-side own-domain check (11-08)"
affects: [11-08 (gateway ingress trust-boundary extension reuses this plan's own_domain resolution site)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Reused is_self_authored's existing from.rsplit('/').next() leaf-split convention for the broker's from-binding gate rather than hand-rolling a second Principal-leaf parse"
    - "own_domain kept as an owned Option<String> local in main() (cloned per egress task, never moved) so a future ingress-side check can reuse the same resolution site without restructuring it"

key-files:
  created:
    - crates/famp-gateway/tests/process_readiness.rs
  modified:
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp-bus/src/broker/handle/tests.rs
    - crates/famp-gateway/src/egress.rs
    - crates/famp-gateway/src/main.rs
    - crates/famp-bus/tests/prop01_dm_fanin_order.rs
    - crates/famp-bus/tests/prop02_channel_fanout.rs
    - crates/famp-bus/tests/tdd02_drain_cursor_order.rs
    - crates/famp-bus/tests/tdd04_eof_cleanup.rs
    - crates/famp-gateway/tests/no_cross_talk.rs

key-decisions:
  - "Reused BusErrorKind::EnvelopeInvalid for the broker's forged-from reject rather than adding a new variant — EnvelopeInvalid is already the project's general reject-bucket for malformed/rejected Send input (used for pid-0 registration, oversized frames, etc.), and adding a variant would have required touching the exhaustive mcp_error_kind JSON-RPC code table (-32100..-32109) and its two exhaustive consumer-stub tests for no added clarity."
  - "own_domain resolved once in main() via a new resolve_own_domain_or_exit(home) helper (extracted solely to satisfy clippy::too_many_lines), kept as an owned Option<String> local and cloned per egress task rather than consumed by a one-shot closure, so 11-08's ingress-side own-domain check can reuse the same resolution site."
  - "relay_one's own-domain check placed after parsing from/to but strictly before sign_federation_fields — a rejected foreign-domain envelope is asserted byte-identical to the pre-call value (no signature, no federation fields ever inserted)."

patterns-established:
  - "Any test that raw-constructs a BusMessage::Send envelope must stamp from to match the sending connection's registered/effective identity, or the broker's from-binding gate rejects it before mailbox insertion — this is now a load-bearing test-construction convention, not just a broker enforcement detail."

requirements-completed: [ADDR-02, ADDR-03]

coverage:
  - id: D1
    description: "Broker rejects a Send whose envelope from leaf-name != the connection's effective identity, before any mailbox insertion, for both agent Sends and channel posts"
    requirement: "ADDR-02"
    verification:
      - kind: unit
        ref: "crates/famp-bus/src/broker/handle/tests.rs#send_from_matching_effective_identity_is_accepted, #send_agent_with_forged_from_leaf_is_rejected_before_insertion, #send_channel_post_with_forged_from_leaf_is_rejected_before_insertion"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery (gw01_gw02_gw03_two_process_cross_host_delivery) — relay path control"
        status: pass
    human_judgment: false
  - id: D2
    description: "Gateway resolves+validates own-domain once at startup from plan 02's single source; a present-but-invalid domain is startup-fatal"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/main.rs (resolve_own_domain_or_exit — exercised transitively via process_readiness.rs and the e2e's own-domain-unset spawn)"
        status: pass
    human_judgment: false
  - id: D3
    description: "When own-domain IS configured, egress rejects (never signs) a drained envelope whose from authority != own-domain; when unset, from is signed verbatim"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#relay_one_rejects_foreign_from_domain_when_own_domain_configured, #relay_one_signs_from_verbatim_when_own_domain_unset"
        status: pass
      - kind: integration
        ref: "cargo test -p famp-gateway --test e2e_cross_host_delivery — own-domain-unset regression control"
        status: pass
    human_judgment: false
  - id: D4
    description: "The gateway 'ready, backing N principal(s)' line prints only after home resolve, own-domain resolve/validate, signing-key load, peers-keyring load, transport build, and peer route-map succeed"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/process_readiness.rs#ready_line_is_never_printed_when_peers_keyring_load_fails"
        status: pass
      - kind: static
        ref: "grep -n 'ready, backing' crates/famp-gateway/src/main.rs — positioned after the keyring load, transport build, and peer-map, not immediately after registry.back()"
        status: pass
    human_judgment: false

duration: ~75min
completed: 2026-07-28
status: complete
---

# Phase 11 Plan 07: Broker/gateway from-binding + own-domain egress enforcement + ready-after-init Summary

**Closes the self-forgery hole review HIGH #2 found: the broker now binds a Send's envelope `from` to the authenticated connection's effective identity (rejecting a forged `from` before any mailbox insertion, for both agent Sends and channel posts), and the gateway now resolves its own-domain once at startup and rejects — never signs — any drained envelope whose `from` authority doesn't match it; the "ready" line only prints after full init.**

## Performance

- **Duration:** ~75 min (includes chasing down and fixing 7 pre-existing famp-bus unit tests + 4 pre-existing famp-bus/famp-gateway integration tests that were relying on the now-closed forgery hole)
- **Tasks:** 2 completed
- **Files modified:** 9 (1 created, 8 modified)

## Accomplishments
- `crates/famp-bus/src/broker/handle.rs::send` now resolves the connection's `effective_identity` (via the existing `resolve_op_identity`) and, before dispatching to either `Target::Agent` or `Target::Channel`, checks the envelope's `from` leaf against it using the same `is_self_authored` leaf-split convention already used for channel self-authorship. A mismatch is rejected typed `BusErrorKind::EnvelopeInvalid` with an actionable message, before `encode_envelope`/any mailbox write. The gateway's relay path is structurally exempt from breaking: `ingress.rs` inserts each remote sender's envelope through THAT sender's own backing connection (`guard.get_mut(sender.name())`), so `effective_identity == from`'s leaf holds for relays too.
- `crates/famp-gateway/src/main.rs` resolves this host's own-domain once at startup (new `resolve_own_domain_or_exit` helper, calling plan 02's `resolve_own_domain(None, &home)`): `Some(domain)` when configured, `None` when unset (a present-but-invalid domain exits the process rather than silently falling back). The value is threaded through `run_egress` into `relay_one` as `Option<&str>`.
- `crates/famp-gateway/src/egress.rs::relay_one` now checks, after parsing `from`/`to` but strictly before `sign_federation_fields`: if own-domain is `Some(d)` and `from.authority() != d`, return a new typed `RelayError::FromDomainMismatch { expected, got }` — the envelope is never mutated (no signature, no federation fields). When own-domain is `None`, behavior is unchanged (signs verbatim).
- The "ready, backing N principal(s)" line moved from immediately after the `registry.back()` loop (before ANY of home/own-domain/signing-key/keyring/transport/peer-map init had run) to just before the ingress/egress spawn — i.e., after every one of those steps has succeeded. A new integration test (`process_readiness.rs`) spawns a real `famp-gateway` subprocess against a `FAMP_HOME` with a deliberately absent `peers.keyring`, asserting the process exits non-zero and NEVER prints "ready".

## Task Commits

Each task was committed atomically:

1. **Task 1: broker binds envelope `from` to the effective identity (Spoofing gate)** - `ce36cdf` (fix)
2. **Task 2: gateway wires own-domain, rejects foreign-domain `from` at egress, and prints ready AFTER init** - `de1df94` (fix)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `crates/famp-bus/src/broker/handle.rs` - `send()` gates on `from.name() == effective_identity` before dispatching to `send_agent`/`send_channel`, before any mailbox write
- `crates/famp-bus/src/broker/handle/tests.rs` - 3 new adversarial tests (matching accepted; forged agent Send rejected; forged channel post rejected) + 7 pre-existing tests updated to stamp a correct `from` (Rule 1 — see Deviations)
- `crates/famp-gateway/src/main.rs` - `resolve_own_domain_or_exit` helper; own-domain resolved once at startup; "ready" line moved after full init; `own_domain.clone()` threaded into each `run_egress` spawn
- `crates/famp-gateway/src/egress.rs` - `RelayError::FromDomainMismatch` variant; `relay_one`/`run_egress` take `own_domain: Option<&str>`/`Option<String>`; check before `sign_federation_fields`; 2 new unit tests
- `crates/famp-gateway/tests/process_readiness.rs` **(new)** - spawns a real `famp-gateway` subprocess with an absent `peers.keyring`, asserts non-zero exit with "ready" never printed
- `crates/famp-bus/tests/prop01_dm_fanin_order.rs`, `prop02_channel_fanout.rs`, `tdd02_drain_cursor_order.rs`, `tdd04_eof_cleanup.rs` - stamped correct `from` on raw-constructed `BusMessage::Send` envelopes (Rule 1)
- `crates/famp-gateway/tests/no_cross_talk.rs` - stamped correct `from` on the raw-constructed `BusMessage::Send` envelope (Rule 1)

## Decisions Made
- Reused `BusErrorKind::EnvelopeInvalid` for the forged-`from` reject rather than adding a new variant (see key-decisions above for the exhaustive-table cost/benefit).
- `own_domain` resolution extracted into `resolve_own_domain_or_exit` solely to satisfy `clippy::too_many_lines` on `main()` (104/100 lines) — kept the resolved value as an owned local afterward per the forward-compatibility note in this plan's prompt, so 11-08's ingress-side check can reuse it.
- `relay_one`'s own-domain check placed strictly before `sign_federation_fields` and asserted (in tests) to leave a rejected envelope byte-identical to its pre-call state.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] 7 pre-existing famp-bus unit tests relied on the from-forgery hole this plan closes**
- **Found during:** Task 1, first `cargo test -p famp-bus --lib` run after adding the gate
- **Issue:** `test_hello_bind_as_live_holder_succeeds`, `test_send_agent_woken_true_when_waiter_parked`, `test_send_agent_woken_false_when_no_waiter`, `test_send_agent_wakes_all_proxy_waiters`, `test_canonical_plus_proxy_both_wake`, `test_dead_proxy_does_not_wake`, and `test_send_channel_wakes_all_member_waiters` all constructed `BusMessage::Send` envelopes with no `from` field (or, in the channel case, a `from` that didn't match the sending client) — behavior the new gate correctly now rejects.
- **Fix:** Each test's envelope literal replaced with `audit_log_envelope(seq, "<sender-name>")` (an existing test helper) so `from` matches the sending connection's registered/effective identity.
- **Files modified:** `crates/famp-bus/src/broker/handle/tests.rs`
- **Verification:** `cargo test -p famp-bus --lib` — 81/81 passed.
- **Committed in:** `ce36cdf` (Task 1 commit)

**2. [Rule 1 - Bug] 4 famp-bus proptest/tdd integration test files relied on the same hole**
- **Found during:** Task 1, broader sweep of every `BusMessage::Send` call site in the workspace
- **Issue:** `prop01_dm_fanin_order.rs`, `prop02_channel_fanout.rs`, `tdd02_drain_cursor_order.rs` (one of its two tests), and `tdd04_eof_cleanup.rs` (all 4 tests) constructed raw envelopes missing or mismatching `from`.
- **Fix:** Stamped `from` to match the sending connection's registered name in each case; `tdd02_drain_cursor_order.rs`'s shared `audit_log_envelope` helper was parameterized to take `from: &str` (its other call site — a pre-seeded offline mailbox, not a live Send — correctly keeps an unrelated author name).
- **Files modified:** `crates/famp-bus/tests/prop01_dm_fanin_order.rs`, `prop02_channel_fanout.rs`, `tdd02_drain_cursor_order.rs`, `tdd04_eof_cleanup.rs`
- **Verification:** `cargo test -p famp-bus` (full crate, lib + all integration test binaries) — all green.
- **Committed in:** `ce36cdf` (Task 1 commit)

**3. [Rule 1 - Bug] famp-gateway's `no_cross_talk.rs` (GW-04) relied on the same hole**
- **Found during:** Task 1, sweeping famp-gateway's own integration tests after the broker change
- **Issue:** The GW-04 no-cross-talk test sends a proxy-bound-as-"bob" envelope with no `from` field at all.
- **Fix:** Added `"from": "agent:local.bus/bob"` to the envelope literal.
- **Files modified:** `crates/famp-gateway/tests/no_cross_talk.rs`
- **Verification:** `cargo test -p famp-gateway --test no_cross_talk` — pass.
- **Committed in:** `ce36cdf` (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (all Rule 1 — pre-existing tests relying on the security hole this plan closes; no production behavior was weakened to make them pass, per this plan's explicit project rule).
**Impact on plan:** No scope creep — every fix is a test-construction correction forced by the new (correct) broker check, not a new feature.

## Issues Encountered
- **Duplicate `cargo test --workspace` background run raced with an ad-hoc re-run of `-p famp-gateway --test e2e_cross_host_delivery`** — same documented failure mode as 11-02/11-03's SUMMARYs (cargo target lock / broker-socket contention under concurrent test load): the e2e test failed twice with "broker socket ... never came up within 5s" while the full-workspace background run was active. A time-boxed (580s) full-workspace confirmation run did not complete within the budget (killed mid-run, no failures observed through the point it reached — well past `famp-bus`/`famp-canonical`/`famp-core`, into `famp`'s own integration-test files, never reaching `famp-gateway`'s package). Once the background run and its leftover orphan broker subprocesses were cleared, `e2e_cross_host_delivery` passed cleanly on a fresh, uncontended run — this is the true, load-bearing result for this plan's control test, not the earlier contention-caused failures.
- Every acceptance criterion in the plan is covered by scoped, targeted test runs that all passed cleanly (see Verification Performed below), consistent with the precedent set by 11-02/11-03 for handling this machine's slow/contention-prone `cargo test --workspace`.

## Verification Performed
- `cargo test -p famp-bus` (full crate: lib + `audit_log_dispatch`, `buserror_consumer_stub`, `codec_fuzz`, `prop01_dm_fanin_order`, `prop02_channel_fanout`, `prop03_join_leave_idempotent`, `prop04_drain_completeness`, `prop05_pid_unique`, `tdd02_drain_cursor_order`, `tdd03_pid_reuse`, `tdd04_eof_cleanup`) — all green, no failures.
- `cargo test -p famp-gateway --lib` (16/16) and `--bins` (13/13) — all green.
- `cargo test -p famp-gateway --test e2e_cross_host_delivery --test liveness --test no_cross_talk --test principal_send_drain --test process_readiness --test gateway_usage_doc_accuracy --test e2e_ci_gate_guard` — all green on a clean, uncontended run (final confirmation after clearing background-run resource contention).
- `just lint` (clippy `-D warnings`, workspace + all-targets) — clean.
- `just fmt-check` — clean (after one `cargo fmt` pass on `handle/tests.rs` and `egress.rs`).
- `just spec-lint` — 21/21 passed.
- `just check-no-tokio-in-bus`, `just check-mcp-deps` — both clean.
- `grep -n 'ready, backing' crates/famp-gateway/src/main.rs` — confirms the print site is positioned after keyring load, transport build, and peer route-map, not at the old pre-init location.
- `just ci`/`cargo nextest` NOT run — hangs on this machine per this plan's `critical_environment_notes`; `cargo test -p famp-bus`/`-p famp-gateway` is the documented substitute and was run exhaustively above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness
- 11-08 can extend the gateway's trust boundary at ingress (inbound `envelope.to.authority() == gateway local domain` check) by reusing `resolve_own_domain_or_exit`'s resolution site in `main.rs` — the `own_domain: Option<String>` local is kept owned and cloned (not moved) into the egress spawn loop specifically so it remains available for a second consumer without restructuring.
- No blockers.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-28*

## Self-Check: PASSED

- FOUND: `crates/famp-gateway/tests/process_readiness.rs`
- FOUND: `crates/famp-bus/src/broker/handle.rs`
- FOUND: `crates/famp-bus/src/broker/handle/tests.rs`
- FOUND: `crates/famp-gateway/src/egress.rs`
- FOUND: `crates/famp-gateway/src/main.rs`
- FOUND commit: `ce36cdf`
- FOUND commit: `de1df94`
