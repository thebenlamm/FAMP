//! Pre-verify ingress guard chain: cheap, stateful checks that run BEFORE
//! [`crate::verify::verify_inbound_any`] (INGR-01, INGR-05; D-05, D-09).
//!
//! Three things this module promises that [`crate::verify`] does not:
//!
//! 1. Every field this module reads is PEEKED, not verified.
//!    [`peek_guard_fields`] performs a raw, duplicate-key-rejecting parse
//!    of the wire bytes — cryptographically UNVERIFIED — and every
//!    downstream check in this module (starting with [`freshness_check`])
//!    acts on those peeked values BEFORE
//!    [`crate::verify::verify_inbound_any`] ever confirms a signature. A
//!    caller must never treat [`PeekedFields`] as trusted.
//! 2. Unlike `verify.rs` (documented "Pure, transport-agnostic" with no
//!    owned state), this module owns mutable state: [`IngressGuard`] is
//!    the process-lifetime anchor plan 02 extends with a replay cache and
//!    plan 03 extends with rate-limit buckets. That owned state is the
//!    reason this lives in its own module rather than as an addition to
//!    `verify.rs`'s stated purity contract.
//! 3. INGR-05's ordering guarantee: cheap gates (this module, via
//!    [`run_cheap_gates`]) precede signature verification
//!    (`verify_inbound_any`), and signature verification precedes ANY
//!    registry/local-bus mutation. Getting this backwards turns signature
//!    verification itself into the DoS amplifier (D-09) — a cheap-to-send
//!    request must never be able to force an expensive Ed25519 verify.
//!
//! [`peek_guard_fields`] is a gateway-local raw parse (D-13/D-15): it does
//! NOT add an accessor to `famp-envelope`'s frozen `EnvelopeView`, which
//! exposes only `from`/`to`/`class`/`body`/`task_id` and has no `ts`/
//! `nonce`/`expiry` accessor. Duplicating the parse here, scoped to this
//! crate, keeps `famp-envelope` byte-identical for the milestone (D-15).

use std::str::FromStr;

use famp::Principal;
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// INGR-01's clock-skew window, in seconds.
///
/// An envelope `ts` more than this many seconds from this gateway's own
/// wall clock — in EITHER direction — is rejected. Enforced symmetrically
/// (past and future) and INCLUSIVE at the boundary (exactly this many
/// seconds is still accepted; one second beyond is not).
///
/// This is a DIFFERENT knob from `egress.rs`'s `EXPIRY_WINDOW_MINUTES`:
/// that constant only formats an outbound, format-validated-only
/// `expiry` field and enforces nothing on receipt. `CLOCK_SKEW_WINDOW_SECS`
/// is the one INGR-01 window this gateway actually enforces against every
/// inbound envelope.
pub const CLOCK_SKEW_WINDOW_SECS: i64 = 300;

/// Rejections this module's cheap, pre-verify checks can produce.
///
/// Extended by plans 02 (replay) and 03 (audience/rate-limit) — this plan
/// defines exactly the two variants [`freshness_check`] and
/// [`peek_guard_fields`] need.
#[derive(Debug, thiserror::Error)]
pub enum GuardReject {
    /// The pre-verify raw peek of `from`/`to`/`ts` failed: malformed
    /// JSON, a duplicate object key (rejected by the same strict-parse
    /// contract `famp_envelope::peek_sender` uses), or a missing/
    /// non-string `from`/`to`/`ts`/`nonce`/`expiry` field. Never implies
    /// anything about whether a valid signature is present — this is a
    /// structural check, not a trust decision, so a perfectly-signed
    /// envelope with a malformed shape at this layer maps here too.
    /// Carries no data: the shape itself is malformed, so there is
    /// nothing safe to echo back.
    #[error(
        "envelope failed pre-verify shape check (malformed JSON, duplicate key, or missing/invalid from/to/ts)"
    )]
    BadEnvelopeShape,

    /// INGR-01: the envelope's peeked `ts` differs from this gateway's
    /// own wall-clock `now` by more than `skew_secs` seconds, in either
    /// direction. Does NOT imply the envelope is unsigned, forged, or
    /// otherwise invalid — a correctly-signed envelope with a stale `ts`
    /// maps here too (that is the entire point: this check runs BEFORE
    /// signature verification, per INGR-05/D-09). Carries only the two
    /// timestamps and the configured window size — never key bytes,
    /// signature material, or envelope body content.
    #[error("envelope timestamp {ts} is outside the {skew_secs}s clock-skew window of gateway now {now}")]
    StaleTimestamp {
        ts: String,
        now: String,
        skew_secs: i64,
    },
}

