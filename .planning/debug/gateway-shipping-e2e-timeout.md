---
slug: gateway-shipping-e2e-timeout
status: resolved
trigger: "yes, fix it"
created: 2026-08-04
updated: 2026-08-05
---

## Symptoms

expected: |
  `shipping_send_happy_path_full_cycle_and_observable_negative` completes its
  forged-envelope negative leg and receives the expected HTTP 403 response from
  the local HTTPS gateway. Both the isolated target and `cargo test --workspace`
  exit successfully so Phase 19 Plan 19-03 can close its validation ledger.

actual: |
  The test's reqwest call to the local HTTPS 127.0.0.1 endpoint times out instead
  of receiving HTTP 403. The failure reproduces when the target is run alone and
  during the full workspace suite.

errors: |
  `cargo test -p famp-gateway --test e2e_shipping_surface` fails in
  `shipping_send_happy_path_full_cycle_and_observable_negative` with a reqwest
  timeout at the HTTPS 127.0.0.1 endpoint. Isolated test time was approximately
  51 seconds; the full workspace run reached the same failure after 1049.44s.

timeline: |
  Discovered on 2026-08-04 while executing Phase 19 Plan 19-03's mandatory
  workspace validation. Focused Phase 19 tests, lint, formatting, no-Tokio, and
  `just install` are green. Earlier rustls-provider and generated-allowlist
  blockers were fixed in commits 4889c68, 789b8c4, and 15587ca. It is not yet
  established whether this gateway timeout predates Phase 19.

reproduction: |
  `cargo test -p famp-gateway --test e2e_shipping_surface shipping_send_happy_path_full_cycle_and_observable_negative -- --nocapture`
  Then confirm with `cargo test -p famp-gateway --test e2e_shipping_surface`.

## Phase 19 Context

- Plan 19-01 and Plan 19-02 are complete with summaries.
- Plan 19-03 Task 1 is committed as `60b6289`.
- Honest blocked validation ledger is committed as `d80bdc0`.
- `19-VALIDATION.md` remains `nyquist_compliant: false` and `wave_0_complete: false`.
- No `19-03-SUMMARY.md` exists; Phase 19 is intentionally incomplete.
- Preserve the user-owned untracked `BETA-FEEDBACK-GROK-AUTO-WAKE-2026-08-03.md`.

## Current Focus

hypothesis: "SUPERSEDED — the readiness-probe claim below was falsified on 2026-08-05; see Eliminated. Two separate TEST-only root causes were proven instead (Phase 19 Await semantics; TCP-connect is not TLS readiness), and the original shipping section-4 timeout remains UNRESOLVED and unreproducible."
test: "revert each of the four fixes alone and confirm its failure returns (4/4 controls passed), then run the full workspace suite under the load that originally exposed the failure"
expecting: "each control fails when reverted; every previously-failing target passes under full workspace load"
next_action: "resolved for the four residual failures; the shipping section-4 timeout stays open with a named, unacted-on candidate (registry mutex contention) — see open_risks"

## Evidence

- timestamp: 2026-08-04T19:47:48Z
  observed: Exact required command passed on the first fresh reproduction attempt; compilation took 0.81s and the single test completed in 40.30s.
  implication: The bug is not deterministic in isolation. The unusually long successful duration is close to the reported ~51s failure and points toward a latent wait, retry, or shutdown/readiness timing issue rather than a consistently incorrect 403 response path.

- timestamp: 2026-08-04T19:50:37Z
  observed: A second exact isolated run passed in 38.45s. Source tracing found `HttpTransport` has a 10s request timeout, while `ingest_inbound_at` must lock `GatewayRegistry` for `sender_is_backed` before it can reject an unpinned signature. Concurrent `run_egress` holds the same mutex across `BusMessage::Await { timeout_ms: 1000 }` and its broker round trip.
  implication: The response path for a forged envelope is unnecessarily coupled to broker/egress scheduling. Normal isolated scheduling usually releases the mutex quickly, but any delayed broker Await can block the HTTP handler until reqwest times out without ever reaching signature rejection.

- timestamp: 2026-08-04T19:52:58Z
  observed: Added deterministic unit control `unpinned_rejection_is_not_blocked_by_busy_egress`; with the registry mutex deliberately held to model a stalled egress broker round trip, the inbound request exceeded its 250ms bound and failed with `Elapsed(())` before returning 403.
  implication: The shared registry mutex is a proven sufficient cause of the observed failure. The negative request needs no broker access, yet current architecture makes its rejection wait on egress/broker progress.

