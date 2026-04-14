---
phase: 01-minimal-signed-envelope
plan: 01
subsystem: famp-envelope
tags: [envelope, primitives, error-enum, vector-0, fixtures]
requires:
  - famp-canonical (CanonicalError)
  - famp-crypto (CryptoError)
  - famp-core (ProtocolError, ProtocolErrorKind)
provides:
  - famp_envelope::MessageClass (5 variants, snake_case, sealed)
  - famp_envelope::EnvelopeScope (3 variants, snake_case)
  - famp_envelope::FampVersion (literal "0.5.1" newtype)
  - famp_envelope::Timestamp (byte-preserving RFC 3339 newtype)
  - famp_envelope::EnvelopeDecodeError (18-variant phase-local enum)
  - tests/vectors/vector_0/ §7.1c byte-exact fixtures
affects:
  - Plans 01-02 and 01-03 (import without further edits to these files)
tech_stack:
  added: []
  patterns:
    - hand-written Serialize/Deserialize for literal-string wire values
    - byte-preserving newtype (no time crate parse path) — PITFALL P6
    - phase-local narrow error enum → exhaustive map to ProtocolError (no _ arm)
key_files:
  created:
    - crates/famp-envelope/src/class.rs
    - crates/famp-envelope/src/scope.rs
    - crates/famp-envelope/src/version.rs
    - crates/famp-envelope/src/timestamp.rs
    - crates/famp-envelope/src/wire.rs
    - crates/famp-envelope/src/error.rs
    - crates/famp-envelope/tests/smoke.rs
    - crates/famp-envelope/tests/errors.rs
    - crates/famp-envelope/tests/vectors/vector_0/envelope.json
    - crates/famp-envelope/tests/vectors/vector_0/canonical.hex
    - crates/famp-envelope/tests/vectors/vector_0/signing_input.hex
    - crates/famp-envelope/tests/vectors/vector_0/signature.hex
    - crates/famp-envelope/tests/vectors/vector_0/signature.b64url
  modified:
    - Cargo.toml (workspace deps: hex 0.4)
    - crates/famp-envelope/Cargo.toml
    - crates/famp-envelope/src/lib.rs
decisions:
  - UnsupportedVersion routes to ProtocolErrorKind::Unsupported (actual famp-core variant; plan suggested non-existent UnsupportedVersion name)
  - Dev-dep silencing via `use X as _;` pattern borrowed verbatim from famp-crypto/src/lib.rs
metrics:
  completed_date: 2026-04-13
  tasks: 3
  commits: 3
  tests: 10 (5 smoke + 5 errors)
---

# Phase 1 Plan 01: Envelope Primitives + Vector 0 Fixtures Summary

One-liner: Scaffolds `famp-envelope` with its full Phase 1 dep set, lands the
byte-exact §7.1c worked-example vector 0 fixtures, and ships the primitive
types (`MessageClass`, `EnvelopeScope`, `FampVersion`, `Timestamp`) plus the
18-variant `EnvelopeDecodeError` skeleton that every subsequent task in Plans
02 and 03 will import without modification.

## What Shipped

### Task 1 — Cargo wiring + vector 0 fixtures (commit `5a83730`)
- `crates/famp-envelope/Cargo.toml`: added `serde`, `serde_json`, `thiserror`,
  `famp-canonical`, `famp-crypto`, `famp-core` as deps, and `proptest`,
  `insta` (json feature), `hex` as dev-deps. No direct deps on `serde_jcs`,
  `ed25519-dalek`, `base64`, or `uuid` — envelope code reaches all of those
  through the three workspace `famp-*` crates per CONTEXT.md.
- Root `Cargo.toml`: added `hex = "0.4"` to `[workspace.dependencies]`.
- `crates/famp-envelope/tests/vectors/vector_0/`:
  - `envelope.json` — §7.1c.7 signed wire envelope verbatim (ack, standalone,
    `disposition: accepted`, with `signature` field).
  - `canonical.hex` — 324-byte RFC 8785 canonical JSON as lowercase hex,
    reconstructed verbatim from §7.1c.3 and programmatically verified
    byte-identical against the spec's own plaintext canonical string.
  - `signing_input.hex` — 336 bytes = 12-byte `FAMP-sig-v1\x00` prefix
    prepended to `canonical.hex`, per §7.1c.5.
  - `signature.hex` — 64-byte raw Ed25519 signature from §7.1c.6.
  - `signature.b64url` — unpadded base64url form from §7.1c.6,
    `k2aqzthUx4mHNZCNLi2XMgiQX9gOL5P-UFcQ9Y8O0fyS47nXoZswss8YT3A1Utr8-RyoEyH1f6aJ0aloZdC2CA`.