/// Fields peeked from unverified inbound bytes, cheaply, before any
/// signature check runs.
///
/// **Every field here is cryptographically UNVERIFIED.** A caller must
/// never treat `PeekedFields.from` (or any other field) as the confirmed
/// sender — that confirmation only exists after
/// [`crate::verify::verify_inbound_any`] returns `Ok`. Name variables
/// derived from this type accordingly (e.g. `peeked_from`, never just
/// `from`, at any call site that also has a verified value in scope).
#[derive(Debug)]
pub struct PeekedFields {
    pub from: Principal,
    pub to: Principal,
    pub ts: String,
    pub nonce: Option<String>,
    pub expiry: Option<String>,
}

/// Peek `from`/`to`/`ts`/`nonce`/`expiry` out of raw, unverified inbound
/// bytes.
///
/// D-13/D-15: this is a deliberate, gateway-local duplicate of a strict
/// parse — NOT an extension of `famp-envelope`'s frozen `EnvelopeView`,
/// which exposes only `from`/`to`/`class`/`body`/`task_id` and has no
/// `ts`/`nonce`/`expiry` accessor. Extending the frozen crate (even with
/// a read-only accessor) would violate D-15; this duplication is the
/// cost of keeping that crate byte-identical. Uses
/// `famp::from_slice_strict` (`famp-canonical`'s duplicate-key-rejecting
/// parse, the same one `famp_envelope::peek_sender` builds on) so a
/// duplicate `from` key is rejected here exactly as it would be
/// post-verify.
pub fn peek_guard_fields(bytes: &[u8]) -> Result<PeekedFields, GuardReject> {
    let value: Value = famp::from_slice_strict(bytes).map_err(|_| GuardReject::BadEnvelopeShape)?;
    let obj: &Map<String, Value> = value.as_object().ok_or(GuardReject::BadEnvelopeShape)?;

    let from = required_principal(obj, "from")?;
    let to = required_principal(obj, "to")?;
    let ts = required_string(obj, "ts")?;
    let nonce = optional_string(obj, "nonce")?;
    let expiry = optional_string(obj, "expiry")?;

    Ok(PeekedFields {
        from,
        to,
        ts,
        nonce,
        expiry,
    })
}

fn required_string(obj: &Map<String, Value>, field: &'static str) -> Result<String, GuardReject> {
    obj.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(GuardReject::BadEnvelopeShape)
}

fn required_principal(
    obj: &Map<String, Value>,
    field: &'static str,
) -> Result<Principal, GuardReject> {
    let raw = required_string(obj, field)?;
    Principal::from_str(&raw).map_err(|_| GuardReject::BadEnvelopeShape)
}

fn optional_string(
    obj: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, GuardReject> {
    match obj.get(field) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(GuardReject::BadEnvelopeShape),
    }
}

/// INGR-01/D-05: reject an envelope whose peeked `ts` is more than
/// `skew_secs` seconds from `now`, in either direction, with the
/// boundary itself (exactly `skew_secs`) still accepted.
///
/// Both `ts` and `now` are parsed with `time::OffsetDateTime::parse`
/// against RFC 3339 — NEVER compared lexically as strings. REL-02
/// (`federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders`,
/// `famp-envelope/src/envelope.rs`) is the precedent this guards against:
/// a non-canonical-but-valid RFC 3339 offset form can lexically misorder
/// against a later UTC instant, so only a parsed-instant comparison is
/// trustworthy here.
///
/// A parse failure on `ts` maps to `BadEnvelopeShape` (the envelope
/// itself is malformed). A parse failure on `now` ALSO maps to
/// `BadEnvelopeShape` — fail closed — even though `now` is expected to
/// always be a `crate::clock::now_canonical_utc()` output and so cannot
/// legitimately fail; there is no reject variant more honest than
/// "something about this check's inputs is malformed."
pub fn freshness_check(ts: &str, now: &str, skew_secs: i64) -> Result<(), GuardReject> {
    let ts_instant =
        OffsetDateTime::parse(ts, &Rfc3339).map_err(|_| GuardReject::BadEnvelopeShape)?;
    let now_instant =
        OffsetDateTime::parse(now, &Rfc3339).map_err(|_| GuardReject::BadEnvelopeShape)?;

    let diff_secs = (now_instant - ts_instant).whole_seconds().abs();
    if diff_secs <= skew_secs {
        Ok(())
    } else {
        Err(GuardReject::StaleTimestamp {
            ts: ts.to_string(),
            now: now.to_string(),
            skew_secs,
        })
    }
}

