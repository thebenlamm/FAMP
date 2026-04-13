//! Documentation anchors for the 11 protocol invariants from FAMP v0.5.1 §3.
//!
//! Each `INV_N` constant carries the invariant statement as its rustdoc;
//! downstream crates can intra-doc-link (e.g. `` [`famp_core::invariants::INV_10`] ``)
//! to pin their enforcement point to the normative text. The constant values
//! are deliberately minimal — the payload is the doc comment.
//!
//! These anchors are verified present and non-empty by
//! `tests/invariants_present.rs`. Enforcement of each invariant lives in the
//! crate that actually models the behavior (envelope, fsm, transport);
//! `famp-core` ships only the anchors.

/// INV-1: Every message identifies exactly one sender principal and one sender
/// instance; the two together plus message id form a unique triple.
pub const INV_1: &str = "INV-1";

/// INV-2: Every message belongs to exactly one scope (standalone, conversation,
/// or task), and scope cannot change mid-message.
pub const INV_2: &str = "INV-2";

/// INV-3: Every causal relation from a message points to a prior message the
/// sender has observed.
pub const INV_3: &str = "INV-3";

/// INV-4: Commitments are bound to exactly one proposal via `commits_against`;
/// commit-without-proposal is rejected.
pub const INV_4: &str = "INV-4";

/// INV-5: Terminal states are absorbing — once a task or conversation enters a
/// terminal state, no further transitions are valid. Enforced at compile time
/// via exhaustive enum `match`.
pub const INV_5: &str = "INV-5";

/// INV-6: Acks classify outcome with one of six dispositions; every non-ack
/// message is eligible for exactly one ack from the recipient.
pub const INV_6: &str = "INV-6";

/// INV-7: Freshness windows apply per message class; stale messages are
/// rejected at ingress before any state transition.
pub const INV_7: &str = "INV-7";

/// INV-8: Extensions may not redefine core semantics; unknown critical
/// extensions are fail-closed.
pub const INV_8: &str = "INV-8";

/// INV-9: Unknown critical extensions cause the carrying message to be
/// rejected; unknown non-critical extensions are ignored.
pub const INV_9: &str = "INV-9";

/// INV-10: Every message must be signed; unsigned messages are rejected on
/// decode. This is the non-negotiable signature gate.
pub const INV_10: &str = "INV-10";

/// INV-11: Negotiation rounds are bounded (default 20); exceeding the ceiling
/// terminates the conversation with `capacity_exceeded`.
pub const INV_11: &str = "INV-11";
