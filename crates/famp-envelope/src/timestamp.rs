//! `Timestamp` — opaque byte-preserving RFC 3339 string.
//!
//! PITFALL P6: do NOT parse through `time::OffsetDateTime` — re-serialization
//! would differ from signed wire bytes and break signature verification.
//! We preserve the caller's input bytes verbatim and perform only a shallow
//! format check (length, `-`/`T`/`:` positions, trailing `Z` or offset).

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;

/// RFC 3339 timestamp preserved byte-exact from the wire.
///
/// Construction via deserialization enforces a shallow format gate, but
/// does NOT normalize the input. `Serialize` emits the original bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Timestamp(pub String);

pub(crate) fn shallow_validate(v: &str) -> bool {
    // PITFALL P6: shallow only. Full parsing would normalize.
    let bytes = v.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return false;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return false;
    }
    // Trailing Z or +HH:MM / -HH:MM offset.
    let last = bytes[bytes.len() - 1];
    if last == b'Z' {
        return true;
    }
    // Offset form: must end with ±HH:MM
    if bytes.len() < 25 {
        return false;
    }
    let off = &bytes[bytes.len() - 6..];
    (off[0] == b'+' || off[0] == b'-')
        && off[1].is_ascii_digit()
        && off[2].is_ascii_digit()
        && off[3] == b':'
        && off[4].is_ascii_digit()
        && off[5].is_ascii_digit()
}

/// Byte-exact canonical whole-second UTC form: `YYYY-MM-DDTHH:MM:SSZ`,
/// exactly 20 bytes, ending in a literal `Z` (never a `+HH:MM`/`-HH:MM`
/// offset, never a fractional-second suffix).
///
/// REL-02 (12-02) finding: [`shallow_validate`] alone accepts EITHER a `Z`
/// suffix or an offset suffix, and does not bound any fractional-second
/// digits before that suffix. Two strings that both pass
/// `shallow_validate` are therefore not guaranteed to share the same
/// representation, so a raw lexical/byte-string `<=`/`>=` comparison
/// between them (as `SignedEnvelope::federation_format_ok`'s expiry-vs-ts
/// ordering check performs) does not reliably reflect true chronological
/// order unless BOTH operands are also known to share this canonical
/// form. Callers that need a lexically-safe ordering comparison must
/// gate on this function first.
pub(crate) fn is_canonical_utc_form(v: &str) -> bool {
    shallow_validate(v) && v.len() == 20 && v.as_bytes()[19] == b'Z'
}

impl serde::Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

struct TimestampVisitor;

impl Visitor<'_> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an RFC 3339 timestamp string (byte-preserving, no normalization)")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        if shallow_validate(v) {
            Ok(Timestamp(v.to_owned()))
        } else {
            Err(de::Error::invalid_value(
                Unexpected::Str(v),
                &"an RFC 3339 timestamp like \"2026-04-13T00:00:00Z\"",
            ))
        }
    }
}

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TimestampVisitor)
    }
}
