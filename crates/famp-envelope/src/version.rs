//! `FampVersion` — literal `"0.5.1"` wire string.
//!
//! Hand-written (de)serialization guarantees the wire bytes cannot be
//! renamed, rewritten, or relaxed by a future `#[serde(rename = ...)]`
//! drive-by edit. Per RESEARCH.md P10 / CONTEXT.md D-B5.

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;

/// The single spec-version string this crate understands.
pub const FAMP_SPEC_VERSION: &str = "0.5.1";

/// Unit-struct placeholder that always serializes as the literal `"0.5.1"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FampVersion;

impl serde::Serialize for FampVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(FAMP_SPEC_VERSION)
    }
}

struct FampVersionVisitor;

impl<'de> Visitor<'de> for FampVersionVisitor {
    type Value = FampVersion;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the literal string \"0.5.1\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        if v == FAMP_SPEC_VERSION {
            Ok(FampVersion)
        } else {
            Err(de::Error::invalid_value(
                Unexpected::Str(v),
                &"the literal string \"0.5.1\"",
            ))
        }
    }
}

impl<'de> serde::Deserialize<'de> for FampVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(FampVersionVisitor)
    }
}