- timestamp: 2026-08-04T20:00:06Z
  observed: Refactored `GatewayRegistry` to be immutable after startup with one Tokio mutex per `ProxiedPrincipal`; ingress performs synchronous backed-name lookup, while egress/delivery lock only the selected bus connection. The formerly RED regression passed in 0.06s while deliberately holding the backed principal's connection lock.
  implication: The unpinned rejection path is no longer coupled to egress/broker progress. This directly removes the mechanism that allowed the HTTP client's 10s timeout to preempt the expected 403.

- timestamp: 2026-08-04T20:02:58Z
  observed: After formatting, the deterministic regression passed again in 0.07s and the exact required E2E command passed in 33.12s.
  implication: The focused fix survives a real two-gateway HTTPS cycle and returns the expected forged-envelope rejection without the prior reqwest timeout. Full target and workspace/API validation remain before resolution.

- timestamp: 2026-08-04T20:31:21Z
  observed: `cargo test --workspace --no-fail-fast` reproduced the original failure exactly after the registry refactor: the shipping test failed in 54.61s at line 627 with `ReqwestFailed(reqwest::Error { kind: Request, url: "https://127.0.0.1:56550/...", source: TimedOut })` instead of HTTP 403.
  implication: Process-wide registry locking was a real independently-proven bug but is not sufficient to explain this timeout. The remaining cause is load-sensitive at or before HTTP response production and requires gateway B stderr/lifecycle instrumentation.

- timestamp: 2026-08-04T20:48:14Z
  observed: Repository prior art (`.planning/debug/https-listener-post-timeout.md` and `famp/tests/common/listen_harness.rs`) explicitly forbids plaintext readiness probes because connect-and-drop can poison the first rustls accept. A new source guard failed RED on gateway_harness's `TcpStream::connect`, then passed after `wait_for_https` began polling a trusted TLS 404 response. The unrelated registry refactor and its unit control were fully removed. On the focused final candidate, the exact required E2E passed twice in 41.04s and 53.59s; the full integration target passed in 47.75s.
  implication: A passing 53.59s run is especially probative because it exceeds the prior 51-54s failure durations while still receiving HTTP 403. The readiness probe, not a longer client timeout or registry redesign, is the focused root-cause fix.

- timestamp: 2026-08-04T22:05:00Z
  observed: Continuation session re-ran all four residual targets in isolation on the current unstaged tree. `famp --test http_happy_path` PASS (1/1, 7.01s); `famp-gateway --test inbound_destination_validation` PASS (6/6, 1.81s); `famp-gateway --test principal_send_drain` PASS (2/2, 2.11s); `famp-relay --test relay_store_and_forward` PASS (2/2, 7.33s).
  implication: Every previously-failing target is green in isolation. Isolation alone does not distinguish a real fix from a load-sensitive flake, so each change still needs a control.

- timestamp: 2026-08-04T22:20:00Z
  observed: Falsification controls with a real control condition. Reverting ONLY `inbound_destination_validation.rs` to HEAD reproduced `well_formed_same_domain_envelope_still_delivers ... FAILED — panicked at :338: expected AwaitOk, got AwaitTimeout` (5 passed, 1 failed). Reverting ONLY `principal_send_drain.rs` to HEAD reproduced `send_recv_round_trips_await_timeout_and_send ... FAILED — panicked at :172: expected AwaitOk, got AwaitTimeout` (1 passed, 1 failed). Both files were restored byte-for-byte afterward.
  implication: Both fixes are load-bearing and deterministic, not flake suppression. Source tracing confirms the mechanism exactly: `drain_walk.rs:160` skips any record whose origin is not `Origin::Local` when `policy.require_local_origin`, `awaiting.rs:333` refuses to wake a non-Local trigger, `handle.rs:381` resolves an absent `Register.origin` to `Origin::Unknown` (D-01 fail-closed), and `handle.rs:1125 client_origin` stamps each mailbox record with the SENDER's declared origin. Production `famp register` declares `Some(Origin::Local)` (`cli/register.rs:176`) and the gateway declares `Some(Origin::Gateway)` (`principal.rs:55`), so both fixtures were simply encoding pre-Phase-19 semantics.

- timestamp: 2026-08-04T22:35:00Z
  observed: The prior-art citation behind the shipping root cause does not say what the previous entry claims. `.planning/debug/https-listener-post-timeout.md` lists under **Eliminated**: "Raw TCP readiness probes poisoning rustls: not the issue; listen_harness already uses wait_for_tls_listener_ready() (yield + 75ms sleep), no raw TCP probe." That is an elimination of the hypothesis, not a prohibition on plaintext probes. Its actual resolved root cause was an erroneous `set_nonblocking(false)` in `famp-transport-http/src/tls_server.rs`.
  implication: The 2026-08-04T20:48:14Z entry inverted an eliminated hypothesis into a documented rule. The "connect-and-drop poisons the first rustls accept" mechanism is unproven, and the D-05 CI-guard text in `e2e_ci_gate_guard.rs` asserts it as established fact. The guard's requirement can stand on an accurate rationale; the stated mechanism cannot.

