//! Provenance stamp for mailbox-append records (D-01/D-02/D-17, QUAR-09).
//!
//! Fail-closed polarity: [`Origin::default()`] is [`Origin::Unknown`], NOT
//! [`Origin::Local`]. Every absence path — a missing `Register.origin`
//! field, a legacy pre-Phase-14 mailbox line with no stamp, or any
//! unrecognized on-disk record shape — resolves to `Unknown`, and every
//! rendering surface treats `Unknown` as untrusted. There is no code path
//! that upgrades absence to trust; a `Local` or `Gateway` stamp must always
//! be the product of an explicit broker decision at `Out::AppendMailbox`
//! time (D-02).

use serde::{Deserialize, Serialize};

/// Where a mailbox-append record's content originated, as declared by the
/// stamping broker at `Out::AppendMailbox` time (D-02).
///
/// NEVER trust-upgrade an absent stamp: [`Origin::default`] is
/// [`Origin::Unknown`]. This is the single most important line in Phase
/// 14 — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Sent by a locally-registered client with no gateway/proxy origin
    /// declared at `Register` time.
    Local,
    /// Sent by `famp-gateway`'s `ProxiedPrincipal::register` on behalf of
    /// a remote cross-host principal (D-01 correction: this is the ONLY
    /// place the gateway declares provenance; its absence is safe
    /// because this enum's `Default` is `Unknown`, never `Local`).
    Gateway,
    /// No stamp was present or recognized: a legacy pre-Phase-14 mailbox
    /// line, a `Register` frame that omitted `origin`, or any other
    /// on-disk shape that does not match [`StampedEnvelope`] exactly.
    /// Every rendering surface treats this as untrusted (QUAR-09).
    Unknown,
}

impl Default for Origin {
    /// D-01/D-02: absence resolves to untrusted, NEVER to local.
    fn default() -> Self {
        Self::Unknown
    }
}

/// On-disk mailbox record shape AND the `BusReply::InboxOk` wire element
/// (D-17): one shape, defined once, used both ways.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StampedEnvelope {
    pub origin: Origin,
    pub envelope: serde_json::Value,
}

/// Wrap an already-canonicalized envelope line in a [`StampedEnvelope`]
/// carrying `origin`, and return the CANONICAL bytes of the wrapper.
///
/// The inner envelope bytes are parsed back to a `Value` and copied
/// verbatim into the wrapper — the broker never reads or writes anything
/// inside it, which is what keeps the stamp unforgeable by a remote
/// sender (D-08): `WireEnvelope` is `deny_unknown_fields`, so an attacker
/// cannot pre-embed an `origin` key inside the envelope and have it
/// still decode.
///
/// Returns [`famp_canonical::CanonicalError`] rather than
/// `serde_json::Error` — the canonicalization step (RFC 8785 JCS) can
/// fail in ways a plain `serde_json::Error` cannot represent (e.g.
/// non-finite numbers, RFC 8785 §3.2.2.2), and this is the error type
/// every other canonicalization call site in the workspace already
/// surfaces.
pub fn stamp_line(
    envelope_line: &[u8],
    origin: Origin,
) -> Result<Vec<u8>, famp_canonical::CanonicalError> {
    let envelope: serde_json::Value = serde_json::from_slice(envelope_line)
        .map_err(famp_canonical::CanonicalError::InvalidJson)?;
    let stamped = StampedEnvelope { origin, envelope };
    famp_canonical::canonicalize(&stamped)
}