/// Process-lifetime pre-verify guard state.
///
/// This plan's only field is `started_at`, the anchor plan 02 uses to
/// bound and document INGR-03's restart-reopened replay window (a fresh
/// `IngressGuard` — and therefore a fresh, empty replay cache — is
/// exactly what a gateway restart produces). Constructed once per
/// gateway process (`build_gateway_router`) and threaded through every
/// inbound request via `GatewayIngressState`.
pub struct IngressGuard {
    started_at: OffsetDateTime,
}

impl IngressGuard {
    /// Capture the current instant as this guard's `started_at`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: OffsetDateTime::now_utc(),
        }
    }

    /// The instant this guard (and therefore this gateway process's
    /// pre-verify guard state) was constructed.
    #[must_use]
    pub const fn started_at(&self) -> OffsetDateTime {
        self.started_at
    }
}

impl Default for IngressGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Input to [`run_cheap_gates`]: the peeked (unverified) fields plus the
/// process-level context every cheap gate needs.
///
/// `own_domain` and `sender_is_backed` are populated by this plan's
/// caller but not yet read by [`run_cheap_gates`] itself — plan 03's
/// audience gate is the first consumer. This is an intentional forward
/// shape (the whole-request context a gate chain needs is assembled
/// once, at the call site, rather than re-derived per gate); do not
/// delete these fields merely because this plan's `run_cheap_gates` body
/// does not yet read them.
pub struct GuardInput<'a> {
    pub peeked: &'a PeekedFields,
    pub now: &'a str,
    pub own_domain: Option<&'a str>,
    pub sender_is_backed: bool,
}