Per PITFALLS P10, none of these bytes were self-generated. `canonical.hex`
was assembled from the spec's hex listing in §7.1c.3 and cross-checked
against the spec's plaintext canonical string; `signing_input.hex` is
`prefix || canonical`; `signature.hex` and `signature.b64url` are copied
literal from §7.1c.6.

### Task 2 — Primitive types + smoke tests (commit `2252194`)
- `src/class.rs`: `MessageClass { Request, Commit, Deliver, Ack, Control }`
  with `#[serde(rename_all = "snake_case", deny_unknown_fields)]` and a
  manual `Display` impl (snake_case wire strings) for error formatting.
- `src/scope.rs`: `EnvelopeScope { Standalone, Conversation, Task }` — same
  attribute set, same Display pattern.
- `src/version.rs`: `FampVersion` unit struct with hand-written
  `Serialize`/`Deserialize` that only ever accepts / emits the literal
  string `"0.5.1"`. Also exports `FAMP_SPEC_VERSION: &str`.
- `src/timestamp.rs`: `Timestamp(pub String)` newtype. Deserialize runs a
  shallow format check (length, `-`/`T`/`:` positions, trailing `Z` or
  `±HH:MM` offset) but never parses through `time::OffsetDateTime`.
  Serialize passes bytes through verbatim. PITFALL P6 documented inline.
- `src/wire.rs`: private `pub(crate)` module with just
  `SIGNATURE_FIELD: &str = "signature"`, allowed dead code, Plan 03 fills
  in the full `WireEnvelope` struct.
- `src/lib.rs`: replaced the Phase 0 stub with the module tree and the
  CRITICAL warning against ever refactoring to `#[serde(flatten)]` or
  `#[serde(tag = ...)]` (per RESEARCH.md P1/P2 + CONTEXT.md D-B5).
- `tests/smoke.rs`: five smoke tests — `version_literal_roundtrip`,
  `version_rejects_wrong_literal`, `message_class_snake_case_roundtrip`,
  `envelope_scope_snake_case_roundtrip`, `timestamp_preserves_bytes`.
  All five green on first run.

### Task 3 — EnvelopeDecodeError skeleton (commit `006fffe`)
- `src/error.rs`: replaced the Task 2 placeholder with the full 18-variant
  enum. Active variants (Plans 01 and 03 reach these now or in the next
  plan): `MalformedJson` (`#[from] CanonicalError`), `MissingField`,
  `UnknownEnvelopeField`, `UnsupportedVersion`, `UnknownClass`,
  `ClassMismatch`, `ScopeMismatch`, `MissingSignature`,
  `InvalidSignatureEncoding` (`#[from] CryptoError`), `SignatureInvalid`.
  Body-level variants shipped now so Plan 02 does not touch `error.rs`:
  `UnknownBodyField`, `InvalidControlAction`, `InterimWithTerminalStatus`,
  `TerminalWithoutStatus`, `MissingErrorDetail`, `MissingProvenance`,
  `InsufficientBounds`, `BodyValidation`.
- `From<EnvelopeDecodeError> for ProtocolError`: compile-time exhaustive
  `match` with no `_ =>` arm. Routes:
  - `MissingSignature | InvalidSignatureEncoding(_) | SignatureInvalid`
    → `ProtocolErrorKind::Unauthorized`
  - `UnsupportedVersion { .. }` → `ProtocolErrorKind::Unsupported`
  - every other variant → `ProtocolErrorKind::Malformed`
  - **never** `ProtocolErrorKind::Other` (and the enum has no `Other`
    variant anyway — sanity check grep confirms it is not referenced).
- `tests/errors.rs`: five tests — mapping spot checks plus a
  "no variant falls through" test that constructs every variant and asserts
  the resulting `ProtocolErrorKind` is one of the three sanctioned values.

## Verification Results

- `cargo test -p famp-envelope`: **10 / 10 green** (5 smoke + 5 errors).
- `cargo clippy -p famp-envelope --all-targets -- -D warnings`: clean.
- `cargo check --workspace`: clean (no downstream regressions).
- Vector 0 `canonical.hex` verified byte-identical (324 bytes) against
  spec's plaintext canonical JSON via an independent Python decode.
- `signature.b64url` byte-length 86 chars as required by §7.1b; hex file
  128 chars + newline (64-byte raw signature).
- `rg 'serde\(flatten\)|serde\(tag' crates/famp-envelope/src` → 0 hits.
- `rg 'ProtocolErrorKind::Other' crates/famp-envelope` → 0 hits.

## Deviations from Plan

### [Rule 3 — Unblocking] UnsupportedVersion mapping target renamed