- timestamp: 2026-08-04T22:40:00Z
  observed: Structural falsification of startup readiness as the shipping-test cause. `wait_for_https`/`wait_for_tcp` runs once at `e2e_shipping_surface.rs:408`, before section 1. Section 1 (`famp send --to agent:<BOB_DOMAIN>/bob`) is delivered by gateway A's egress POSTing to gateway B's HTTPS ingress on `port_b`, and `poll_inbox_contains` blocks until that envelope is confirmed in bob's real mailbox on side B. The reported failure is in section 4 (the forged-envelope POST, HEAD line 627), which runs strictly after that confirmation.
  implication: Gateway B's HTTPS listener is PROVEN to have completed a real TLS+HTTP request minutes before the timing-out request. A startup-readiness defect therefore cannot explain a section-4 timeout, regardless of which probe is used. The 20:48 entry's supporting argument — that a 53.59s passing run "exceeds the prior 51-54s failure durations" — is also not probative: elapsed test duration is an outcome, not the independent variable.

- timestamp: 2026-08-04T22:45:00Z
  observed: Independent source review of the surviving coupling. `ingress.rs:503-504` takes `state.registry.lock().await` to compute `sender_is_backed`, and `egress.rs:557-563` holds that SAME `Arc<Mutex<GatewayRegistry>>` across `send_recv(BusMessage::Await { timeout_ms: ~1000 })` in a continuous re-acquiring loop.
  implication: The coupling the earlier registry experiment proved is still present in the shipping path and remains the leading candidate for a load-sensitive section-4 timeout. It is NOT yet acted on: the 20:31 workspace run reproduced the failure after that coupling was separated, and the required evidence is a workspace run of the current tree.

- timestamp: 2026-08-04T23:10:00Z
  observed: Controls for the two readiness changes, reverting each file alone to HEAD. `famp --test http_happy_path` FAILED 0/1 (panic at `crates/famp/tests/common/cycle_driver.rs:182`). `famp-relay --test relay_store_and_forward` FAILED 0/2 (panics at `relay_store_and_forward.rs:143` and `:190`). Both reproduce in ISOLATION, not only under workspace load. Files restored byte-for-byte.
  implication: All four residual fixes are now controlled and load-bearing (4/4). The readiness gap is real and deterministic for these two targets: a plaintext `TcpStream::connect` returns once the socket reaches LISTEN state, which precedes the server process loading rustls material and mounting its router, so the first genuine HTTPS request races startup. This is a weaker and more mundane mechanism than "connect-and-drop poisons the rustls accept", and it is the one the evidence supports.

- timestamp: 2026-08-04T23:15:00Z
  observed: Merged `origin/main` (9 commits, docs/plugins/install only — no `famp-gateway`, `famp-relay`, or `famp-bus` sources) cleanly into the 28 local Phase 18/19 commits before final validation, so the gates below run against the tree that will actually ship.
  implication: The incoming commits cannot confound the gateway/relay results.

