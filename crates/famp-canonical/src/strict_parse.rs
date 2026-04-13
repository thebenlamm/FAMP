//! Strict ingress parsing — duplicate-key rejection at parse time.
//!
//! Per CONTEXT.md D-04..D-07: canonicalization and strict parsing are
//! **separate public API surfaces**. `canonicalize` operates on typed
//! in-memory values (trusted). The strict parsers below operate on inbound
//! JSON bytes (untrusted) and enforce the FAMP protocol guarantee that a
//! signed message MUST NOT contain duplicate object keys at any depth.
//!
//! Implementation strategy (per D-06): we use a serde-visitor on a custom
//! "strict tree" type that errors on the first duplicate key it sees during
//! deserialization. After the strict pass succeeds we know the bytes are
//! duplicate-free and re-parse into the caller's target type `T`. The
//! two-pass cost is acceptable for protocol-message sizes (a few KB) and
//! avoids maintaining two parser surfaces.

use std::collections::HashSet;
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::error::CanonicalError;

/// Parse JSON bytes into `T`, rejecting any duplicate object key at any
/// depth.
///
/// # Errors
///
/// - [`CanonicalError::DuplicateKey`] if any object in the input contains a
///   repeated key.
/// - [`CanonicalError::InvalidJson`] for any other parse failure.
///
/// # Cost
///
/// The input is parsed twice: once into an internal "strict tree" that
/// surfaces duplicates, then again into the target type `T`. This is
/// intentional (see module docs).
pub fn from_slice_strict<T: serde::de::DeserializeOwned>(
    input: &[u8],
) -> Result<T, CanonicalError> {
    // Pass 1: strict structural validation — proves no duplicate keys.
    let mut de = serde_json::Deserializer::from_slice(input);
    let _: StrictTree = StrictTree::deserialize(&mut de).map_err(map_serde_err)?;
    de.end().map_err(CanonicalError::InvalidJson)?;
    // Pass 2: now that we've proven duplicate-freedom, parse into the
    // caller's target type. Any error here is a structural mismatch with T,
    // not a duplicate-key issue.
    serde_json::from_slice::<T>(input).map_err(CanonicalError::InvalidJson)
}

/// Convenience wrapper for `&str` inputs — see [`from_slice_strict`].
pub fn from_str_strict<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, CanonicalError> {
    from_slice_strict(input.as_bytes())
}

/// Translate a `serde_json::Error` raised during the strict pass into a
/// [`CanonicalError`]. Duplicate-key signals are smuggled through serde's
/// `custom` error channel using a sentinel prefix because the visitor has no
/// other way to attach structured payloads to the error.
fn map_serde_err(e: serde_json::Error) -> CanonicalError {
    let msg = e.to_string();
    if let Some(rest) = msg.strip_prefix("__DUPLICATE_KEY__:") {
        // serde_json appends " at line N column M" to custom errors; trim it
        // so the reported key matches what the user actually wrote.
        let key = rest
            .split(" at line")
            .next()
            .unwrap_or(rest)
            .trim()
            .to_string();
        CanonicalError::DuplicateKey { key }
    } else {
        CanonicalError::InvalidJson(e)
    }
}

/// Internal "strict tree" representation. The only purpose of this type is
/// to drive a `Visitor` that errors on duplicate object keys; the resulting
/// tree is discarded by `from_slice_strict`, so the field payloads are
/// intentionally never read.
#[allow(dead_code)]
enum StrictTree {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<StrictTree>),
    Object(Vec<(String, StrictTree)>),
}

impl<'de> Deserialize<'de> for StrictTree {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StrictTree;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(StrictTree::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(StrictTree::Null)
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictTree::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictTree::Number(v.into()))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictTree::Number(v.into()))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(StrictTree::Number)
                    .ok_or_else(|| de::Error::custom("non-finite number"))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StrictTree::String(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(StrictTree::String(v))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element::<StrictTree>()? {
                    out.push(v);
                }
                Ok(StrictTree::Array(out))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut seen: HashSet<String> = HashSet::new();
                let mut entries: Vec<(String, StrictTree)> = Vec::new();
                while let Some(k) = map.next_key::<String>()? {
                    if !seen.insert(k.clone()) {
                        return Err(de::Error::custom(format!("__DUPLICATE_KEY__:{k}")));
                    }
                    let v: StrictTree = map.next_value()?;
                    entries.push((k, v));
                }
                Ok(StrictTree::Object(entries))
            }
        }
        d.deserialize_any(V)
    }
}
