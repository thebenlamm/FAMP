//! `famp-transport-http` — FAMP v0.5.1 HTTP transport binding.

#![forbid(unsafe_code)]

// Silencers for dependencies consumed in later Plan 04-03 tasks but not
// yet wired at this Wave 2 point. Remove from lib.rs as each Task lands.
use rustls_platform_verifier as _;
use rustls_pemfile as _;
use rustls as _;
use axum_server as _;
use famp_crypto as _;
use serde_json as _;

pub mod error;
pub mod middleware;
pub mod server;

pub use error::{HttpTransportError, MiddlewareError};
pub use middleware::FampSigVerifyLayer;
pub use server::{build_router, InboxRegistry, ServerState, INBOX_ROUTE};

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn middleware_error_status_mapping_is_load_bearing() {
        // D-C6 status mapping is the load-bearing distinguishability test for
        // CONF-05/06/07 at the HTTP layer. If this regresses, the adversarial
        // matrix collapses.
        assert_eq!(MiddlewareError::BodyTooLarge.into_response().status().as_u16(), 413);
        assert_eq!(MiddlewareError::BadPrincipal.into_response().status().as_u16(), 400);
        assert_eq!(MiddlewareError::BadEnvelope.into_response().status().as_u16(), 400);
        assert_eq!(MiddlewareError::CanonicalDivergence.into_response().status().as_u16(), 400);
        assert_eq!(MiddlewareError::UnknownSender.into_response().status().as_u16(), 401);
        assert_eq!(MiddlewareError::SignatureInvalid.into_response().status().as_u16(), 401);
        assert_eq!(MiddlewareError::UnknownRecipient.into_response().status().as_u16(), 404);
        assert_eq!(MiddlewareError::Internal.into_response().status().as_u16(), 500);
    }
}
