//! Public type-state: `UnsignedEnvelope<B>` → `SignedEnvelope<B>`.
//!
//! INV-10 at the type level: `Option<FampSignature>` does not appear anywhere.
//! `SignedEnvelope` can only be constructed via [`UnsignedEnvelope::sign`] (which
//! consumes self) or [`SignedEnvelope::decode`] (which verifies before returning).
//! There is no third "parsed but not yet verified" state.
//!
//! PITFALL P3: signature verification operates on the raw `serde_json::Value`
//! with the signature field stripped — NOT on the typed struct re-serialized.
//! Typed decode → reserialize would drop unknown-to-us-today fields and break
//! downstream agents that added envelope extensions.
//!
//! PITFALL P8: `decode_value` removes the `signature` key BEFORE canonicalizing
//! for verify. A dedicated test locks this property.

#![allow(
    clippy::missing_const_for_fn,
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::too_long_first_doc_paragraph,
    clippy::doc_lazy_continuation,
    clippy::use_self
)]

use crate::body::deliver::TerminalStatus;
use crate::body::BodySchema;
use crate::causality::Causality;
use crate::wire::{WireEnvelope, SIGNATURE_FIELD};
use crate::{
    EnvelopeDecodeError, EnvelopeScope, FampVersion, MessageClass, Timestamp,
};
use famp_canonical::from_slice_strict;
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{
    sign_value, verify_value, FampSignature, FampSigningKey, TrustedVerifyingKey,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// An envelope that has been assembled but not yet signed.
///
/// Consumed by [`UnsignedEnvelope::sign`] into a `SignedEnvelope<B>`. There is
/// no public API that yields an on-wire envelope in this state — the only path
/// to wire bytes is through `sign()` → `SignedEnvelope::encode()`.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct UnsignedEnvelope<B: BodySchema> {
    pub famp: FampVersion,
    pub id: MessageId,
    pub from: Principal,
    pub to: Principal,
    pub scope: EnvelopeScope,
    pub class: MessageClass,
    pub causality: Option<Causality>,
    pub authority: AuthorityScope,
    pub ts: Timestamp,
    pub terminal_status: Option<TerminalStatus>,
    pub idempotency_key: Option<String>,
    pub extensions: Option<BTreeMap<String, Value>>,
    pub body: B,
}

/// An envelope that has been verified — either by signing it ourselves or by
/// decoding + `verify_strict` over the sender's key.
///
/// "Signed" ≡ "verified by construction." No third `VerifiedEnvelope` state.
/// The inner `signature` is private; there is no public constructor that
/// takes an `Option<FampSignature>` — INV-10 at the type level.
///
/// # INV-10 compile_fail gate 1 — no public constructor
///
/// ```compile_fail
/// use famp_envelope::{SignedEnvelope, body::AckBody};
/// // Must fail: `inner` and `signature` are private fields — no public
/// // constructor exists. The only paths in are `UnsignedEnvelope::sign()`
/// // and `SignedEnvelope::decode()`.
/// let _: SignedEnvelope<AckBody> = SignedEnvelope {
///     inner: unimplemented!(),
///     signature: unimplemented!(),
/// };
/// ```
///
/// # INV-10 compile_fail gate 2 — signature is never `Option`
///
/// ```compile_fail
/// use famp_envelope::SignedEnvelope;
/// use famp_envelope::body::AckBody;
/// fn accepts_option(_: Option<famp_crypto::FampSignature>) {}
/// let e: SignedEnvelope<AckBody> = unimplemented!();
/// // `signature` is private AND non-Option, so this fails to type-check
/// // regardless of how you reach for it.
/// accepts_option(e.signature);
/// ```
#[derive(Debug, Clone)]
pub struct SignedEnvelope<B: BodySchema> {
    inner: UnsignedEnvelope<B>,
    signature: FampSignature,
}

impl<B: BodySchema> UnsignedEnvelope<B> {
    /// Construct a typed `UnsignedEnvelope` with `scope = B::SCOPE` and
    /// `class = B::CLASS` forced at the type level. Wrong `(class, body)`
    /// pairs are unrepresentable.
    pub fn new(
        id: MessageId,
        from: Principal,
        to: Principal,
        authority: AuthorityScope,
        ts: Timestamp,
        body: B,
    ) -> Self {
        Self {
            famp: FampVersion,
            id,
            from,
            to,
            scope: B::SCOPE,
            class: B::CLASS,
            causality: None,
            authority,
            ts,
            terminal_status: None,
            idempotency_key: None,
            extensions: None,
            body,
        }
    }

