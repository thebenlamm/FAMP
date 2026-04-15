//! `AnySignedEnvelope` — router enum for the envelope decode path.
//!
//! Used when the caller does not know the body class in advance (Phase 3
//! `MemoryTransport` and Phase 4 HTTP pre-routing middleware). Dispatches on
//! the wire `class` field via manual `serde_json::Value` inspection — NOT
//! `#[serde(tag = "class")]`. A tagged enum would interact badly with
//! `deny_unknown_fields` on the body variants. See RESEARCH.md "How
//! `AnySignedEnvelope` Decode Works Without Tagged Enums".

#![allow(clippy::module_name_repetitions)]

use crate::body::{AckBody, CommitBody, ControlBody, DeliverBody, RequestBody};
use crate::{EnvelopeDecodeError, MessageClass, SignedEnvelope};
use famp_canonical::from_slice_strict;
use famp_crypto::TrustedVerifyingKey;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum AnySignedEnvelope {
    Request(SignedEnvelope<RequestBody>),
    Commit(SignedEnvelope<CommitBody>),
    Deliver(SignedEnvelope<DeliverBody>),
    Ack(SignedEnvelope<AckBody>),
    Control(SignedEnvelope<ControlBody>),
}

impl AnySignedEnvelope {
    /// Return the envelope class. Cheap — reads from the already-decoded inner.
    #[must_use]
    pub fn class(&self) -> MessageClass {
        match self {
            Self::Request(e) => e.class(),
            Self::Commit(e) => e.class(),
            Self::Deliver(e) => e.class(),
            Self::Ack(e) => e.class(),
            Self::Control(e) => e.class(),
        }
    }

    /// Decode wire bytes into a typed router variant. Strict-parses first,
    /// then inspects the top-level `class` string to pick a typed decode
    /// path. Unknown classes short-circuit with
    /// [`EnvelopeDecodeError::UnknownClass`] BEFORE signature verification
    /// runs — an unknown-class envelope is by definition un-dispatchable.
    pub fn decode(
        bytes: &[u8],
        verifier: &TrustedVerifyingKey,
    ) -> Result<Self, EnvelopeDecodeError> {
        let value: Value = from_slice_strict(bytes).map_err(EnvelopeDecodeError::MalformedJson)?;
        let class_str = value
            .get("class")
            .and_then(Value::as_str)
            .ok_or(EnvelopeDecodeError::MissingField { field: "class" })?
            .to_string();
        match class_str.as_str() {
            "request" => Ok(Self::Request(SignedEnvelope::<RequestBody>::decode_value(
                value, verifier,
            )?)),
            "commit" => Ok(Self::Commit(SignedEnvelope::<CommitBody>::decode_value(
                value, verifier,
            )?)),
            "deliver" => Ok(Self::Deliver(SignedEnvelope::<DeliverBody>::decode_value(
                value, verifier,
            )?)),
            "ack" => Ok(Self::Ack(SignedEnvelope::<AckBody>::decode_value(
                value, verifier,
            )?)),
            "control" => Ok(Self::Control(SignedEnvelope::<ControlBody>::decode_value(
                value, verifier,
            )?)),
            _ => Err(EnvelopeDecodeError::UnknownClass { found: class_str }),
        }
    }
}
