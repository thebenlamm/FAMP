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