    #[must_use]
    pub fn with_causality(mut self, c: Causality) -> Self {
        self.causality = Some(c);
        self
    }

    #[must_use]
    pub fn with_terminal_status(mut self, ts: TerminalStatus) -> Self {
        self.terminal_status = Some(ts);
        self
    }

    #[must_use]
    pub fn with_idempotency_key(mut self, k: String) -> Self {
        self.idempotency_key = Some(k);
        self
    }

    /// Sign the envelope. Consumes self — no half-signed state possible.
    ///
    /// INV-10 is enforced by type state: the only way out of `UnsignedEnvelope`
    /// is through this method (or dropping the value on the floor). There is
    /// no public API that writes an unsigned envelope to the wire.
    pub fn sign(self, sk: &FampSigningKey) -> Result<SignedEnvelope<B>, EnvelopeDecodeError> {
        // Serialize via a borrowing view so we don't have to clone `body: B`.
        let view = WireEnvelopeRef::<'_, B> {
            famp: self.famp,
            id: &self.id,
            from: &self.from,
            to: &self.to,
            scope: self.scope,
            class: self.class,
            causality: self.causality.as_ref(),
            authority: self.authority,
            ts: &self.ts,
            terminal_status: self.terminal_status.as_ref(),
            idempotency_key: self.idempotency_key.as_ref(),
            extensions: self.extensions.as_ref(),
            body: &self.body,
        };
        let value = serde_json::to_value(&view)
            .map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))?;
        let signature =
            sign_value(sk, &value).map_err(EnvelopeDecodeError::InvalidSignatureEncoding)?;
        Ok(SignedEnvelope {
            inner: self,
            signature,
        })
    }
}


/// Borrowing serialize-only projection over `UnsignedEnvelope` / `SignedEnvelope`.
/// Avoids cloning `B` for the sign and encode paths. Private — never public.
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireEnvelopeRef<'a, B: BodySchema> {
    famp: FampVersion,
    id: &'a MessageId,
    from: &'a Principal,
    to: &'a Principal,
    scope: EnvelopeScope,
    class: MessageClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    causality: Option<&'a Causality>,
    authority: AuthorityScope,
    ts: &'a Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_status: Option<&'a TerminalStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotency_key: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<&'a BTreeMap<String, Value>>,
    body: &'a B,
}

impl<B: BodySchema> SignedEnvelope<B> {
    /// Decode bytes into a typed `SignedEnvelope<B>`. This is the only public
    /// entry point for callers that know the body class in advance.
    ///
    /// Flow: strict-parse → strip signature → `verify_strict` on the stripped
    /// Value → typed deserialize → class/scope cross-check → body cross-field
    /// validation.
    pub fn decode(
        bytes: &[u8],
        verifier: &TrustedVerifyingKey,
    ) -> Result<Self, EnvelopeDecodeError> {
        let value = from_slice_strict(bytes).map_err(EnvelopeDecodeError::MalformedJson)?;
        Self::decode_value(value, verifier)
    }

    /// Shared decode core. Pre-condition: `value` is a JSON object.
    /// Used by both the typed path and [`crate::AnySignedEnvelope::decode`].
    ///
    /// PITFALL P3: signature is verified over the raw `Value` (with the
    /// `signature` field stripped), NOT over the typed struct.
    pub(crate) fn decode_value(
        mut value: Value,
        verifier: &TrustedVerifyingKey,
    ) -> Result<Self, EnvelopeDecodeError> {
        let obj = value.as_object_mut().ok_or_else(|| {
            EnvelopeDecodeError::BodyValidation("envelope root is not a JSON object".into())
        })?;

        // Step 1: extract and strip the signature field.
        let sig_val = obj
            .remove(SIGNATURE_FIELD)
            .ok_or(EnvelopeDecodeError::MissingSignature)?;
        let sig_str = sig_val
            .as_str()
            .ok_or(EnvelopeDecodeError::MissingSignature)?;
        let signature = FampSignature::from_b64url(sig_str)
            .map_err(EnvelopeDecodeError::InvalidSignatureEncoding)?;

        // Step 2: verify over the stripped Value (PITFALL P3 — NOT the typed
        // struct re-serialized). `verify_value` canonicalizes internally.
        verify_value(verifier, &value, &signature)
            .map_err(|_| EnvelopeDecodeError::SignatureInvalid)?;

        // Step 3: deserialize into the typed wire struct. `deny_unknown_fields`
        // surfaces envelope-level unknown keys here as typed errors.
        let wire: WireEnvelope<B> =
            serde_json::from_value(value).map_err(Self::map_serde_error)?;

        // Step 4: class + scope cross-check (D-C1 / §7.3a).
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

        // Step 5: body cross-field validation (deliver interim × terminal_status,
        // bounds ≥2-key rule, etc.). Default impl is a no-op.
        wire.body
            .post_decode_validate(wire.terminal_status.as_ref())?;

        let inner = UnsignedEnvelope {
            famp: wire.famp,
            id: wire.id,
            from: wire.from,
            to: wire.to,
            scope: wire.scope,
            class: wire.class,
            causality: wire.causality,
            authority: wire.authority,
            ts: wire.ts,
            terminal_status: wire.terminal_status,
            idempotency_key: wire.idempotency_key,
            extensions: wire.extensions,
            body: wire.body,
        };
        Ok(SignedEnvelope { inner, signature })
    }

