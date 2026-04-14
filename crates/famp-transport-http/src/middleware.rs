//! Pre-routing signature-verification middleware (D-C1, D-C2, D-C3, TRANS-09).
//!
//! Mirrors `crates/famp/src/runtime/loop_fn.rs` byte-for-byte on the decode
//! path so both transports produce identical distinguishable errors for
//! CONF-05/06/07.

use std::{
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    body::{to_bytes, Body},
    http::Request,
    response::{IntoResponse, Response},
};
use famp_canonical::{canonicalize, from_slice_strict};
use famp_envelope::{peek_sender, AnySignedEnvelope, EnvelopeDecodeError};
use famp_keyring::Keyring;
use futures_util::future::BoxFuture;
use tower::{Layer, Service};

use crate::error::MiddlewareError;

const ONE_MIB: usize = 1_048_576;

#[derive(Clone)]
pub struct FampSigVerifyLayer {
    keyring: Arc<Keyring>,
}

impl FampSigVerifyLayer {
    #[must_use]
    pub const fn new(keyring: Arc<Keyring>) -> Self {
        Self { keyring }
    }
}

impl<S> Layer<S> for FampSigVerifyLayer {
    type Service = FampSigVerifyService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        FampSigVerifyService {
            inner,
            keyring: self.keyring.clone(),
        }
    }
}

#[derive(Clone)]
pub struct FampSigVerifyService<S> {
    inner: S,
    keyring: Arc<Keyring>,
}

impl<S> Service<Request<Body>> for FampSigVerifyService<S>
where
    S: Service<Request<Body>, Response = Response, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Clone-and-replace ready-pattern (the canonical tower bug prevention).
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let keyring = self.keyring.clone();
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            // Body already capped by outer RequestBodyLimitLayer; belt-and-braces cap here.
            let Ok(bytes) = to_bytes(body, ONE_MIB).await else {
                return Ok(MiddlewareError::BodyTooLarge.into_response());
            };

            // Step 1: peek sender.
            let Ok(sender) = peek_sender(&bytes) else {
                return Ok(MiddlewareError::BadEnvelope.into_response());
            };

            // Step 2: keyring lookup.
            let Some(pinned) = keyring.get(&sender).cloned() else {
                return Ok(MiddlewareError::UnknownSender.into_response());
            };

            // Step 3: canonical pre-check (CONF-07 distinguishability).
            // Mirrors loop_fn.rs exactly: from_slice_strict -> canonicalize -> compare.
            //
            // INVARIANT (MED-02): the Value-based canonicalize() performed here
            // MUST stay byte-identical to whatever canonical form
            // AnySignedEnvelope::decode validates against in Step 4. Any future
            // serde-layer refactor that breaks this symmetry would silently
            // desynchronize the two paths and make CONF-07
            // (canonical_divergence vs signature_invalid) indistinguishable
            // from the wire side. If you touch either path, update the
            // `canonical_pre_check_*` unit tests below to pin the new
            // equivalence.
            let Ok(parsed) = from_slice_strict(&bytes) else {
                return Ok(MiddlewareError::BadEnvelope.into_response());
            };
            let parsed: serde_json::Value = parsed;
            let Ok(re_canonical) = canonicalize(&parsed) else {
                return Ok(MiddlewareError::BadEnvelope.into_response());
            };
            if re_canonical.as_slice() != bytes.as_ref() {
                return Ok(MiddlewareError::CanonicalDivergence.into_response());
            }

            // Step 4: decode + verify.
            let envelope = match AnySignedEnvelope::decode(&bytes, &pinned) {
                Ok(e) => e,
                Err(
                    EnvelopeDecodeError::SignatureInvalid
                    | EnvelopeDecodeError::InvalidSignatureEncoding(_),
                ) => {
                    return Ok(MiddlewareError::SignatureInvalid.into_response());
                }
                Err(_) => return Ok(MiddlewareError::BadEnvelope.into_response()),
            };

            // Step 5: stash envelope + re-attach body (Pitfall 1).
            let mut req = Request::from_parts(parts, Body::from(bytes));
            req.extensions_mut().insert(Arc::new(envelope));
            inner.call(req).await
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod canonical_pre_check_tests {
    //! MED-02 parity pin: the Value-based canonicalize() path in `call()`
    //! must be byte-identical to what a typed decode would re-canonicalize
    //! to. We don't re-run the middleware here; we verify the underlying
    //! primitive (`from_slice_strict::<Value>` -> `canonicalize`) handles
    //! the same edge cases the typed path does, so a future refactor that
    //! diverges the two will trip a unit test before it ships.
    use super::{canonicalize, from_slice_strict};
    use serde_json::Value;

    fn round_trip(input: &[u8]) -> Vec<u8> {
        let v: Value = from_slice_strict(input).expect("strict parse");
        canonicalize(&v).expect("canonicalize")
    }

    #[test]
    fn canonical_pre_check_roundtrip_ascii() {
        // Already-canonical bytes: sorted keys, no whitespace.
        let bytes = br#"{"a":1,"b":2}"#;
        assert_eq!(round_trip(bytes).as_slice(), bytes.as_ref());
    }

    #[test]
    fn canonical_pre_check_roundtrip_unicode_bmp() {
        // RFC 8785 §3.2.2: non-ASCII code points pass through as UTF-8
        // bytes (no \uXXXX escaping). Catches any serde-layer flag flip
        // that would re-escape strings.
        let bytes = "{\"k\":\"héllo→\"}".as_bytes();
        assert_eq!(round_trip(bytes), bytes);
    }

    #[test]
    fn canonical_pre_check_rejects_duplicate_keys() {
        // Strict parse MUST reject duplicates before canonicalization
        // runs, so the middleware collapses to BadEnvelope rather than
        // silently picking a winning key. Pins from_slice_strict's
        // duplicate-key-rejection contract against the middleware's
        // assumption.
        let bytes = br#"{"a":1,"a":2}"#;
        let r: Result<Value, _> = from_slice_strict(bytes);
        assert!(r.is_err(), "duplicate keys must be rejected");
    }

    #[test]
    fn canonical_pre_check_whitespace_diverges() {
        // Whitespace-bearing input parses fine but re-canonicalization
        // strips it, so the middleware flags CanonicalDivergence. Pins
        // the byte-level inequality the middleware's `!=` check relies
        // on.
        let bytes = br#"{ "a" : 1 }"#;
        let canon = round_trip(bytes);
        assert_ne!(canon.as_slice(), bytes.as_ref());
        assert_eq!(canon.as_slice(), br#"{"a":1}"# as &[u8]);
    }

    #[test]
    fn canonical_pre_check_integer_number_formatting() {
        // RFC 8785 number canonicalization edge case: integer-valued
        // numbers serialize WITHOUT trailing `.0`. If a future serde
        // refactor enables `arbitrary_precision` or similar, this will
        // start emitting `1e0` or `1.0` and the assertion fails.
        let bytes = br#"{"n":1}"#;
        assert_eq!(round_trip(bytes).as_slice(), bytes.as_ref());
    }

    #[test]
    fn canonical_pre_check_key_sorting() {
        // Keys must sort lexicographically by UTF-16 code unit per RFC
        // 8785 §3.2.3. Input with reversed key order canonicalizes to
        // sorted form.
        let bytes = br#"{"b":2,"a":1}"#;
        assert_eq!(round_trip(bytes).as_slice(), br#"{"a":1,"b":2}"# as &[u8]);
    }
}