/// Split a raw on-disk / wire JSON value into its declared origin and the
/// inner envelope value.
///
/// Returns `(stamped_origin, inner)` ONLY when `line` is a JSON object
/// whose key set is EXACTLY `{"origin", "envelope"}` (the two keys of
/// [`StampedEnvelope`]) and whose `origin` value parses as a known
/// [`Origin`] variant. Every other shape — a bare pre-Phase-14 envelope
/// object, an object with the right keys but an unrecognized `origin`
/// string, an object with a third key, a non-object value — returns
/// `(Origin::Unknown, line)` unchanged. This is the fail-closed decode
/// side of D-02: absence or ambiguity of a stamp is untrusted, never
/// local.
#[must_use]
pub fn split_stamped(line: &serde_json::Value) -> (Origin, &serde_json::Value) {
    let Some(obj) = line.as_object() else {
        return (Origin::Unknown, line);
    };
    if obj.len() != 2 {
        return (Origin::Unknown, line);
    }
    let (Some(origin_value), Some(envelope_value)) = (obj.get("origin"), obj.get("envelope"))
    else {
        return (Origin::Unknown, line);
    };
    match serde_json::from_value::<Origin>(origin_value.clone()) {
        Ok(origin) => (origin, envelope_value),
        Err(_) => (Origin::Unknown, line),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{split_stamped, stamp_line, Origin, StampedEnvelope};
    use serde_json::json;

    #[test]
    fn origin_default_is_unknown() {
        assert_eq!(Origin::default(), Origin::Unknown);
    }

    #[test]
    fn origin_serde_snake_case_round_trips() {
        for (origin, wire) in [
            (Origin::Local, "\"local\""),
            (Origin::Gateway, "\"gateway\""),
            (Origin::Unknown, "\"unknown\""),
        ] {
            let bytes = famp_canonical::canonicalize(&origin).unwrap();
            assert_eq!(std::str::from_utf8(&bytes).unwrap(), wire);
            let decoded: Origin = famp_canonical::from_slice_strict(&bytes).unwrap();
            assert_eq!(decoded, origin);
        }
    }

    #[test]
    fn stamp_line_wraps_and_split_stamped_recovers() {
        let envelope = json!({"body": "hello", "id": "abc"});
        let line = famp_canonical::canonicalize(&envelope).unwrap();
        let stamped_bytes = stamp_line(&line, Origin::Gateway).unwrap();
        let value: serde_json::Value = famp_canonical::from_slice_strict(&stamped_bytes).unwrap();
        let (origin, inner) = split_stamped(&value);
        assert_eq!(origin, Origin::Gateway);
        assert_eq!(inner, &envelope);
    }

    #[test]
    fn split_stamped_legacy_bare_envelope_resolves_unknown() {
        // Pre-Phase-14 on-disk shape: a bare envelope object, no stamp.
        let legacy = json!({"body": "hello", "id": "abc", "extra": true});
        let (origin, inner) = split_stamped(&legacy);
        assert_eq!(origin, Origin::Unknown);
        assert_eq!(inner, &legacy);
    }

    #[test]
    fn split_stamped_non_object_resolves_unknown() {
        let value = json!("not an object");
        let (origin, inner) = split_stamped(&value);
        assert_eq!(origin, Origin::Unknown);
        assert_eq!(inner, &value);
    }

    /// D-01/D-02 fail-closed pin: an unrecognized origin string must
    /// resolve to `Unknown`, never panic, never default to `Local`.
    #[test]
    fn split_stamped_unknown_origin_string_resolves_unknown() {
        let value = json!({"origin": "not_a_real_origin", "envelope": {"body": "x"}});
        let (origin, inner) = split_stamped(&value);
        assert_eq!(origin, Origin::Unknown);
        // Shape matched but the origin value didn't parse — the whole
        // raw line is returned unchanged, not the (unverified) inner.
        assert_eq!(inner, &value);
    }

    /// D-02 fail-closed pin: a third key means the shape does not match
    /// `StampedEnvelope` exactly, so it must resolve to `Unknown`.
    #[test]
    fn split_stamped_extra_key_resolves_unknown() {
        let value = json!({
            "origin": "local",
            "envelope": {"body": "x"},
            "extra": "surprise",
        });
        let (origin, inner) = split_stamped(&value);
        assert_eq!(origin, Origin::Unknown);
        assert_eq!(inner, &value);
    }

    #[test]
    fn stamped_envelope_round_trips_via_serde() {
        let stamped = StampedEnvelope {
            origin: Origin::Local,
            envelope: json!({"a": 1}),
        };
        let bytes = famp_canonical::canonicalize(&stamped).unwrap();
        let decoded: StampedEnvelope = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(decoded, stamped);
    }
}
