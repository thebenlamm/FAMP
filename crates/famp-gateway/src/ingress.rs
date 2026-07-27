//! Inbound HTTP listener: wire -> local bus (Phase 9 D-04/D-05).
//!
//! A gateway-owned axum router (NOT `famp_transport_http::build_router`,
//! whose `FampSigVerifyLayer` would introduce a second trust source —
//! 09-RESEARCH.md §3 Pitfall 1) that extracts the raw request body,
//! verifies it with [`crate::verify::verify_inbound_any`] against the
//! gateway's own peers keyring, and on success delivers the verified
//! envelope onto the local bus via the backed *sender* stand-in's
//! [`crate::ProxiedPrincipal::send_recv`] (D-05). A rejected envelope
//! produces zero bus writes and surfaces as an HTTP 4xx.
//!
//! Body lands in 09-03.
