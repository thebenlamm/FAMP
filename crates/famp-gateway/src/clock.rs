//! Verifier-side wall clock (REVK-01/REVK-03).
//!
//! `now_canonical_utc()` is the ONLY place `famp-gateway` reads the system
//! clock for a trust decision — `famp_keyring::Keyring::active_key` never
//! reads a clock itself; `now` always arrives as an argument (T-15-10).
//! Building the canonical string positionally (rather than through a
//! generic RFC 3339 formatter + trim) makes this function infallible, so
//! there is no fallible-clock branch anywhere on the expiry-decision path.

use time::OffsetDateTime;

/// Return the current instant as the canonical whole-second UTC form
/// `YYYY-MM-DDTHH:MM:SSZ` — exactly 20 bytes, ending in a literal `Z`.
#[must_use]
pub fn now_canonical_utc() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Strip any subsecond component `time`'s `Rfc3339` formatter may emit,
/// rebuilding as the trimmed `YYYY-MM-DDTHH:MM:SSZ` form. Relocated from
/// `egress.rs::federation_expiry`'s private helper of the same name and
/// behavior (byte-identical) so both the egress-expiry path and (future)
/// clock-adjacent code share one implementation.
pub(crate) fn strip_subseconds(ts: &str) -> String {
    let Some(dot_idx) = ts.find('.') else {
        return ts.to_string();
    };
    let tail_idx = ts[dot_idx..]
        .find(['Z', '+', '-'])
        .map_or(ts.len(), |i| dot_idx + i);
    let mut out = String::with_capacity(ts.len() - (tail_idx - dot_idx));
    out.push_str(&ts[..dot_idx]);
    out.push_str(&ts[tail_idx..]);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn now_canonical_utc_is_exactly_20_bytes_ending_in_z() {
        let s = now_canonical_utc();
        assert_eq!(s.len(), 20, "expected exactly 20 bytes, got: {s}");
        assert!(s.ends_with('Z'), "expected trailing 'Z', got: {s}");
        assert!(
            famp_keyring::entry::is_canonical_utc(&s),
            "must satisfy the canonical-UTC gate: {s}"
        );
    }

    #[test]
    fn strip_subseconds_trims_fractional_and_leaves_plain_z_alone() {
        assert_eq!(
            strip_subseconds("2026-07-27T00:00:00.123456Z"),
            "2026-07-27T00:00:00Z"
        );
        assert_eq!(
            strip_subseconds("2026-07-27T00:00:00Z"),
            "2026-07-27T00:00:00Z"
        );
    }
}
