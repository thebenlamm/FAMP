//! The single spec-version string FAMP v0.5.1 uses on the wire.
//!
//! The version-rejection gate lives in `envelope::SignedEnvelope::decode_value`
//! (see PR #2 / spec §Δ01 §19): a tampered `famp` field produces
//! `EnvelopeDecodeError::UnsupportedVersion` → `ProtocolErrorKind::Unsupported`.
//! There is exactly one such site in the decode path.
//!
//! # Spec lag (T5, 2026-04-27)
//!
//! `FAMP-v0.5.1-spec.md` was amended to v0.5.2 in commit `f44f3ee`
//! (audit_log MessageClass; see spec §8a.6, §7.3a, §19 Δ29–Δ33). This
//! constant intentionally remains at `"0.5.1"` until v0.9 Phase 1 ships
//! the `MessageClass::AuditLog` impl, body validation, and inbox storage
//! path. Bumping the constant before the impl lands would declare false
//! conformance on every signed envelope, since v0.5.2 conformance
//! REQUIRES receiver behavior we don't yet implement (durable storage,
//! no-ack response, "audits" causality acceptance — all per §8a.6).
//! The amendment's spec text includes a non-normative
//! reference-implementation note acknowledging this gap.

pub const FAMP_SPEC_VERSION: &str = "0.5.1";