- **Found during:** Task 3 (wiring the `From` impl).
- **Issue:** Plan 01-01 Task 3 specified mapping `UnsupportedVersion { .. }`
  to `ProtocolErrorKind::UnsupportedVersion`. That variant does not exist
  in `famp-core/src/error.rs` — the 15-category §15.1 enum has
  `Unsupported` (single word, no suffix).
- **Fix:** Mapped to `ProtocolErrorKind::Unsupported` instead. Consistent
  with the `snake_case` wire string `"unsupported"` that the famp-core
  integration test `error_wire_strings` already locks.
- **Files modified:** `crates/famp-envelope/src/error.rs`,
  `crates/famp-envelope/tests/errors.rs`
- **Commit:** `006fffe`

### [Rule 3 — Unblocking] Dev-dep silencing via `use X as _;`

- **Found during:** Task 3 clippy pass with `-D warnings`.
- **Issue:** The workspace lint `unused_crate_dependencies = "warn"`
  escalates under `-D warnings`. Lib compile unit flagged `serde_json`
  (only used by tests). Integration tests each compile as their own
  crate, so `tests/smoke.rs` and `tests/errors.rs` also needed to
  acknowledge every dev-dep — including deps pulled in transitively via
  the `[dev-dependencies]` block (proptest, insta, hex) — even though
  the Plan 01-01 tests don't reach for them yet.
- **Fix:** Borrowed the exact `use X as _;` idiom already in use in
  `crates/famp-crypto/src/lib.rs` and `crates/famp-core/src/lib.rs`.
  Added `#[cfg(test)] use hex as _;` etc. in `src/lib.rs` and
  corresponding `use X as _;` lines in both integration test files.
- **Files modified:** `crates/famp-envelope/src/lib.rs`,
  `crates/famp-envelope/tests/smoke.rs`,
  `crates/famp-envelope/tests/errors.rs`
- **Commit:** `006fffe`

### [Rule 3 — Unblocking] Clippy pedantic cleanups

Surfaced by Task 3 `-D warnings` run — none are behavioral changes:
- `doc_markdown`: added backticks around `snake_case` in `class.rs` and
  `scope.rs` module docstrings.
- `elidable_lifetime_names`: elided `'de` to `'_` on the private
  `Visitor` impls in `version.rs` and `timestamp.rs`.
- `redundant_pub_crate`: added `#[allow(clippy::redundant_pub_crate)]`
  to `wire::SIGNATURE_FIELD` (cannot be `pub` per CONTEXT.md D-A3).

All landed in commit `006fffe`.

## Must-Haves Verification

- [x] `famp-envelope` crate builds with full Phase 1 dep set (serde,
  serde_json, thiserror, famp-canonical, famp-crypto, famp-core).
- [x] `MessageClass`, `EnvelopeScope`, `FampVersion`, `Timestamp` all
  deserialize per v0.5.1 §7.1c (snake_case classes, literal `"0.5.1"`,
  opaque byte-preserving timestamps) — asserted by smoke.rs.
- [x] `EnvelopeDecodeError` skeleton compiles with all wire-level and
  signature variants shipped now; Plans 02/03 will not touch error.rs
  again.
- [x] §7.1c worked-example vector 0 committed byte-exact as on-disk
  fixtures.
- [x] `cargo nextest`/`cargo test` green, clippy green, workspace clean.
- [x] No `serde(flatten)`, no `serde(tag`, no `ProtocolErrorKind::Other`.

## Handoff Notes for Plans 01-02 and 01-03

- Import `MessageClass`, `EnvelopeScope`, `FampVersion`, `FAMP_SPEC_VERSION`,
  `Timestamp`, and `EnvelopeDecodeError` from the crate root (`pub use`).
- `wire::SIGNATURE_FIELD` is available at `pub(crate)` scope for the Plan 03
  decode path (module already declared `pub(crate) mod wire`).
- All body-level `EnvelopeDecodeError` variants exist now; Plan 02 only
  needs to construct them from new call sites — no `error.rs` edits.
- Vector 0 fixture loader for Plan 03: `include_bytes!` /
  `include_str!("../tests/vectors/vector_0/canonical.hex")` etc. Files are
  committed at byte-exact sizes (324 canonical, 336 signing-input, 64 raw
  signature, 86-char b64url).
- Dev-dep discipline: any new integration test file must add the same
  `use X as _;` block that `tests/smoke.rs` and `tests/errors.rs` carry,
  or relax `unused_crate_dependencies` at crate root.

## Self-Check: PASSED

All created files verified present on disk; all three commit hashes
(`5a83730`, `2252194`, `006fffe`) verified in `git log`.
