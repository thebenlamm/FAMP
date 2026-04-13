//! [`ArtifactId`] — parsed `sha256:<64-lowercase-hex>` identifier (D-14..D-18).
//!
//! Owns the type-level invariant in `famp-core`; the actual SHA-256 hashing
//! helpers live in `famp-canonical` / `famp-crypto`. This crate does not
//! depend on either.

use std::fmt;
use std::str::FromStr;

/// A parsed artifact identifier. Invariant: the inner string matches
/// `^sha256:[0-9a-f]{64}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactId(String);

impl ArtifactId {
    /// Return the canonical wire form (e.g. `sha256:e3b0...`).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors returned when parsing an [`ArtifactId`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseArtifactIdError {
    #[error("missing 'sha256:' prefix")]
    MissingPrefix,
    #[error("unsupported algorithm (only 'sha256' accepted in v0.6)")]
    UnsupportedAlgorithm,
    #[error("hex payload must be exactly 64 lowercase hex characters")]
    InvalidHex,
}

impl FromStr for ArtifactId {
    type Err = ParseArtifactIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(colon) = s.find(':') else {
            return Err(ParseArtifactIdError::MissingPrefix);
        };
        let algo = &s[..colon];
        let hex = &s[colon + 1..];

        if algo != "sha256" {
            return Err(ParseArtifactIdError::UnsupportedAlgorithm);
        }
        if hex.len() != 64 {
            return Err(ParseArtifactIdError::InvalidHex);
        }
        for &b in hex.as_bytes() {
            let ok = matches!(b, b'0'..=b'9' | b'a'..=b'f');
            if !ok {
                return Err(ParseArtifactIdError::InvalidHex);
            }
        }
        Ok(Self(s.to_owned()))
    }
}

impl TryFrom<&str> for ArtifactId {
    type Error = ParseArtifactIdError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl TryFrom<String> for ArtifactId {
    type Error = ParseArtifactIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for ArtifactId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}
