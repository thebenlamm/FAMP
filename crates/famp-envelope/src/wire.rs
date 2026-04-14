//! Private wire-plumbing for envelope encode/decode. NOT public API.
//!
//! Per CONTEXT.md D-A3, `WireEnvelope<B>` is the only serde-visible shape of
//! the envelope header + body; the public `UnsignedEnvelope<B>` /
//! `SignedEnvelope<B>` type-state sits on top.
//!
//! CRITICAL: no `#[serde(flatten)]`, no `#[serde(tag = ...)]` anywhere.
//! See `lib.rs` top-of-file warning and RESEARCH.md Pitfalls 1 and 2.

use crate::body::deliver::TerminalStatus;
use crate::body::BodySchema;
use crate::causality::Causality;
use crate::{EnvelopeScope, FampVersion, MessageClass, Timestamp};
use famp_core::{AuthorityScope, MessageId, Principal};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[allow(clippy::redundant_pub_crate)]
pub(crate) const SIGNATURE_FIELD: &str = "signature";

/// Generic wire envelope. Used as BOTH the serialize target (for building
/// the signing input inside `UnsignedEnvelope::sign`) AND the deserialize
/// target (inside `SignedEnvelope::decode_value` after the signature field
/// has been stripped from the raw JSON `Value`).
///
/// `body: B` is a plain generic struct field — NOT a tagged enum, NOT a
/// flattened type. This is the only composition in serde 1.0.228 that
/// actually enforces `deny_unknown_fields` on both envelope and body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "B: Serialize",
        deserialize = "B: serde::de::DeserializeOwned"
    )
)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct WireEnvelope<B: BodySchema> {
    pub famp: FampVersion,
    pub id: MessageId,
    pub from: Principal,
    pub to: Principal,
    pub scope: EnvelopeScope,
    pub class: MessageClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causality: Option<Causality>,
    pub authority: AuthorityScope,
    pub ts: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<TerminalStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<BTreeMap<String, serde_json::Value>>,
    pub body: B,
}
