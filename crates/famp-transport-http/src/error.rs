//! Phase-local error enums for famp-transport-http.
//!
//! `MiddlewareError`     — server-side rejections from the sig-verify / body-limit tower stack (D-C7).
//! `HttpTransportError`  — client-side `Transport::Error` associated type (D-C8).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use famp_core::Principal;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum MiddlewareError {
    #[error("body too large")]
    BodyTooLarge,
    #[error("bad principal in path")]
    BadPrincipal,
    #[error("bad envelope")]
    BadEnvelope,
    #[error("canonical divergence")]
    CanonicalDivergence,
    #[error("unknown sender (no pinned key)")]
    UnknownSender,
    #[error("signature invalid")]
    SignatureInvalid,
    #[error("unknown recipient")]
    UnknownRecipient,
    #[error("internal error")]
    Internal,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    detail: String,
}

impl IntoResponse for MiddlewareError {
    fn into_response(self) -> Response {
        let (code, slug) = match self {
            Self::BodyTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "body_too_large"),
            Self::BadPrincipal => (StatusCode::BAD_REQUEST, "bad_principal"),
            Self::BadEnvelope => (StatusCode::BAD_REQUEST, "bad_envelope"),
            Self::CanonicalDivergence => (StatusCode::BAD_REQUEST, "canonical_divergence"),
            Self::UnknownSender => (StatusCode::UNAUTHORIZED, "unknown_sender"),
            Self::SignatureInvalid => (StatusCode::UNAUTHORIZED, "signature_invalid"),
            Self::UnknownRecipient => (StatusCode::NOT_FOUND, "unknown_recipient"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let body = ErrorBody {
            error: slug,
            detail: self.to_string(),
        };
        (code, Json(body)).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpTransportError {
    #[error("unknown recipient: {principal}")]
    UnknownRecipient { principal: Principal },
    #[error("reqwest failure")]
    ReqwestFailed(#[source] reqwest::Error),
    #[error("server returned status {code}: {body}")]
    ServerStatus { code: u16, body: String },
    #[error("inbox closed for principal: {principal}")]
    InboxClosed { principal: Principal },
    #[error("invalid url")]
    InvalidUrl(#[source] url::ParseError),
    #[error("tls config error: {0}")]
    TlsConfig(String),
}
