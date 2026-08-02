---
phase: 17-protocol-grade-ingress-reachability-implementation
plan: 05
subsystem: reachability
tags: [relay-fetch, ed25519, signed-fetch, tokio, reqwest, gateway, e2e]

# Dependency graph
requires:
  - phase: 17 (plans 01-03)
    provides: "famp-gateway's single ingest core (ingress::ingest_inbound) and its four pre-verify cheap gates (audience, freshness, replay, rate-limit), which this plan's relay-fetch client inherits for free"
  - phase: 17 (plan 04)
    provides: "famp-relay's signed-fetch wire contract (RELAY_FETCH_ROUTE, the four x-famp-relay-* headers, sign_fetch_auth/verify_fetch_auth, normalize_audience) — this plan is its only client"
provides:
  - "crates/famp-gateway/src/relay_fetch.rs: run_relay_fetch, a polling signed-fetch drain loop authorized by the gateway's already-loaded Ed25519 identity key, routing every fetched envelope through ingest_inbound"
  - "GatewayIngressState widened to pub with a pub constructor, and build_gateway_router/run_ingress now take an externally-supplied IngressGuard, so main.rs shares exactly ONE guard between the HTTP router and the relay-fetch loop"
  - "main.rs: --relay-fetch spawns run_relay_fetch as an additional tokio::select! branch; any unrecognized --flag (including the never-shipped --relay-token) is now a distinct usage error, not silently absorbed as a positional name"
  - "crates/famp-gateway/tests/e2e_relay_bidirectional.rs: three real processes (relay + gateway A + gateway B) proving bidirectional delivery with NEITHER gateway's argument vector containing the other's listen address"
affects: [18 (follower-doc disclosure obligations D-25/D-27/D-29/DOC-06)]

