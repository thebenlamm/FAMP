---
phase: 01-famp-bus-library-and-audit-log
plan: 03
subsystem: envelope-and-bus
tags: [rust, famp-envelope, famp-bus, audit-log, v0.5.2, atomic-bump, BUS-11, D-09]

requires:
  - phase: 01-famp-bus-library-and-audit-log
    provides: famp-bus protocol primitives, codec, mailbox/liveness test env, pure broker actor with PROP-01..05 GREEN, temporary `Vec<serde_json::Value>` drain shape
provides:
  - `MessageClass::AuditLog` 6th wire variant
  - `AuditLogBody { event, subject?, details? }` schema with `deny_unknown_fields` + `post_decode_validate`
  - `Relation::Audits` causality variant
  - `AnySignedEnvelope::AuditLog` dispatch arm (federation/Layer-1 path)
  - `BusEnvelope<B>` sibling type — private inner, signature-forbidden, two compile_fail doctests (BUS-11)
  - `AnyBusEnvelope` 6-arm dispatch enum on the bus side
  - `EnvelopeDecodeError::UnexpectedSignature` variant routed to `ProtocolErrorKind::Malformed`
  - `FAMP_SPEC_VERSION` flipped `"0.5.1"` → `"0.5.2"` with the T5 lag block deleted
  - vector_1 worked-example fixture (audit_log under v0.5.2) alongside vector_0 v0.5.1 historical
  - Broker drain typed-decoder gate (D-09): each line is type-validated via `AnyBusEnvelope::decode` before populating `RegisterOk.drained`; decode failure emits `BusReply::Err{EnvelopeInvalid}` and aborts cursor advance
  - PROP-04 re-asserted GREEN against typed-decoder invariant; new negative-case test for malformed drain line
  - `just check-spec-version-coherence` recipe wired into `just ci` to prevent split commits in the future

affects: [phase-02-uds-wire, federation-gateway, all-FAMP-conformance-claims]

tech-stack:
  added: []
  patterns: [private-inner type-state for compile-time invariants, sibling envelope types for split federation/bus trust boundaries, atomic-bump invariant via grep-gate]

key-files:
  created:
    - crates/famp-envelope/src/body/audit_log.rs
    - crates/famp-envelope/src/bus.rs
    - crates/famp-envelope/tests/audit_log_red_gate.rs
    - crates/famp-envelope/tests/vector_1_audit_log.rs
    - crates/famp-envelope/tests/vectors/vector_1/envelope.json
    - crates/famp-envelope/tests/vectors/vector_1/canonical.json
    - crates/famp-envelope/tests/vectors/vector_1/keys.json
    - crates/famp-bus/tests/audit_log_dispatch.rs
  modified:
    - Justfile
    - crates/famp-core/src/class.rs
    - crates/famp-envelope/src/body/mod.rs
    - crates/famp-envelope/src/causality.rs
    - crates/famp-envelope/src/dispatch.rs
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/error.rs
    - crates/famp-envelope/src/lib.rs
    - crates/famp-envelope/src/version.rs
    - crates/famp-envelope/tests/provisional_scope_instructions_vector.rs
    - crates/famp-envelope/tests/scope_more_coming_round_trip.rs
    - crates/famp-envelope/tests/vector_zero.rs
    - crates/famp-bus/src/lib.rs
    - crates/famp-bus/src/proto.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp-bus/tests/prop04_drain_completeness.rs
    - crates/famp-bus/tests/tdd02_drain_cursor_order.rs
    - crates/famp/src/runtime/adapter.rs
    - crates/famp-transport-http/src/server.rs