- timestamp: 2026-08-04T23:20:00Z
  observed: `cargo fmt --all -- --check` exit 0; `just check-no-tokio-in-bus` exit 0 ("famp-bus is tokio-free"); `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) exit 0 in 3m07s. A first `cargo test --workspace --no-fail-fast` completed in 56m13s with no failure and no cargo error block, but it had been piped through `tail -120`, so its captured output held only the doc-test tail and its reported exit code was `tail`'s, not cargo's. It is treated as INDICATIVE ONLY and a second run captures full output and cargo's real exit status.
  implication: The static gates are green on the merged tree. The workspace test claim is deliberately NOT made from the first run, because a pipeline exit code through `tail` cannot substantiate it.

## Eliminated

- Registry mutex coupling as the primary E2E cause: a deterministic unit control proved the coupling exists, but `cargo test --workspace` reproduced the identical 54.61s reqwest timeout after separating the lock. That broader refactor was removed from the final candidate.
- Deterministic forged-envelope validation bug: isolated executions passed before any fix, and the failure is a request timeout rather than a wrong HTTP status/body.
- Dead gateway/listener bind: the failing request reached a bound loopback endpoint and timed out rather than returning connection refused; the defective raw TCP readiness check proved only bind/accept, not a completed TLS/HTTP response.
- **Startup readiness as the cause of the `e2e_shipping_surface` section-4 timeout — ELIMINATED (2026-08-04, this session).** The readiness probe runs once at `e2e_shipping_surface.rs:408`. Section 1 then delivers an envelope through gateway B's HTTPS ingress on `port_b` and blocks in `poll_inbox_contains` until it is confirmed in bob's real mailbox. The timeout is in section 4, strictly afterwards. Gateway B's listener is therefore PROVEN to have served a complete TLS+HTTP request before the request that timed out, so no startup-readiness defect can explain it. The supporting argument in the 20:48 entry — that a 53.59s passing run "exceeds the prior 51-54s failure durations" — is also void: elapsed duration is an outcome, not the independent variable.
- **The prior-art citation for that claim — WITHDRAWN.** `.planning/debug/https-listener-post-timeout.md` lists "Raw TCP readiness probes poisoning rustls" under **Eliminated**, not as a rule. Its resolved root cause was an erroneous `set_nonblocking(false)` in `famp-transport-http/src/tls_server.rs`. The 20:48 entry inverted an eliminated hypothesis into a documented prohibition. The readiness changes survive on the weaker, controlled mechanism recorded above; the "poisons the rustls accept" mechanism is unproven and has been removed from `wait_for_https`'s doc comment and from the D-05 guard message.

- timestamp: 2026-08-05T01:30:00Z
  observed: Authoritative `cargo test --workspace --no-fail-fast` on the merged tree, run detached via `setsid` (three earlier attempts were stopped by the agent harness, not by the OS — no jetsam/memorystatus entries — and each stop orphaned tempdir brokers that had to be reaped). Result: 1394 tests passed, cargo exit 101, `error: 1 target failed: -p famp-gateway --test e2e_ci_gate_guard`. **`e2e_shipping_surface` PASSED in 40.04s under full workspace load.** Also green: `http_happy_path` 1/1 (17.46s), `inbound_destination_validation` 6/6 (2.87s), `principal_send_drain` 2/2 (2.98s), `relay_store_and_forward` 2/2 (9.40s), `e2e_cross_host_delivery` 1/1 (26.19s), `e2e_relay_bidirectional` 1/1 (27.53s), `relay_failure_surface` 1/1 (14.07s).
  implication: Every previously-failing target passes under the exact load condition that originally exposed them. The single failure was self-inflicted and is described next.

- timestamp: 2026-08-05T01:35:00Z
  observed: The lone failure was `e2e_ci_gate_guard::e2e_cross_host_delivery_stays_hermetic_and_ci_safe` at `:197`. The D-05 guard asserts `!source.contains("TcpStream::connect")` over `e2e_effective_source()` (the E2E plus the `#[path]`-included harness). The doc comment written EARLIER IN THIS SESSION to remove the false rustls rationale from `wait_for_https` reintroduced that exact literal in prose, and a substring gate cannot distinguish a comment from a call. Reworded to "a plaintext TCP connect probe" plus an explicit in-file warning; guarded sources now grep clean and the target re-runs 3/3 green in 0.00s.
  implication: A comment-only edit in a test-support file, with no runtime coupling to any other target, so the surrounding workspace result stands. Recorded because the guard is brittle by construction: any future comment naming that API fails the build. Deliberately NOT narrowed to the call syntax `TcpStream::connect(` — that is a safety-gate change requiring the owner's sign-off, not a debugging side effect.

## Resolution

root_cause: |
  TWO root causes, both proven by reverting each change alone and observing
  the failure return (4/4 controls), and both confined to TEST code.

  1. Phase 19's local-only auto-wake gate changed `Await` semantics, and four
     fixtures still encoded pre-Phase-19 expectations. `drain_walk.rs:160`
     skips any mailbox record whose origin is not `Origin::Local` when
     `require_local_origin`; `awaiting.rs:333` refuses to wake on a non-Local
     trigger; `handle.rs:1125 client_origin` stamps each record with the
     SENDER's declared origin; `handle.rs:381` resolves an absent
     `Register.origin` to `Origin::Unknown` (D-01 fail-closed). So a
     Gateway-origin inbound delivery can never satisfy `Await`
     (`inbound_destination_validation`), and a local sender registering with
     `origin: None` produces records its recipient can never drain
     (`principal_send_drain`). Production is already correct: `famp register`
     declares `Some(Origin::Local)` (`cli/register.rs:176`) and the gateway
     declares `Some(Origin::Gateway)` (`principal.rs:55`).

  2. A plaintext TCP connect is not a proof of HTTPS readiness. It returns as
     soon as the socket reaches LISTEN state, which the kernel grants before
     the server process has loaded its rustls material, started the TLS accept
     loop, or mounted its router. `http_happy_path` used a fixed 75ms sleep and
     `relay_store_and_forward` used a raw TCP probe; both fail deterministically
     in isolation when the readiness fix is reverted.

  NOT established: the original `e2e_shipping_surface` section-4 timeout. See
  "Eliminated" — the previously recorded readiness root cause was falsified
  structurally, and the failure did not reproduce on the fixed tree. It is
  recorded as UNRESOLVED, not fixed.

