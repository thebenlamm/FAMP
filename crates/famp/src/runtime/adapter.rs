//! Envelope -> FSM input adapter. The ~20-line function Phase 2 D-D3
//! committed to. Returns `None` for `ack` and `audit_log` classes (D-D4/D-15:
//! wire-only/non-FSM-firing classes never enter the FSM).

use famp_core::{MessageClass, Principal, TerminalStatus};
use famp_envelope::AnySignedEnvelope;
use famp_fsm::TaskTransitionInput;

/// Derive the FSM transition input from a decoded envelope.
///
/// Returns `None` when the envelope is `ack` (D-D4 — ack is wire-only),
/// `audit_log` (D-15 — store-only, non-FSM-firing), OR
/// `request` (Plan 03-04 Rule-1 fix — the 5-state FSM starts in `Requested`
/// and has no transition consuming a `request` input; the initial state IS
/// the creation event, so receiving the request message is a no-op at the
/// FSM layer and only commit/deliver/control actually drive transitions).
/// Returns `Some(input)` for `commit`, `deliver`, and `control`.
#[must_use]
pub fn fsm_input_from_envelope(env: &AnySignedEnvelope) -> Option<TaskTransitionInput> {
    let (class, terminal_status): (MessageClass, Option<TerminalStatus>) = match env {
        AnySignedEnvelope::Commit(e) => (e.class(), e.terminal_status().copied()),
        AnySignedEnvelope::Deliver(e) => (e.class(), e.terminal_status().copied()),
        AnySignedEnvelope::Control(e) => (e.class(), e.terminal_status().copied()),
        AnySignedEnvelope::Request(_)
        | AnySignedEnvelope::Ack(_)
        | AnySignedEnvelope::AuditLog(_) => return None,
    };
    Some(TaskTransitionInput {
        class,
        terminal_status,
    })
}

/// Helper for the recipient cross-check. Delegates to the inner
/// `SignedEnvelope<B>::to_principal()` since `AnySignedEnvelope` does not
/// expose a direct accessor.
#[must_use]
pub fn envelope_recipient(env: &AnySignedEnvelope) -> &Principal {
    match env {
        AnySignedEnvelope::Request(e) => e.to_principal(),
        AnySignedEnvelope::Commit(e) => e.to_principal(),
        AnySignedEnvelope::Deliver(e) => e.to_principal(),
        AnySignedEnvelope::Ack(e) => e.to_principal(),
        AnySignedEnvelope::Control(e) => e.to_principal(),
        AnySignedEnvelope::AuditLog(e) => e.to_principal(),
    }
}

/// Helper for the sender cross-check and trace logging.
#[must_use]
pub fn envelope_sender(env: &AnySignedEnvelope) -> &Principal {
    match env {
        AnySignedEnvelope::Request(e) => e.from_principal(),
        AnySignedEnvelope::Commit(e) => e.from_principal(),
        AnySignedEnvelope::Deliver(e) => e.from_principal(),
        AnySignedEnvelope::Ack(e) => e.from_principal(),
        AnySignedEnvelope::Control(e) => e.from_principal(),
        AnySignedEnvelope::AuditLog(e) => e.from_principal(),
    }
}

#[must_use]
pub fn envelope_class(env: &AnySignedEnvelope) -> MessageClass {
    match env {
        AnySignedEnvelope::Request(e) => e.class(),
        AnySignedEnvelope::Commit(e) => e.class(),
        AnySignedEnvelope::Deliver(e) => e.class(),
        AnySignedEnvelope::Ack(e) => e.class(),
        AnySignedEnvelope::Control(e) => e.class(),
        AnySignedEnvelope::AuditLog(e) => e.class(),
    }
}
