//! Identity types: [`Principal`] and [`Instance`] (FAMP v0.5.1 §5.1, §5.2).
//!
//! Per Phase 3 CONTEXT D-01..D-09 and D-34..D-36:
//! - Strict ASCII, no normalization, case-sensitive byte-for-byte round trip.
//! - Separate parsers — `Principal` rejects instance-bearing strings and
//!   `Instance` rejects principal-only strings (D-01).
//! - Narrow parse error enums that do NOT cross-convert into
//!   `ProtocolErrorKind` (D-08, D-35).

use std::fmt;
use std::str::FromStr;

// ---------- Principal ----------

/// A FAMP principal identity: `agent:<authority>/<name>`.
///
/// Parsed, validated, and stored byte-for-byte. No normalization.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal {
    authority: String,
    name: String,
}

impl Principal {
    /// Authority component (DNS-style, case-sensitive).
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Name component.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Errors returned when parsing a [`Principal`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParsePrincipalError {
    #[error("principal must start with 'agent:'")]
    MissingScheme,
    #[error("principal must not carry an instance tail '#...'")]
    UnexpectedInstanceTail,
    #[error("authority is empty or malformed: {0}")]
    InvalidAuthority(&'static str),
    #[error("name is empty or malformed: {0}")]
    InvalidName(&'static str),
    #[error("missing '/' between authority and name")]
    MissingNameSeparator,
}

impl FromStr for Principal {
    type Err = ParsePrincipalError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s
            .strip_prefix("agent:")
            .ok_or(ParsePrincipalError::MissingScheme)?;

        // Principal MUST NOT contain an instance tail.
        if rest.contains('#') {
            return Err(ParsePrincipalError::UnexpectedInstanceTail);
        }

        let slash = rest
            .find('/')
            .ok_or(ParsePrincipalError::MissingNameSeparator)?;
        let authority = &rest[..slash];
        let name = &rest[slash + 1..];

        validate_authority(authority).map_err(ParsePrincipalError::InvalidAuthority)?;
        validate_name_or_instance_id(name).map_err(ParsePrincipalError::InvalidName)?;

        Ok(Self {
            authority: authority.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl fmt::Display for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent:{}/{}", self.authority, self.name)
    }
}

impl serde::Serialize for Principal {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Principal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// ---------- Instance ----------

/// A FAMP instance identity: `agent:<authority>/<name>#<instance_id>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(clippy::struct_field_names)]
pub struct Instance {
    authority: String,
    name: String,
    instance_id: String,
}

impl Instance {
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }
}

/// Errors returned when parsing an [`Instance`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseInstanceError {
    #[error("instance must start with 'agent:'")]
    MissingScheme,
    #[error("instance must carry an instance tail '#<id>'")]
    MissingInstanceTail,
    #[error("authority is empty or malformed: {0}")]
    InvalidAuthority(&'static str),
    #[error("name is empty or malformed: {0}")]
    InvalidName(&'static str),
    #[error("instance id is empty or malformed: {0}")]
    InvalidInstanceId(&'static str),
    #[error("missing '/' between authority and name")]
    MissingNameSeparator,
}

impl FromStr for Instance {
    type Err = ParseInstanceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s
            .strip_prefix("agent:")
            .ok_or(ParseInstanceError::MissingScheme)?;

        let slash = rest
            .find('/')
            .ok_or(ParseInstanceError::MissingNameSeparator)?;
        let authority = &rest[..slash];
        let tail = &rest[slash + 1..];

        let hash = tail
            .find('#')
            .ok_or(ParseInstanceError::MissingInstanceTail)?;
        let name = &tail[..hash];
        let instance_id = &tail[hash + 1..];

        // Reject a second '#' in instance_id.
        if instance_id.contains('#') {
            return Err(ParseInstanceError::InvalidInstanceId("multiple '#' separators"));
        }

        validate_authority(authority).map_err(ParseInstanceError::InvalidAuthority)?;
        validate_name_or_instance_id(name).map_err(ParseInstanceError::InvalidName)?;
        validate_name_or_instance_id(instance_id)
            .map_err(ParseInstanceError::InvalidInstanceId)?;

        Ok(Self {
            authority: authority.to_owned(),
            name: name.to_owned(),
            instance_id: instance_id.to_owned(),
        })
    }
}

impl fmt::Display for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent:{}/{}#{}", self.authority, self.name, self.instance_id)
    }
}

impl serde::Serialize for Instance {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Instance {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// ---------- Private validators ----------

/// D-04: DNS-style authority. Labels `[A-Za-z0-9-]+`, no leading/trailing `-`,
/// no underscores, total length ≤ 253, ≥1 label. Strict ASCII.
fn validate_authority(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("empty authority");
    }
    if s.len() > 253 {
        return Err("authority exceeds 253 bytes");
    }
    if !s.is_ascii() {
        return Err("authority must be ASCII");
    }
    let mut label_count = 0usize;
    for label in s.split('.') {
        label_count += 1;
        if label.is_empty() {
            return Err("empty label");
        }
        let bytes = label.as_bytes();
        if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
            return Err("label must not start or end with '-'");
        }
        for &b in bytes {
            let ok = b.is_ascii_alphanumeric() || b == b'-';
            if !ok {
                return Err("label contains invalid character");
            }
        }
    }
    if label_count == 0 {
        return Err("authority requires at least one label");
    }
    Ok(())
}

/// D-05 / D-06: name / instance-id. ASCII `[A-Za-z0-9._-]+`, length 1..=64.
fn validate_name_or_instance_id(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("empty");
    }
    if s.len() > 64 {
        return Err("length exceeds 64 bytes");
    }
    if !s.is_ascii() {
        return Err("must be ASCII");
    }
    for &b in s.as_bytes() {
        let ok = b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-';
        if !ok {
            return Err("contains invalid character");
        }
    }
    Ok(())
}