    fn map_serde_error(e: serde_json::Error) -> EnvelopeDecodeError {
        let msg = e.to_string();
        if let Some(rest) = msg.strip_prefix("unknown field `") {
            if let Some(field) = rest.split('`').next() {
                return EnvelopeDecodeError::UnknownEnvelopeField {
                    field: field.to_string(),
                };
            }
        }
        EnvelopeDecodeError::BodyValidation(msg)
    }

    /// Serialize a `SignedEnvelope` back to wire bytes. Not canonical — the
    /// canonical form is reconstructed at verify time.
    pub fn encode(&self) -> Result<Vec<u8>, EnvelopeDecodeError> {
        let view = WireEnvelopeRef::<'_, B> {
            famp: self.inner.famp,
            id: &self.inner.id,
            from: &self.inner.from,
            to: &self.inner.to,
            scope: self.inner.scope,
            class: self.inner.class,
            causality: self.inner.causality.as_ref(),
            authority: self.inner.authority,
            ts: &self.inner.ts,
            terminal_status: self.inner.terminal_status.as_ref(),
            idempotency_key: self.inner.idempotency_key.as_ref(),
            extensions: self.inner.extensions.as_ref(),
            body: &self.inner.body,
        };
        let mut value = serde_json::to_value(&view)
            .map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))?;
        let obj = value
            .as_object_mut()
            .ok_or_else(|| EnvelopeDecodeError::BodyValidation("wire not an object".into()))?;
        obj.insert(
            SIGNATURE_FIELD.to_string(),
            Value::String(self.signature.to_b64url()),
        );
        serde_json::to_vec(&value).map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))
    }

    // --- Stable read accessors for Phase 2 (FSM) and Phase 3 (transport) ---

    pub fn body(&self) -> &B {
        &self.inner.body
    }

    pub fn from_principal(&self) -> &Principal {
        &self.inner.from
    }

    pub fn to_principal(&self) -> &Principal {
        &self.inner.to
    }

    pub fn id(&self) -> &MessageId {
        &self.inner.id
    }

    pub fn class(&self) -> MessageClass {
        self.inner.class
    }

    pub fn scope(&self) -> EnvelopeScope {
        self.inner.scope
    }

    pub fn authority(&self) -> AuthorityScope {
        self.inner.authority
    }

    pub fn ts(&self) -> &Timestamp {
        &self.inner.ts
    }

    pub fn causality(&self) -> Option<&Causality> {
        self.inner.causality.as_ref()
    }

    pub fn terminal_status(&self) -> Option<&TerminalStatus> {
        self.inner.terminal_status.as_ref()
    }

    pub fn signature(&self) -> &FampSignature {
        &self.signature
    }

    pub fn inner(&self) -> &UnsignedEnvelope<B> {
        &self.inner
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::body::AckBody;

    // RFC 8032 Test 1 keypair (matches §7.1c).
    const SECRET: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];
    const PUBLIC: [u8; 32] = [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ];

    const VECTOR_0_BYTES: &[u8] = include_bytes!("../tests/vectors/vector_0/envelope.json");

    fn test1_keys() -> (FampSigningKey, TrustedVerifyingKey) {
        (
            FampSigningKey::from_bytes(SECRET),
            TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap(),
        )
    }

    #[test]
    fn sign_consumes_unsigned_and_returns_signed() {
        let (sk, _vk) = test1_keys();
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap();
        let from: Principal = "agent:example.test/alice".parse().unwrap();
        let to: Principal = "agent:example.test/bob".parse().unwrap();
        let ts = Timestamp("2026-04-13T00:00:00Z".to_string());
        let body = AckBody {
            disposition: crate::body::AckDisposition::Accepted,
            reason: None,
        };
        let unsigned =
            UnsignedEnvelope::<AckBody>::new(id, from, to, AuthorityScope::Advisory, ts, body);
        let signed = unsigned.sign(&sk).unwrap();
        assert_eq!(signed.class(), MessageClass::Ack);
    }

    #[test]
    fn vector_0_decodes_through_typed_signed_envelope() {
        let (_, vk) = test1_keys();
        let signed = SignedEnvelope::<AckBody>::decode(VECTOR_0_BYTES, &vk).unwrap();
        assert_eq!(
            signed.body().disposition,
            crate::body::AckDisposition::Accepted
        );
        assert_eq!(signed.class(), MessageClass::Ack);
        assert_eq!(signed.scope(), EnvelopeScope::Standalone);
    }

    #[test]
    fn vector_0_missing_signature_field_rejected() {
        let (_, vk) = test1_keys();
        let mut value: Value = serde_json::from_slice(VECTOR_0_BYTES).unwrap();
        value.as_object_mut().unwrap().remove("signature");
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk).unwrap_err();
        assert!(
            matches!(err, EnvelopeDecodeError::MissingSignature),
            "expected MissingSignature, got {err:?}"
        );
    }

    #[test]
    fn vector_0_tampered_last_byte_fails_typed() {
        let (_, vk) = test1_keys();
        // Flip a byte inside the signature value so the JSON still parses but
        // verify_strict rejects. The last byte of the file is `\n` or `}` —
        // flipping it may break JSON parsing; target the signature string
        // instead so we exercise SignatureInvalid not MalformedJson.
        let mut value: Value = serde_json::from_slice(VECTOR_0_BYTES).unwrap();
        let sig = value
            .get("signature")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        // Replace the middle character with a different valid base64url char
        // to produce a structurally valid b64url string that decodes to a
        // well-formed but wrong signature.
        let mid = sig.len() / 2;
        let orig = sig.as_bytes()[mid];
        let new_byte = if orig == b'A' { b'B' } else { b'A' };
        let mut tampered_bytes = sig.into_bytes();
        tampered_bytes[mid] = new_byte;
        let tampered = String::from_utf8(tampered_bytes).unwrap();
        *value.get_mut("signature").unwrap() = Value::String(tampered);
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk).unwrap_err();
        assert!(
            matches!(
                err,
                EnvelopeDecodeError::SignatureInvalid
                    | EnvelopeDecodeError::InvalidSignatureEncoding(_)
                    | EnvelopeDecodeError::MalformedJson(_)
            ),
            "expected signature/parse failure, got {err:?}"
        );
    }

    #[test]
    fn pitfall_p3_verify_operates_on_raw_value_not_typed_struct() {
        // Inject an unknown TOP-LEVEL envelope field AND re-sign so that
        // verification succeeds over the full (including unknown) Value.
        // WireEnvelope<B> has deny_unknown_fields, so we expect the typed
        // deserialize step to surface `UnknownEnvelopeField { field: "x_future" }`
        // — NOT SignatureInvalid. That proves Pitfall P3 is locked: verify
        // ran over the raw Value (including the unknown field) and succeeded,
        // THEN typed decode caught it afterwards.
        let (sk, vk) = test1_keys();
        let mut value: Value = serde_json::from_slice(VECTOR_0_BYTES).unwrap();
        value.as_object_mut().unwrap().remove("signature");
        value
            .as_object_mut()
            .unwrap()
            .insert("x_future".to_string(), Value::from(1));
        let sig = sign_value(&sk, &value).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("signature".to_string(), Value::String(sig.to_b64url()));
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = SignedEnvelope::<AckBody>::decode(&bytes, &vk).unwrap_err();
        assert!(
            matches!(err, EnvelopeDecodeError::UnknownEnvelopeField { ref field } if field == "x_future"),
            "expected UnknownEnvelopeField {{ x_future }}, got {err:?}"
        );
    }
}
