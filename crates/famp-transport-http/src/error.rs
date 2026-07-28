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
    #[error("reqwest failure: {0}")]
    ReqwestFailed(#[source] reqwest::Error),
    #[error("server returned status {code}: {body}")]
    ServerStatus { code: u16, body: String },
    #[error("inbox closed for principal: {principal}")]
    InboxClosed { principal: Principal },
    #[error("invalid url: {0}")]
    InvalidUrl(#[source] url::ParseError),
    #[error("tls config error: {0}")]
    TlsConfig(String),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// OBS-01: a `ReqwestFailed` Display must be a strict superset of the
    /// bare fixed prefix — it must interpolate the wrapped `reqwest::Error`
    /// text (the underlying cause), not just say "reqwest failure".
    /// Constructing a real `reqwest::Error` synchronously (no network I/O):
    /// `Client::get` with an unparseable URL stores the parse failure and
    /// only surfaces it when `.send()` is awaited.
    #[tokio::test]
    async fn reqwest_failed_display_contains_source_text() {
        let inner_err = reqwest::Client::new()
            .get("not a url")
            .send()
            .await
            .expect_err("an unparseable URL must fail at send()");
        let inner_text = inner_err.to_string();

        let wrapped = HttpTransportError::ReqwestFailed(inner_err);
        let displayed = wrapped.to_string();

        assert_ne!(
            displayed, "reqwest failure",
            "Display must not be the bare fixed prefix"
        );
        assert!(
            displayed.contains(&inner_text),
            "Display {displayed:?} must contain the wrapped reqwest error text {inner_text:?}"
        );
    }

    /// Sibling variant: `InvalidUrl` must likewise surface the underlying
    /// `url::ParseError` text, not just a fixed "invalid url" prefix.
    #[test]
    fn invalid_url_display_contains_source_text() {
        let inner_err = "not a url"
            .parse::<url::Url>()
            .expect_err("must fail to parse as a URL");
        let inner_text = inner_err.to_string();

        let wrapped = HttpTransportError::InvalidUrl(inner_err);
        let displayed = wrapped.to_string();

        assert_ne!(displayed, "invalid url");
        assert!(displayed.contains(&inner_text));
    }
}