# Tech tracking
tech-stack:
  added: [reqwest (famp-gateway direct dep, rustls-no-provider feature, for relay_fetch.rs's own HTTPS client)]
  patterns:
    - "relay_fetch.rs never assembles or verifies a signed form itself — it calls famp_relay::fetch_auth::sign_fetch_auth and famp_relay::fetch_auth::normalize_audience exclusively, so client/server agreement is enforced by one shared definition, proven by a unit test against the relay's own verify_fetch_auth"
    - "One IngressGuard per process: GatewayIngressState::new(registry, keyring, own_domain, guard) is now the sole constructor, called once in main.rs and shared (via Arc::clone) between run_ingress's router and run_relay_fetch's loop — never two independently-built guards"
    - "poll_once/ingest_success_batch extracted from run_relay_fetch purely to satisfy clippy::too_many_lines, mirroring main.rs's own resolve_own_domain_or_exit/build_route_map extraction precedent"

key-files:
  created:
    - crates/famp-gateway/src/relay_fetch.rs
    - crates/famp-gateway/tests/e2e_relay_bidirectional.rs
  modified:
    - crates/famp-gateway/Cargo.toml
    - crates/famp-gateway/src/lib.rs
    - crates/famp-gateway/src/ingress.rs
    - crates/famp-gateway/src/main.rs
    - crates/famp-gateway/tests/inbound_destination_validation.rs

key-decisions:
  - "GatewayIngressState widened from pub(crate) to pub (deviation from the plan's literal file list, Rule 3): main.rs — a separate crate from famp-gateway's lib target — must construct one to hand to run_relay_fetch, and a pub(crate) type cannot be named or constructed from outside the defining crate. ingest_inbound itself was NOT widened and stays pub(crate), unchanged, per the plan's explicit prohibition."
  - "run_relay_fetch's signature gained a trust_cert: Option<PathBuf> parameter beyond what the plan's artifact list literally specified. HttpTransport (the crate's existing outbound HTTPS client) exposes no raw-GET-with-custom-headers surface, so relay_fetch.rs builds its own reqwest::Client via the same famp_transport_http::tls::build_client_config construction — and that client must trust the SAME --trust-cert the operator already configures for egress, or every fetch would fail TLS verification against the relay's cert. Reusing the existing --trust-cert flag (rather than adding a second, relay-specific one) keeps the CLI surface from widening."
  - "The relay-fetch drain loop's HTTP status handling is factored into a pure classify_fetch_status(StatusCode) -> FetchOutcome function, unit-tested directly, rather than asserted only through the slower three-process e2e — mirrors this codebase's established pure-predicate-extraction convention (e.g. egress.rs::sender_is_itself_backed)."
  - "--relay-token was never actually added to this codebase (17-03 corrected the plan before executing it, per D-26). This task's real work was therefore a regression LOCK, not a removal: any unrecognized --flag is now a distinct usage error naming the offending token, rather than being silently pushed onto the positional-names list. A dead credential-shaped flag on an open-internet-facing gateway would send an operator hunting for a secret that doesn't exist; an honest unknown-flag error is the safer failure mode."
  - "e2e_relay_bidirectional.rs does not reuse gateway_harness.rs::spawn_gateway (its fixed shape always points --peer at the sibling gateway's own port). A new gateway_args_via_relay() builds the argument Vec<String> explicitly, asserted against directly (not only in a comment) before the gateway is spawned with those exact args — the negative property (no direct peer address anywhere) is provable by a reviewer reading the assertion, not by trusting a comment."
  - "One narrow new lint suppression: #[allow(clippy::literal_string_with_formatting_args)] on relay_fetch.rs::build_fetch_url's '{domain}' path-template placeholder comparison. Direct precedent already exists in this codebase for the identical false positive: famp-relay/tests/relay_store_and_forward.rs::fetch_url carries the same allow for the same reason."

requirements-completed: [REACH-04]

coverage:
  - id: D1
    description: "run_relay_fetch: a signed relay-fetch drain loop authorized with the gateway's already-loaded Ed25519 identity key, routing every fetched envelope through the single ingest_inbound core (same gates as the direct HTTPS path)"
    requirement: REACH-04
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#fetch_authorization_built_here_verifies_at_the_relay_for_matching_domain_and_audience"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#fetch_authorization_built_here_fails_at_the_relay_for_a_different_domain"
        status: pass
      - kind: integration
        ref: "crates/famp-gateway/src/relay_fetch.rs#relay_fetched_stale_timestamp_is_rejected_same_as_a_direct_post"
        status: pass
      - kind: integration
        ref: "crates/famp-gateway/src/relay_fetch.rs#relay_fetched_replayed_nonce_is_rejected_on_second_delivery"
        status: pass
      - kind: integration
        ref: "crates/famp-gateway/src/relay_fetch.rs#relay_fetched_bad_signature_is_rejected_and_performs_zero_registry_mutation"
        status: pass
      - kind: integration
        ref: "crates/famp-gateway/src/relay_fetch.rs#relay_supplied_recipient_disagreeing_with_signed_to_is_rejected_misaddressed"
        status: pass
    human_judgment: false
  - id: D2
    description: "decode_fetch_batch: parses the relay's fetch-response batch, decoding byte-identical bodies and failing the WHOLE batch on any malformed entry rather than silently skipping it"
    requirement: REACH-04
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#decode_fetch_batch_round_trips_byte_identical_bytes"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#decode_fetch_batch_empty_envelopes_is_an_empty_vec_not_an_error"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#decode_fetch_batch_unparseable_recipient_is_bad_recipient_and_drops_nothing_silently"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#decode_fetch_batch_invalid_base64_is_malformed_response"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/relay_fetch.rs#decode_fetch_batch_one_malformed_entry_fails_the_whole_batch"
        status: pass
    human_judgment: false
  - id: D3
    description: "REACH-04 loopback half: two gateways with NO direct address for each other, only a relay URL, exchange envelopes bidirectionally through a real three-process topology, both task FSMs reaching COMPLETED"
    requirement: REACH-04
    verification:
      - kind: e2e
        ref: "crates/famp-gateway/tests/e2e_relay_bidirectional.rs#reach_04_bidirectional_delivery_through_relay_with_no_direct_peer_address"
        status: pass
    human_judgment: false
  - id: D4
    description: "REACH-04 cross-network half (genuinely different networks, no shared VPN) — explicitly NOT proven by this plan, blocked on Ben (carrier hotspot / second network). Carried forward as a clearly-marked pending item per the Phase 10 DOC-04 precedent."
    human_judgment: true
    rationale: "Requires Ben to run the test from a second physical network; cannot be automated or verified by the executor."
  - id: D5
    description: "--relay-token regression lock: an unrecognized --flag is a distinct usage error naming it, never silently swallowed as a positional principal name; the printed usage string never mentions --relay-token"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/main.rs#relay_token_flag_is_rejected_as_unrecognized_never_swallowed_as_a_name"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/main.rs#usage_error_names_relay_fetch_and_never_relay_token"
        status: pass
    human_judgment: false

duration: ~5h
completed: 2026-08-02
status: complete
---

# Phase 17 Plan 05: Signed Relay-Fetch Loop and the REACH-04 Loopback Proof Summary

**`famp-gateway` gained an inbound path that works from behind NAT — a polling relay-fetch loop authorized by the gateway's own already-loaded Ed25519 key, feeding every fetched envelope through the exact same ingest core as a direct HTTPS POST — and a three-process loopback e2e proves it works bidirectionally with no direct peer address anywhere in either gateway's configuration.**

## Performance

- **Duration:** ~5h
- **Completed:** 2026-08-02
- **Tasks:** 2/2
- **Files modified:** 7 (2 created, 5 modified)

## REACH-04 status (report explicitly, per the plan's `<output>` contract)

**Loopback half: PROVEN.** `crates/famp-gateway/tests/e2e_relay_bidirectional.rs::reach_04_bidirectional_delivery_through_relay_with_no_direct_peer_address` stands up three real processes (a `famp-relay`, gateway A, gateway B) on loopback. Neither gateway's argument vector contains the other's `--listen` address anywhere — the test asserts this directly (`!joined_a.contains(&format!(":{port_b}"))` and the symmetric check), not only in a comment — and the ONLY path between them is the relay's URL, configured via both `--peer <domain>=<relay-url>` (egress, needed zero new code) and `--relay-fetch <relay-url>` (ingress, this plan's deliverable). An `alice -> bob` request, a `bob -> alice` commit and terminal deliver, and an `alice -> bob` ack all traverse the relay in both directions; both sides converge on `famp inspect tasks`' `COMPLETED` state. Confirmed passing standalone (`cargo test -p famp-gateway --test e2e_relay_bidirectional`, exit 0, 1 passed, ~15-19s across five separate runs this session) and inside the full `cargo test --workspace --no-fail-fast` run.

**Genuinely-different-networks half: PENDING, NOT proven by this plan.** Blocked on Ben running the equivalent from a second physical network (carrier hotspot or similar) — the exact same gap 17-CONTEXT.md's `<domain>` section named up front. Carried forward as a clearly-marked pending item, same pattern as Phase 10's DOC-04 precedent, never silently implied closed.

**REACH-02 (real symmetric-NAT validation) stays OPEN regardless of this plan's outcome.** Nothing in this plan closes it, and nothing here should be read as satisfying it — it remains blocked on Ben's carrier hotspot, independent of REACH-04's own status.

**REQUIREMENTS.md's REACH-04 checkbox was deliberately left UNCHECKED** by this executor. The row in the traceability table (line ~185) is updated to reflect the loopback proof + pending cross-network leg accurately, but the top-level checkbox decision is left to the orchestrator/human, per this session's explicit direction (the same discipline 17-03/17-CONTEXT.md already established after a prior premature tick had to be reverted).

## D-26 disclosure: no new credential input, `--relay-token` removed vs. never added

`--relay-token` was **never actually implemented** in this codebase. 17-03-PLAN.md was corrected before its own execution (documented in 17-03-SUMMARY.md and 17-CONTEXT.md's D-26 addendum) to add only `--relay-fetch`, once the lead settled on signed-fetch over a bearer-credential design. This task's real job was therefore a **regression lock**, not a removal: `main.rs::parse_args` now rejects any unrecognized `--`-prefixed argument as a distinct usage error naming the offending flag (`relay_token_flag_is_rejected_as_unrecognized_never_swallowed_as_a_name`), rather than silently treating it as a positional principal name — the prior behavior, which would have let a future `--relay-token` reintroduction pass parsing silently. The printed `USAGE` string never mentions `--relay-token` (`usage_error_names_relay_fetch_and_never_relay_token`, pre-existing, still green).

The gateway needs **no new credential-shaped input** for the relay path: the relay URL comes from `--relay-fetch`, the domain from the already-mandatory own-domain, the audience from `famp_relay::fetch_auth::normalize_audience` applied to the relay URL, and the credential from the identity key the gateway already loads for egress (`famp::cli::peer::identity::load_or_generate`, idempotent, T-08-12). `e2e_relay_bidirectional.rs` asserts this directly: neither gateway's argument vector contains the substring `"token"` or `"secret"` anywhere.

## For Phase 18's follower-doc author (D-25/D-27/D-29/DOC-06, restated verbatim per the plan's `<output>` requirement)

1. **The relay operator can read message content in plaintext.** FAMP signs but does not encrypt (D-25) — TLS terminates at the relay box, and signing gives integrity/authenticity, never confidentiality. Nothing in this plan, `relay_fetch.rs`, or the new e2e test describes the relay path as private or encrypted anywhere. This must be stated plainly and prominently in Phase 18's follower doc — a second person cannot meaningfully consent to using Ben's relay without knowing this.
2. **Pairing a new peer requires a manual operator step (D-27, accepted friction).** Queue ownership at the relay is explicit `--domain <domain>=<pubkey>` config, never TOFU/first-come. Adding a peer means: the peer runs `famp peer export --as <principal>` and sends the operator the SECOND whitespace-separated field (their base64url public key) — `e2e_relay_bidirectional.rs` exercises this exact real operator workflow to obtain each side's pubkey, rather than reaching into the identity file — and the operator adds a `--domain <their-domain>=<their-pubkey>` entry and restarts the relay. This is a real cost against the unassisted-follower bar and must be written into Phase 18's doc as an explicit step, not hidden.
3. **The runtime symptom of that friction, as built:** a 401/403 from the relay produces a distinct, actionable log line (`RelayFetchError::Unauthorized`'s `Display`) naming the D-27 remedy verbatim — "most likely the relay operator has not yet configured this gateway's public key for this domain; run `famp peer export`..." — distinct from both a transport failure (`RelayFetchError::Transport`, unreachable relay) and a 429 rate limit (`RelayFetchError::RateLimited`, deliberately NO misconfiguration-shaped log, since a rate limit is transient and messages stay queued until their TTL). The loop backs off (doubling, capped at `RELAY_FETCH_BACKOFF_MAX_MS` = 30s) and keeps polling in all three cases — it never panics, never exits, never treats a failed fetch as an empty queue.

## Accomplishments

- **`crates/famp-gateway/src/relay_fetch.rs`** (Task 1): `run_relay_fetch`, a polling drain loop that signs each fetch authorization with the gateway's existing Ed25519 identity key via `famp_relay::fetch_auth::sign_fetch_auth` (never assembling the signed form itself), decodes the relay's response batch via `decode_fetch_batch` (fails the whole batch on any malformed entry, never skips silently), and hands every entry to `ingress::ingest_inbound` — the SAME single ingest core the direct HTTPS path uses, so the relay path inherits all four cheap gates (audience, freshness, replay, rate-limit) plus signature verification identically.
- **`GatewayIngressState`** widened from `pub(crate)` to `pub`, with a new `pub const fn new(...)` constructor, so `main.rs` (a separate crate from the lib target) can assemble one state shared between `run_ingress`'s router and `run_relay_fetch`'s loop. `build_gateway_router`/`run_ingress` now take an externally-supplied `Arc<Mutex<IngressGuard>>` rather than constructing their own — exactly ONE guard per process (INGR-02/06/08), never two independently-built ones that would double every replay/rate-limit budget.
- **`main.rs`** (Task 2): spawns `run_relay_fetch` as an additional `tokio::select!` branch (extracted into `run_relay_fetch_branch` to satisfy `clippy::too_many_lines`) when `--relay-fetch` is configured; a permanently-pending future otherwise, so a non-relay gateway's behavior is byte-identical to before this plan. Any unrecognized `--`-prefixed argument is now a distinct, actionable usage error.
- **`crates/famp-gateway/tests/e2e_relay_bidirectional.rs`** (Task 2): three real processes — `famp-relay`, gateway A, gateway B — proving bidirectional delivery (request/commit/deliver/ack) with neither gateway's argument vector containing the sibling's listen port anywhere, both task FSMs converging on `COMPLETED`.
- Distinct `RelayFetchError` variants (`Transport`, `MalformedResponse`, `Unauthorized`, `RateLimited`, `AuthBuild`, `BadRecipient`) so an operator can tell a down relay apart from a not-yet-configured key apart from a transient rate limit apart from a malformed response — never a flattened generic reject.

## Task Commits

1. **Task 1: Signed relay-fetch loop routed through the single ingest core** — `2457774` (feat)
2. **Task 2: Wire the fetch loop into main, retire the superseded credential flag, and prove REACH-04 with three processes and no direct peer URL** — `3e8bcfe` (feat)

**Plan metadata:** (this commit)

## Files Created/Modified

- `crates/famp-gateway/src/relay_fetch.rs` — new: `run_relay_fetch`, `poll_once`, `ingest_success_batch`, `decode_fetch_batch`, `RelayFetchError`, `build_fetch_client`, `build_fetch_url`, `classify_fetch_status`, `next_backoff_ms`; 23 unit/integration tests
- `crates/famp-gateway/tests/e2e_relay_bidirectional.rs` — new: the three-process REACH-04 loopback e2e
- `crates/famp-gateway/Cargo.toml` — adds `famp-relay`, `base64`, `serde`, `reqwest` (rustls-no-provider)
- `crates/famp-gateway/src/lib.rs` — `pub mod relay_fetch;`
- `crates/famp-gateway/src/ingress.rs` — `GatewayIngressState` widened to `pub` with a `pub const fn new`; `build_gateway_router`/`run_ingress` take an externally-supplied guard
- `crates/famp-gateway/src/main.rs` — `--relay-fetch` wiring, `run_relay_fetch_branch` extraction, `USAGE` constant, unrecognized-flag rejection, silencers for the four new deps
- `crates/famp-gateway/tests/inbound_destination_validation.rs` — updated `build_gateway_router` call site for the new guard parameter

## Decisions Made

See `key-decisions` in frontmatter above: `GatewayIngressState` widened to `pub` (necessary for cross-crate construction); `run_relay_fetch` gained a `trust_cert` parameter beyond the plan's literal artifact list (necessary for TLS to actually work against the relay); pure `classify_fetch_status`/`next_backoff_ms` extraction for testability; `--relay-token` regression-lock framing (never actually removed, since never added); `e2e_relay_bidirectional.rs` builds its own gateway-argument-vector helper rather than reusing `gateway_harness::spawn_gateway`; one narrow, precedented new lint suppression.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `GatewayIngressState` needed `pub` visibility, not `pub(crate)`, for `main.rs` to construct it**
- **Found during:** Task 1, while designing `run_relay_fetch`'s signature per the plan's artifact list
- **Issue:** The plan's artifact list types `run_relay_fetch`'s last parameter as `GatewayIngressState`, and Task 2's action text has `main.rs` construct one directly to share the SAME guard as `run_ingress`'s router. `main.rs` (the `[[bin]]` target) is a SEPARATE crate from the `famp-gateway` lib target, and a `pub(crate)` type in the lib crate cannot be named or constructed from an external crate — regardless of whether the function that takes it as a parameter is itself `pub`.
- **Fix:** Widened `GatewayIngressState` from `pub(crate)` to `pub`, added a `pub const fn new(registry, keyring, own_domain, guard) -> Self` constructor (the sole way to construct one; the four fields themselves stay private). `build_gateway_router`/`run_ingress` now take an externally-supplied `Arc<Mutex<IngressGuard>>` instead of constructing their own. `ingest_inbound` itself was NOT touched — stays `pub(crate)`, exactly as the plan requires.
- **Files modified:** `crates/famp-gateway/src/ingress.rs`, plus its two internal test call sites and one external test file (`tests/inbound_destination_validation.rs`) that call `build_gateway_router` directly.
- **Verification:** `cargo test -p famp-gateway --lib` (74/74) and `cargo test -p famp-gateway --test inbound_destination_validation` (6/6) both green after the change.
- **Committed in:** `2457774` (Task 1 commit)

**2. [Rule 3 - Blocking] `run_relay_fetch` needed a `trust_cert: Option<PathBuf>` parameter beyond the plan's literal signature**
- **Found during:** Task 1, while implementing the fetch client's TLS construction
- **Issue:** `famp_transport_http::HttpTransport` (the crate's existing outbound HTTPS client) exposes only `Transport::send` (a fixed inbox POST) — no raw-GET-with-custom-headers surface `relay_fetch.rs` could reuse. Building a second `reqwest::Client` with no configured trust cert would trust only the OS root store, and the relay in `e2e_relay_bidirectional.rs` (and any real self-hosted relay) presents a cert that isn't OS-trusted — every fetch would fail TLS verification.
- **Fix:** `run_relay_fetch` takes a `trust_cert: Option<PathBuf>` parameter, sourced in `main.rs` from the ALREADY-EXISTING `--trust-cert` flag (`args.trust_cert.clone()`) — no new CLI flag added. The fetch client is built via `famp_transport_http::tls::build_client_config`, the SAME construction `HttpTransport::new_client_only` uses internally, so there is exactly one TLS-client-building pattern in this crate, not two.
- **Files modified:** `crates/famp-gateway/src/relay_fetch.rs`, `crates/famp-gateway/src/main.rs`
- **Verification:** `e2e_relay_bidirectional.rs`'s live three-process test (both gateways configured with `--trust-cert bob.crt`, the relay's own cert) passes end to end.
- **Committed in:** `2457774` (Task 1), wired in `3e8bcfe` (Task 2)

**3. [Rule 3 - Blocking] `clippy::too_long_first_doc_paragraph`, `clippy::literal_string_with_formatting_args`, `clippy::too_many_lines` (three sites), `clippy::missing_const_for_fn`**
- **Found during:** Task 1 and Task 2's own `cargo clippy -p famp-gateway --all-targets -- -D warnings` runs
- **Issue:** Five distinct lint failures: two doc comments whose first paragraph ran long; `build_fetch_url`'s `"{domain}"` literal misread as a formatting-macro argument; `run_relay_fetch` and (later) `main()` each exceeded the 100-line budget after the new wiring; `GatewayIngressState::new` could be `const`.
- **Fix:** Split the two doc comments into a short first paragraph + a blank-line-separated second paragraph; narrowly `#[allow(clippy::literal_string_with_formatting_args)]`'d `build_fetch_url` with a comment citing the exact same precedent already in `famp-relay/tests/relay_store_and_forward.rs::fetch_url`; extracted `poll_once`/`ingest_success_batch` out of `run_relay_fetch`, and `run_relay_fetch_branch` out of `main()` (mirroring `main.rs`'s own `resolve_own_domain_or_exit`/`build_route_map` extraction precedent); made `GatewayIngressState::new` a `const fn`.
- **Files modified:** `crates/famp-gateway/src/relay_fetch.rs`, `crates/famp-gateway/src/ingress.rs`, `crates/famp-gateway/src/main.rs`
- **Verification:** `cargo clippy -p famp-gateway --all-targets -- -D warnings` clean; `just lint` (workspace-wide, nursery lints) clean.
- **Committed in:** `2457774` (Task 1, most fixes) and `3e8bcfe` (Task 2, the `main()` extraction)

**4. [Rule 2 - Missing coverage] `--relay-token` had no regression lock at all**
- **Found during:** Task 2, reading the plan's acceptance criteria against the actual `parse_args` behavior
- **Issue:** `parse_args`'s fallback arm silently pushed ANY unrecognized `--`-prefixed argument onto the positional-names list — meaning a future accidental reintroduction of `--relay-token` (or any other unknown flag) would parse without complaint, defeating the point of an acceptance criterion asserting it is rejected as unknown.
- **Fix:** Added a `USAGE` constant (shared between the "no positional names" error and the new unknown-flag error) and a match-guard arm rejecting any `other if other.starts_with("--")` with a distinct usage error naming the offending flag, before the general positional-name fallback.
- **Files modified:** `crates/famp-gateway/src/main.rs`
- **Verification:** New test `relay_token_flag_is_rejected_as_unrecognized_never_swallowed_as_a_name` passes; all 24 pre-existing `main.rs` unit tests still pass.
- **Committed in:** `3e8bcfe` (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (2 blocking/Rule 3 direct compile-time/design consequences of building exactly what the plan specified, 1 blocking/Rule 3 clippy, 1 missing-coverage/Rule 2 test gap found while verifying the plan's own acceptance criteria)
**Impact on plan:** All four are necessary, in-scope consequences of implementing this plan's stated design (a shared single-ingest-core relay-fetch client, one guard per process, a real TLS-trusting client, and a real regression lock for the superseded flag). No architectural changes, no Rule 4 escalation needed.

## Issues Encountered

None beyond the deviations above. The `just fmt`/pre-commit fmt-check hook caught genuine formatting drift twice (once per task) — both resolved by running `cargo fmt --all`, confirmed to touch only this plan's own files (`git diff --stat` before/after), and both task commits' tests/clippy re-verified green after formatting.

## User Setup Required

None — no external service configuration required. No Lightsail provisioning happened in this plan (D-04 respected).

## Next Phase Readiness

- REACH-04's loopback half is proven; its genuinely-different-networks half and REACH-02 both stay explicitly open, carried forward per the Phase 10 DOC-04 precedent — Phase 18 must not read either as closed.
- Phase 18's follower-doc author has, verbatim above, the D-25 plaintext-relay disclosure obligation, the D-27 manual-pairing operator step, and the concrete runtime symptom (401/403 vs. 429 vs. transport-error log lines) that step produces.
- `--relay-token` is confirmed absent from the shipped CLI surface, with a regression lock (not just its absence) — a future accidental reintroduction of ANY unrecognized flag will now fail loudly at parse time.
- Layer 0 (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`) confirmed byte-identical: `git diff --name-only` against this plan's base commit shows no path under any of the five frozen crates. `BUS_PROTO_VERSION` unchanged.
- No blockers for Phase 18 from this plan's own work. The only open item this plan could not close is external (Ben's second-network availability for the cross-network REACH-04 leg and REACH-02).

## Known Stubs

None.

## Self-Check: PASSED

- All created/modified files confirmed present on disk: `crates/famp-gateway/src/relay_fetch.rs`, `crates/famp-gateway/tests/e2e_relay_bidirectional.rs`, `crates/famp-gateway/Cargo.toml`, `crates/famp-gateway/src/lib.rs`, `crates/famp-gateway/src/ingress.rs`, `crates/famp-gateway/src/main.rs`, `crates/famp-gateway/tests/inbound_destination_validation.rs`.
- Both task commits confirmed present in `git log`: `2457774`, `3e8bcfe`.
- `cargo test --workspace --no-fail-fast`: 179 `test result:` blocks, 0 failed (confirmed by direct grep against the captured log, not inferred).
- `just lint`, `just check-no-tokio-in-bus`, `just check-quarantine-surfaces`, `just check-shellcheck`: all exit 0, confirmed directly.

---
*Phase: 17-protocol-grade-ingress-reachability-implementation*
*Completed: 2026-08-02*