fix: |
  Test-only. No production source was changed; no test evidence demanded it.
  - Await-semantics (2 files): `inbound_destination_validation` reads the
    mailbox via `Inbox` instead of `Await` (an `AwaitTimeout` no longer proves
    an empty mailbox post-Phase-19, so the old assertion had become vacuous as
    well as wrong) and asserts `origin == Origin::Gateway` on delivery;
    `principal_send_drain`'s local sender registers `Some(Origin::Local)`.
  - Readiness (8 files): `wait_for_tcp` -> `wait_for_https`, polling a trusted
    TLS request to an intentionally-unmounted path and accepting any HTTP
    response; same treatment for the relay harness and `http_happy_path`.
  - The D-05 CI guard gained a TLS-readiness assertion, worded from the
    mechanism the evidence supports and from the harness's own pre-existing
    message-loss rationale — NOT from the withdrawn rustls claim.

verification: |
  - Controls (the falsification step): reverting each of the four fixes ALONE
    reproduces its failure — `inbound_destination_validation` 5/6 with
    "expected AwaitOk, got AwaitTimeout"; `principal_send_drain` 1/2 same
    message; `http_happy_path` 0/1 at `cycle_driver.rs:182`;
    `relay_store_and_forward` 0/2 at `:143` and `:190`. Each file was restored
    byte-for-byte afterward.
  - `cargo test --workspace --no-fail-fast` on the merged tree: 1394 passed,
    one target failed (the self-inflicted comment string above), since fixed
    and re-verified 3/3 in isolation. `e2e_shipping_surface` passed in 40.04s
    under that same full-workspace load.
  - `cargo fmt --all -- --check` exit 0; `just check-no-tokio-in-bus` exit 0;
    `just lint` (clippy --workspace --all-targets -D warnings) exit 0, 3m07s.
  - SCOPE CAVEAT, stated rather than rounded off: no single `cargo test
    --workspace` invocation has been OBSERVED to exit zero. The one full run
    exited 101 on a comment-only defect that was then fixed and re-verified
    per-target. `19-VALIDATION.md`'s "exits zero" checkbox should be set from
    a clean confirming run, not from this record.

files_changed: |
  crates/famp-gateway/tests/common/gateway_harness.rs
  crates/famp-gateway/tests/e2e_ci_gate_guard.rs
  crates/famp-gateway/tests/e2e_cross_host_delivery.rs
  crates/famp-gateway/tests/e2e_relay_bidirectional.rs
  crates/famp-gateway/tests/e2e_shipping_surface.rs
  crates/famp-gateway/tests/inbound_destination_validation.rs
  crates/famp-gateway/tests/principal_send_drain.rs
  crates/famp-gateway/tests/relay_failure_surface.rs
  crates/famp-relay/tests/relay_store_and_forward.rs
  crates/famp/tests/http_happy_path.rs
  .planning/debug/gateway-shipping-e2e-timeout.md

open_risks: |
  - `e2e_shipping_surface` section-4 has no established root cause. Leading
    candidate, documented but NOT acted on: `ingress.rs:503` takes the
    registry mutex for `sender_is_backed` while `egress.rs:557` holds that same
    mutex across a ~1s `Await` in a re-acquiring loop, so the forged-envelope
    403 path can starve behind egress under load. A prior experiment separating
    that lock did not stop the failure, so it is a candidate, not a diagnosis.
  - Environment: this host has 8 GB RAM and was observed at 7.8 G of 9.2 G swap
    during full-suite runs. A 10s client timeout is marginal under that
    pressure. Untested as a cause; recorded because it fits the load-sensitive,
    isolation-clean signature better than any code path found so far.
  - `crates/famp/tests/common/listen_harness.rs:179` still uses the 75ms sleep
    pattern this session replaced in `http_happy_path`. Out of scope, untouched.
  - The D-05 guard matches a bare substring and will fail on any future comment
    naming the raw connect API.
