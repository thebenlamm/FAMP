//! `TerminalStatus` — terminal delivery status carried on the envelope header.
//!
//! Defined in `famp-core` so `famp-fsm` can consume it without depending on
//! `famp-envelope` (crate layering decision D-D1).

/// Terminal status carried on the envelope header for terminal deliveries.
///
/// Wire form is `snake_case` per §8a.3. Exactly three variants — `Cancelled`
/// arrives via a `control` message, not a `deliver`, but is included here so
/// the FSM and envelope layers share a single type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalStatus {
    Completed,
    Failed,
    Cancelled,
}
