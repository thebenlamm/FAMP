//! `famp-relay` — the store-and-forward relay Phase 13 chose for REACH-04:
//! a publicly-addressable box both gateways reach OUTBOUND (POST to
//! enqueue, GET to fetch) when they are both behind NAT and neither can
//! accept the other's inbound connection.
//!
//! Five sentences are this crate's whole contract, and every module below
//! exists to keep exactly these five true:
//!
//! 1. It forwards message bytes it never parses; the destination comes
//!    from the URL path, never from the body ([`queue`], [`http`]).
//! 2. It makes exactly ONE trust decision — whether the caller presenting
//!    a fetch is authorized to drain that domain's queue — and it makes
//!    that decision only against public keys an operator configured in
//!    advance ([`fetch_auth`]).
//! 3. It makes NO trust decision about any message: the receiving
//!    gateway's inbound envelope check against its own pinned keyring
//!    remains the sole such decision (D-02). This crate never verifies,
//!    re-signs, strips, or rewrites an envelope.
//! 4. **It is not confidential.** FAMP signs but does not encrypt (D-25);
//!    TLS terminates here, so the operator of this box can read every
//!    queued body in plaintext. Never describe this crate as private,
//!    zero-knowledge, or trustless — anywhere, in any doc this crate's
//!    behavior is cited from.
//! 5. Its state is [`queue::RelayQueues`] plus [`fetch_auth::FetchAuthState`],
//!    both in memory with no disk I/O, so Phase 16's pairing rendezvous
//!    can be added as a module with its own disjoint state (D-14) —
//!    state boundaries here are load-bearing for that future addition,
//!    not an aesthetic preference.

#![forbid(unsafe_code)]

// Silencer: `reqwest` and `assert_cmd` are dev-dependencies consumed
// only by `tests/relay_store_and_forward.rs` (this crate's live-process
// test client and subprocess spawner) — a separate compilation unit
// from this lib target's own `#[cfg(test)]` unit tests.
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use reqwest as _;

pub mod fetch_auth;
pub mod http;
pub mod queue;