/// The single ordered call site every cheap gate is added to.
///
/// This plan runs exactly one gate: [`freshness_check`]. The required
/// order later plans MUST preserve when adding their own gate: audience
/// binding, then freshness, then replay, then rate limit — matching the
/// order 17-CONTEXT.md's D-08/D-05/D-06/D-10 discuss them. Returns on the
/// FIRST reject; later gates never run once an earlier one has failed
/// (INGR-05: cheap-before-expensive applies within this chain too, not
/// just relative to signature verification).
///
/// `guard` is `&mut` even though this plan's body never mutates it — the
/// signature is intentionally forward-shaped for plan 02's replay cache,
/// which lives on `IngressGuard` and must be checked/updated from this
/// same call site. `#[allow(...)]` below is scoped to this one fact, not
/// a blanket suppression.
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn run_cheap_gates(
    input: &GuardInput<'_>,
    _guard: &mut IngressGuard,
) -> Result<(), GuardReject> {
    freshness_check(input.peeked.ts.as_str(), input.now, CLOCK_SKEW_WINDOW_SECS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn to_canonical_rfc3339(instant: OffsetDateTime) -> String {
        crate::clock::strip_subseconds(&instant.format(&Rfc3339).unwrap())
    }

    #[test]
    fn freshness_check_ts_equal_now_is_ok() {
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        assert!(freshness_check(&now, &now, CLOCK_SKEW_WINDOW_SECS).is_ok());
    }

    #[test]
    fn freshness_check_boundary_is_inclusive_in_both_directions() {
        // Every timestamp below is derived from the SAME `base` instant so
        // the deltas are exact, whole-second differences with zero clock
        // drift between calls (no repeated `OffsetDateTime::now_utc()`
        // calls that could straddle a second boundary).
        let base = OffsetDateTime::now_utc();
        let now = to_canonical_rfc3339(base);

        let past_boundary =
            to_canonical_rfc3339(base - time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS));
        assert!(
            freshness_check(&past_boundary, &now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "exactly -{CLOCK_SKEW_WINDOW_SECS}s must be accepted (inclusive lower bound)"
        );

        let future_boundary =
            to_canonical_rfc3339(base + time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS));
        assert!(
            freshness_check(&future_boundary, &now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "exactly +{CLOCK_SKEW_WINDOW_SECS}s must be accepted (inclusive upper bound)"
        );

        let past_beyond =
            to_canonical_rfc3339(base - time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS + 1));
        assert!(
            matches!(
                freshness_check(&past_beyond, &now, CLOCK_SKEW_WINDOW_SECS),
                Err(GuardReject::StaleTimestamp { .. })
            ),
            "one second beyond the past boundary must be rejected"
        );

        let future_beyond =
            to_canonical_rfc3339(base + time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS + 1));
        assert!(
            matches!(
                freshness_check(&future_beyond, &now, CLOCK_SKEW_WINDOW_SECS),
                Err(GuardReject::StaleTimestamp { .. })
            ),
            "one second beyond the future boundary must be rejected"
        );
    }

    #[test]
    fn freshness_check_rejects_unparseable_ts_as_bad_envelope_shape() {
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        let result = freshness_check("not-a-timestamp", &now, CLOCK_SKEW_WINDOW_SECS);
        assert!(matches!(result, Err(GuardReject::BadEnvelopeShape)));
    }

    /// REL-02 precedent
    /// (`federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders`,
    /// `famp-envelope/src/envelope.rs`): a non-canonical, valid RFC 3339
    /// offset form can represent an instant that a naive string/date
    /// comparison would misjudge. Here `ts`
    /// ("2026-07-26T23:00:00-01:00", i.e. local 23:00 one hour BEHIND
    /// UTC) is the SAME UTC instant as `now` ("2026-07-27T00:00:00Z") —
    /// diff 0 — but its leading date component ("...26T23...") lexically
    /// sorts as an entire calendar day earlier than `now`'s
    /// ("...27T00..."). Only a parsed-instant comparison gets this right.
    #[test]
    fn freshness_check_uses_parsed_instant_not_lexical_order() {
        let now = "2026-07-27T00:00:00Z";
        let ts = "2026-07-26T23:00:00-01:00";
        assert!(
            ts < now,
            "the offset form must actually lexically sort before `now` for this to be a real test"
        );
        assert!(
            freshness_check(ts, now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "ts and now are the SAME UTC instant once parsed -- must not be treated as \
             stale merely because the raw strings misorder"
        );
    }

    #[test]
    fn peek_guard_fields_rejects_duplicate_from_key() {
        let bytes = br#"{"from":"agent:local/alice","from":"agent:local/eve","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z"}"#;
        let result = peek_guard_fields(bytes);
        assert!(matches!(result, Err(GuardReject::BadEnvelopeShape)));
    }

    #[test]
    fn peek_guard_fields_extracts_all_fields_on_well_formed_envelope() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z","nonce":"abc123","expiry":"2026-07-27T00:05:00Z"}"#;
        let peeked = peek_guard_fields(bytes).expect("well-formed envelope must peek");
        assert_eq!(peeked.from.to_string(), "agent:local/alice");
        assert_eq!(peeked.to.to_string(), "agent:local/bob");
        assert_eq!(peeked.ts, "2026-07-27T00:00:00Z");
        assert_eq!(peeked.nonce.as_deref(), Some("abc123"));
        assert_eq!(peeked.expiry.as_deref(), Some("2026-07-27T00:05:00Z"));
    }

    #[test]
    fn peek_guard_fields_nonce_and_expiry_absent_are_none() {
        let bytes =
            br#"{"from":"agent:local/alice","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z"}"#;
        let peeked =
            peek_guard_fields(bytes).expect("envelope without nonce/expiry must still peek");
        assert!(peeked.nonce.is_none());
        assert!(peeked.expiry.is_none());
    }

    #[test]
    fn peek_guard_fields_rejects_missing_ts() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob"}"#;
        assert!(matches!(
            peek_guard_fields(bytes),
            Err(GuardReject::BadEnvelopeShape)
        ));
    }
}
