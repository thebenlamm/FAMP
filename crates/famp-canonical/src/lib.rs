//! `famp-canonical` — RFC 8785 JCS canonicalization for FAMP.
//!
//! This crate is the byte-exact substrate that every other FAMP crate signs
//! against. If two implementations disagree on the bytes this crate produces,
//! signature verification will fail and nothing else matters — so the public
//! API surface is intentionally narrow and the conformance gate is enforced
//! externally against RFC 8785 Appendix B vectors and the cyberphone corpus.
//!
//! # Public surfaces
//!
//! - [`canonicalize`] / [`Canonicalize`] — produce canonical bytes from any
//!   `serde::Serialize` value. This is the primary path.
//! - [`from_slice_strict`] / [`from_str_strict`] — parse inbound JSON bytes
//!   while rejecting duplicate object keys at any depth (FAMP protocol
//!   guarantee, not a hygiene preference).
//! - [`artifact_id_for_canonical_bytes`] / [`artifact_id_for_value`] —
//!   compute `sha256:<hex>` artifact IDs.
//! - [`CanonicalError`] — typed error surface; never returns `anyhow`.
//!
//! # INGRESS NOTE
//!
//! Inbound JSON bytes carrying a signature MUST be parsed via
//! [`from_slice_strict`] or [`from_str_strict`]. Calling `serde_json::from_*`
//! directly silently merges duplicate keys and breaks the FAMP protocol
//! guarantee (RESEARCH Pitfall 4).

#![forbid(unsafe_code)]

pub mod artifact_id;
pub mod canonical;
pub mod error;
pub mod strict_parse;

pub use artifact_id::{artifact_id_for_canonical_bytes, artifact_id_for_value, ArtifactIdString};
pub use canonical::{canonicalize, Canonicalize};
pub use error::CanonicalError;
pub use strict_parse::{from_slice_strict, from_str_strict};