key-decisions:
  - **AUDIT-05 atomic-bump invariant preserved.** The constant flip, the impl that justifies it, and the D-09 broker drain rewiring all land in ONE commit. `just check-spec-version-coherence` is a permanent grep-gate against future regressions of the form "constant says 0.5.2 but `MessageClass::AuditLog` doesn't exist."
  - **D-09 implemented as type-validation-only**, not a wire-shape change. `RegisterOk.drained` stays `Vec<serde_json::Value>` on the wire to preserve BUS-02/03 canonical-JSON round-trip via `serde::Deserialize`. The broker calls `AnyBusEnvelope::decode` against each line FIRST and only inserts the matching `serde_json::Value` if decode succeeds. This satisfies "Broker decodes each line via the bus-side envelope decoder before populating `drained`" without forcing a manual `Deserialize for BusReply::RegisterOk`.
  - **Broker `decode_lines` gates the whole drain.** First decode failure short-circuits the entire register reply with `BusReply::Err{EnvelopeInvalid}` and never emits `Out::AdvanceCursor`, so the bad line is preserved and re-drained on the next register attempt (matches the plan's "ABORT cursor advance" requirement).
  - **Historical fixtures shifted from "decode OK" to "reject with `UnsupportedVersion { found: \"0.5.1\" }`".** Vector_0, the provisional scope vector, and the pre-pc7 fixture all assert the new boundary behavior — the spec mandates this exact rejection string under §19. Vector_1 ships as the new v0.5.2 worked example.
  - **HTTP transport URL path (`/famp/v0.5.1/inbox/...`) intentionally NOT bumped.** Plan scopes the bump to the envelope wire `famp` field. URL versioning is a separate concern owned by the transport binding and tracked outside this plan.
  - **Exhaustive-match fallout absorbed into the atomic commit.** `crates/famp/src/runtime/adapter.rs` (4 match sites: fsm_input_from_envelope, recipient, sender, class) and `crates/famp-transport-http/src/server.rs` (envelope_sender) gained the `AuditLog` arm. `fsm_input_from_envelope` returns `None` for AuditLog, joining `Ack` in the wire-only/non-FSM-firing class — D-15 / Δ31 honored. These files were outside the plan's `files_modified`, but the workspace cannot compile without them, so they ride the same commit.

requirements:
  satisfied:
    - BUS-11: BusEnvelope<B> sibling type with private inner; UnexpectedSignature error variant; AnyBusEnvelope 6-arm dispatch including AuditLog; two compile_fail doctests passing under `cargo test --doc`.
    - AUDIT-01: MessageClass::AuditLog variant + Display + module doc bumped to "six v0.5.2 message classes."
    - AUDIT-02: AuditLogBody { event: String, subject: Option<String>, details: Option<Value> } with deny_unknown_fields and empty-event rejection in post_decode_validate.
    - AUDIT-03: NO change to crates/famp-fsm — audit_log is non-FSM-firing per D-15. Adapter returns None for AuditLog (joining Ack precedent).
    - AUDIT-04: Relation::Audits 6th causality variant.
    - AUDIT-05: ALL changes ship in ONE git commit; commit message cites AUDIT-05 token; `just check-spec-version-coherence` recipe wired into `just ci`.
    - AUDIT-06: T5 doc-comment lag block (lines 7-19 of version.rs at HEAD~1) deleted.
    - D-09: Broker drain decoder type-validates each line via AnyBusEnvelope::decode before populating drained; decode failure short-circuits to BusReply::Err{EnvelopeInvalid} and aborts cursor advance. PROP-04 re-asserted GREEN; negative-case test added.

verification:
  targeted_passing:
    - `cargo build --workspace --all-targets` — clean (0.81s incremental).
    - `cargo fmt --all -- --check` — clean.
    - `cargo clippy -p famp-envelope -p famp-bus -p famp -p famp-transport-http --all-targets --no-deps -- -D warnings` — clean.
    - `cargo nextest run -p famp-envelope --test audit_log_red_gate` — 1 passed (pure-Rust RED→GREEN).
    - `cargo nextest run -p famp-envelope --test vector_1_audit_log` — 1 passed (fixture-based RED→GREEN).
    - `cargo nextest run -p famp-bus --test audit_log_dispatch` — 2 passed.
    - `cargo nextest run -p famp-bus --test prop04_drain_completeness` — 2 passed (positive proptest + new malformed-line negative case).
    - `cargo test --doc -p famp-envelope` — 6 doctests passed (incl. both BusEnvelope BUS-11 compile_fail gates).
    - `cargo nextest run -p famp --test await_commit_advance_error_surfaces` — 1 passed (synthetic inline `"famp": "0.5.1"` fixture survives because `poll.rs::find_match` does not validate the version field).
    - `just check-spec-version-coherence` — exit 0.
    - `just check-no-tokio-in-bus` — exit 0 (BUS-01 contract intact).

  blocked:
    - Full `cargo nextest run --workspace` and `just ci` are blocked by 8 pre-existing test failures in `crates/famp/tests/` and `crates/famp-transport-http/tests/`. Failure mode is uniformly `reqwest::Error { kind: Request, source: TimedOut }` against TLS loopback URLs (`https://127.0.0.1:.../famp/v0.5.1/inbox/...`). The same 8 tests fail identically when stashing all Wave 3 changes and re-running on Wave 2 commit `ae905ed` — so this is a pre-existing local-environment issue (likely macOS rustls-platform-verifier vs. ephemeral TLS cert + loopback) and **not a Wave 3 regression**. Affected tests:
      - famp::listen_smoke smoke_post_delivers_to_inbox
      - famp::listen_multi_peer_keyring accepts_envelope_from_self
      - famp::listen_multi_peer_keyring accepts_envelope_from_registered_peer
      - famp::listen_multi_peer_keyring rejects_envelope_from_unknown_principal
      - famp::listen_durability sigkill_after_200_leaves_line_intact
      - famp::http_happy_path http_happy_path_same_process
      - famp::await_blocks_until_message await_blocks_until_message_arrives
      - famp::example_happy_path personal_two_agents_exits_zero_with_expected_trace (120s timeout)
    - Recommendation: triage these listener/E2E timeouts as a separate hygiene phase; they are not gated by AUDIT-05.

red_green_trace:
  pure_rust_gate:
    file: crates/famp-envelope/tests/audit_log_red_gate.rs
    pre_commit_state: would fail to compile at HEAD~1 (no `MessageClass::AuditLog` variant; FAMP_SPEC_VERSION = "0.5.1"). Captured in commit message body.
    post_commit_state: GREEN — `cargo nextest run -p famp-envelope --test audit_log_red_gate` exits 0.
  fixture_gate:
    file: crates/famp-envelope/tests/vector_1_audit_log.rs
    pre_commit_state: would fail at HEAD~1 — both the fixture file and the AnySignedEnvelope::AuditLog dispatch arm absent.
    post_commit_state: GREEN — vector_1 envelope.json decodes via AnySignedEnvelope, dispatches to AuditLog arm, body event/subject round-trip exactly.

d09_evidence:
  proto.rs: D-09 invariant comment block above RegisterOk explains why drained stays `Vec<serde_json::Value>` (BUS-02/03 round-trip) but is type-validated via AnyBusEnvelope::decode at the broker handler.
  broker/handle.rs decode_lines: each line is run through `famp_envelope::AnyBusEnvelope::decode` BEFORE the existing `from_slice_strict::<serde_json::Value>` reparse. Decode failure propagates a String error that the Register handler maps to `BusReply::Err{EnvelopeInvalid}`, short-circuiting before any `Out::AdvanceCursor` is emitted.
  prop04_drain_completeness: the proptest now sends actual canonical `audit_log` envelopes (not arbitrary `{offline_seq: N}` JSON) so the typed-decoder gate accepts them. After Register, every drained Value is re-parsed through `AnyBusEnvelope::decode` and asserted to dispatch to the AuditLog arm.
  prop04 negative case: `malformed_drain_line_returns_error_and_does_not_advance_cursor` injects `b"{not json"` into alice's mailbox, runs Register, asserts `Out::Reply(_, BusReply::Err{kind: EnvelopeInvalid, ..})` is the only output and `Out::AdvanceCursor` is absent.

bus11_evidence:
  type_level: `BusEnvelope<B>` has a private `inner: WireEnvelope<B>` field; no public constructor; `from_wire` is `pub(crate)`. Federation handlers typed for `SignedEnvelope<B>` cannot accept a `BusEnvelope<B>` (the two types do not unify). Both invariants are enforced as `compile_fail` doctests in `crates/famp-envelope/src/bus.rs`.
  runtime_level: `BusEnvelope::decode` checks for the `signature` key BEFORE typed deserialization and returns `EnvelopeDecodeError::UnexpectedSignature` if present. Routed to `ProtocolErrorKind::Malformed` via the existing exhaustive match in `From<EnvelopeDecodeError> for ProtocolError`.

caveats:
  - The `crates/famp-envelope/tests/adversarial.rs` " 0.5.1" / "0.5.1\n" tests are intentional whitespace-rejection tests (the field carries the bad string verbatim), unrelated to the constant flip — left untouched.
  - `crates/famp/src/cli/await_cmd/poll.rs:9` still mentions `"0.5.1"` in a doc-comment example. `poll.rs` does NOT validate the version field, so this is stale prose, not active drift. Out of plan scope.
  - HTTP URL path `/famp/v0.5.1/inbox/{principal}` is unchanged — plan scopes only the envelope `famp` field bump; URL versioning is a separate transport concern.

deferred_followups:
  - Triage the 8 pre-existing listener/E2E timeouts as a hygiene task. Likely candidates: rustls-platform-verifier behavior on macOS 25.3 + loopback, ephemeral TLS cert stapling, or reqwest::default_pool keepalive interaction.
  - Bump HTTP transport URL `/famp/v0.5.1/...` → `/famp/v0.5.2/...` in a future plan if/when transport-version drift is addressed (not required for AUDIT-05 conformance).
  - Update stale `"0.5.1"` doc-comment in `crates/famp/src/cli/await_cmd/poll.rs:9` in a future docs sweep.

next_phase_handoff:
  - Phase 1 (famp-bus library + audit_log) is now WAVE-COMPLETE. All three plans (01-01 scaffold, 01-02 broker actor, 01-03 audit_log + v0.5.2 atomic bump) shipped.
  - Phase 2 (UDS wire + transport) inherits a stable v0.5.2 envelope surface, the `AnyBusEnvelope` typed dispatcher, and a broker drain handler that already type-validates lines. The `serde_json::Value` shape on `RegisterOk.drained` is preserved for BUS-02/03 round-trip; consumers wanting typed access can call `AnyBusEnvelope::decode` per line.
