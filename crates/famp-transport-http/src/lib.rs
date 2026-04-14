//! `famp-transport-http` — FAMP v0.5.1 HTTP transport binding.

#![forbid(unsafe_code)]

// Silencers for dependencies still pending wiring after Plan 04-03. As each
// later plan lands, remove the matching `use _ as _;` line.
use famp_crypto as _;
use serde_json as _;

pub mod error;
pub mod middleware;
pub mod server;
pub mod tls;
pub mod tls_server;

pub use error::{HttpTransportError, MiddlewareError};
pub use middleware::FampSigVerifyLayer;
pub use server::{build_router, InboxRegistry, ServerState, INBOX_ROUTE};
pub use tls::{build_client_config, build_server_config, load_pem_cert, load_pem_key, TlsError};

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
