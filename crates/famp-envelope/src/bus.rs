//! Bus-side envelope variant.
//!
//! INVARIANT (BUS-11): `BusEnvelope<B>` carries NO signature, ever. The bus is
//! the local-host trust boundary; envelopes identify their sender by the
//! broker's PID-checked register path, not by Ed25519 signature. INV-10 lives
//! at the federation gateway, not here.

use crate::body::{
    AckBody, AuditLogBody, BodySchema, CommitBody, ControlBody, DeliverBody, RequestBody,
};
use crate::wire::WireEnvelope;
use crate::{EnvelopeDecodeError, FAMP_SPEC_VERSION};
use famp_core::MessageClass;
use serde_json::Value;

/// # BUS-11 `compile_fail` gate 1 — no public constructor
///
/// ```compile_fail
/// use famp_envelope::bus::BusEnvelope;
/// use famp_envelope::body::AckBody;
/// let _: BusEnvelope<AckBody> = BusEnvelope { inner: unimplemented!() };
/// ```
///
/// # BUS-11 `compile_fail` gate 2 — federation/bus types do not unify
///
/// ```compile_fail
/// use famp_envelope::{SignedEnvelope, bus::BusEnvelope, body::AckBody};
/// fn fed_only(_e: SignedEnvelope<AckBody>) {}
/// fn use_bus(b: BusEnvelope<AckBody>) { fed_only(b); }
/// ```
#[derive(Debug, Clone)]
pub struct BusEnvelope<B: BodySchema> {
    inner: WireEnvelope<B>,
}

impl<B: BodySchema> BusEnvelope<B> {
    #[must_use]
    pub const fn body(&self) -> &B {
        &self.inner.body
    }

    #[must_use]
    pub const fn class(&self) -> MessageClass {
        self.inner.class
    }

    /// Decode canonical JSON without a `signature` field into a typed bus
    /// envelope. Any `signature` key is rejected before typed deserialization.
    pub fn decode(bytes: &[u8]) -> Result<Self, EnvelopeDecodeError> {
        let mut value: Value =
            famp_canonical::from_slice_strict(bytes).map_err(EnvelopeDecodeError::MalformedJson)?;
        let obj = value.as_object_mut().ok_or_else(|| {
            EnvelopeDecodeError::BodyValidation("envelope root is not a JSON object".into())
        })?;
        if obj.contains_key("signature") {
            return Err(EnvelopeDecodeError::UnexpectedSignature);
        }
        match obj.get("famp") {
            Some(Value::String(s)) if s == FAMP_SPEC_VERSION => {}
            Some(Value::String(s)) => {
                return Err(EnvelopeDecodeError::UnsupportedVersion { found: s.clone() });
            }
            Some(_) => {
                return Err(EnvelopeDecodeError::BodyValidation(
                    "envelope.famp must be a string".into(),
                ));
            }
            None => return Err(EnvelopeDecodeError::MissingField { field: "famp" }),
        }
        let wire: WireEnvelope<B> = serde_json::from_value(value)
            .map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))?;
        if wire.class != B::CLASS {
            return Err(EnvelopeDecodeError::ClassMismatch {
                expected: B::CLASS,
                got: wire.class,
            });
        }
        if wire.scope != B::SCOPE {
            return Err(EnvelopeDecodeError::ScopeMismatch {
                class: B::CLASS,
                expected: B::SCOPE,
                got: wire.scope,
            });
        }
        wire.body
            .post_decode_validate(wire.terminal_status.as_ref())?;
        Ok(Self { inner: wire })
    }
}

/// Bus-side dispatch enum paralleling `AnySignedEnvelope`.
#[derive(Debug, Clone)]
pub enum AnyBusEnvelope {
    Request(BusEnvelope<RequestBody>),
    Commit(BusEnvelope<CommitBody>),
    Deliver(BusEnvelope<DeliverBody>),
    Ack(BusEnvelope<AckBody>),
    Control(BusEnvelope<ControlBody>),
    AuditLog(BusEnvelope<AuditLogBody>),
}

impl AnyBusEnvelope {
    #[must_use]
    pub const fn class(&self) -> MessageClass {
        match self {
            Self::Request(e) => e.class(),
            Self::Commit(e) => e.class(),
            Self::Deliver(e) => e.class(),
            Self::Ack(e) => e.class(),
            Self::Control(e) => e.class(),
            Self::AuditLog(e) => e.class(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, EnvelopeDecodeError> {
        let value: Value =
            famp_canonical::from_slice_strict(bytes).map_err(EnvelopeDecodeError::MalformedJson)?;
        let class_str = value
            .get("class")
            .and_then(Value::as_str)
            .ok_or(EnvelopeDecodeError::MissingField { field: "class" })?
            .to_string();
        match class_str.as_str() {
            "request" => Ok(Self::Request(BusEnvelope::<RequestBody>::decode(bytes)?)),
            "commit" => Ok(Self::Commit(BusEnvelope::<CommitBody>::decode(bytes)?)),
            "deliver" => Ok(Self::Deliver(BusEnvelope::<DeliverBody>::decode(bytes)?)),
            "ack" => Ok(Self::Ack(BusEnvelope::<AckBody>::decode(bytes)?)),
            "control" => Ok(Self::Control(BusEnvelope::<ControlBody>::decode(bytes)?)),
            "audit_log" => Ok(Self::AuditLog(BusEnvelope::<AuditLogBody>::decode(bytes)?)),
            _ => Err(EnvelopeDecodeError::UnknownClass { found: class_str }),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn audit_log_value() -> Value {
        serde_json::json!({
            "famp": FAMP_SPEC_VERSION,
            "class": "audit_log",
            "scope": "standalone",
            "id": "01890000-0000-7000-8000-000000000001",
            "from": "agent:example.test/alice",
            "to": "agent:example.test/bob",
            "authority": "advisory",
            "ts": "2026-04-27T12:00:00Z",
            "body": { "event": "user_login" }
        })
    }

    #[test]
    fn rejects_signed_envelope_at_runtime() {
        let mut json = audit_log_value();
        json.as_object_mut()
            .unwrap()
            .insert("signature".into(), Value::String("AAAA".into()));
        let bytes = serde_json::to_vec(&json).unwrap();
        let err = BusEnvelope::<AuditLogBody>::decode(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeDecodeError::UnexpectedSignature));
    }

    #[test]
    fn audit_log_decodes_unsigned() {
        let bytes = famp_canonical::canonicalize(&audit_log_value()).unwrap();
        let env = BusEnvelope::<AuditLogBody>::decode(&bytes).unwrap();
        assert_eq!(env.body().event, "user_login");
        assert_eq!(env.class(), MessageClass::AuditLog);
    }
}
