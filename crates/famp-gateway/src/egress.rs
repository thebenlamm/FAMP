//! Outbound drain-then-relay: mailbox -> wire (Phase 9 D-01/D-03).
//!
//! Awaits/drains each locally-backed remote principal's mailbox via its
//! own [`crate::ProxiedPrincipal::send_recv`] connection, mutates the
//! drained `serde_json::Value` in place to add the outer federation
//! fields (`from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry`),
//! signs the mutated value with the gateway's own persisted key, and
//! POSTs the resulting bytes to the remote gateway's inbox via
//! `famp_transport_http::HttpTransport`. Content-transparent: `task_id`,
//! `class`, and `body` are never rewritten (09-RESEARCH.md §3/§5).
//!
//! Body lands in 09-02.
