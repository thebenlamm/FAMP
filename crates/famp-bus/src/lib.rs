//! famp-bus - v0.9 local-first bus protocol primitives.
//!
//! Layer 1 substrate: transport-neutral types, length-prefixed canonical-JSON
//! frame codec, pure broker actor, in-memory mailbox.
//!
//! INVARIANT (BUS-01 / BUS-09): NO `tokio` in the runtime dependency tree.
//! `cargo tree -p famp-bus --edges normal | grep tokio` MUST be empty;
//! `just check-no-tokio-in-bus` enforces this. Async lives in Phase 2's
//! wire layer, never here.
//!
//! INVARIANT (BUS-11): bus-side envelopes carry NO signature. The
//! type-level enforcement lives in `famp-envelope::bus::BusEnvelope`
//! (added in Plan 01-03's atomic v0.5.2 commit). Bus ↔ federation
//! translation table:
//! `docs/superpowers/specs/2026-04-30-bus11-translation-table.md`.
//!
//! CONVENTION (BUS-11 / v0.9): the bus path carries DMs, deliveries, and
//! channel posts as **unsigned `audit_log` envelopes** with event prefix
//! `famp.send.*` and a mode-tagged inner payload under `body.details`
//! (`mode ∈ {new_task, deliver, deliver_terminal, channel_post}`). This
//! is bus-only — no `famp.send.*` envelope ever crosses the federation
//! gateway. Real `audit_log` envelopes (non-`famp.send.*` event) retain
//! their actual audit-event semantics on both surfaces. The gateway
//! disambiguates by `body.event` (architect-counseled, Option 1+).
//! Single construction site: `famp::cli::send::build_envelope_value`.
//! Drift constraints in
//! `docs/superpowers/specs/2026-04-30-bus11-translation-table.md`
//! "Drift Invariant" — any second construction site or new event prefix
//! requires a translation-table amendment in the same commit.
//!
//! CARRY-04: Nyquist VALIDATION.md backfill for v0.8 phases is formally
//! deferred to the v0.9 milestone-close audit per D-18.

#![forbid(unsafe_code)]

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib compile unit.
use famp_core as _;
use famp_envelope as _;
#[cfg(test)]
use proptest as _;
// `tokio` is a dev-only dep added in Phase 2 plan 02-01 for `start_paused`
// time-forward tests; the lib unit-test compile unit doesn't reference it
// directly, so silence the workspace lint here.
#[cfg(test)]
use tokio as _;

pub mod broker;
pub mod codec;
pub mod env;
pub mod error;
pub mod liveness;
pub mod mailbox;
pub mod proto;

pub use broker::{Broker, BrokerInput, Out};
pub use codec::{encode_frame, try_decode_frame, FrameError, LEN_PREFIX_BYTES, MAX_FRAME_BYTES};
pub use env::BrokerEnv;
pub use error::BusErrorKind;
pub use famp_envelope::bus::{AnyBusEnvelope, BusEnvelope};
pub use liveness::{AlwaysAliveLiveness, FakeLiveness, LivenessProbe};
pub use mailbox::{DrainResult, InMemoryMailbox, MailboxErr, MailboxName, MailboxRead};
pub use proto::{AwaitFilter, BusMessage, BusReply, ClientId, Delivered, SessionRow, Target};
