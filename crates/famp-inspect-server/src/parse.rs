//! Envelope-field and timestamp parsers shared across the inspect handlers.
//!
//! Pure functions over already-parsed `serde_json::Value` envelopes plus the
//! RFC3339 / `SystemTime` epoch helpers. Extracted from `lib.rs` so each
//! inspect-kind handler (`tasks`, `messages`, …) depends on one parsing module
//! instead of the dispatcher carrying parser utilities.

use std::time::{SystemTime, UNIX_EPOCH};

use famp_envelope::EnvelopeView;

/// Derive FSM state from envelope fields using canonical class strings
/// and `famp_core::TerminalStatus` `snake_case` mode strings.
pub fn derive_fsm_state(env: &serde_json::Value) -> String {
    let view = EnvelopeView::new(env);
    let class = view.class().unwrap_or("");
    let details = view.body().and_then(|b| b.get("details"));
    let mode = details
        .and_then(|d| d.get("mode"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let terminal = details
        .and_then(|d| d.get("terminal"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let action = details
        .and_then(|d| d.get("action"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    // FSM truth table — keep each (class, mode, terminal, action) arm explicit
    // so protocol extensions see the full decision surface.
    #[allow(clippy::match_same_arms)]
    match (class, mode, terminal, action) {
        ("request", _, _, _) => "REQUESTED".into(),
        ("commit", _, _, _) => "COMMITTED".into(),
        ("deliver", "completed", true, _) => "COMPLETED".into(),
        ("deliver", "failed", true, _) => "FAILED".into(),
        ("deliver", "cancelled", true, _) => "CANCELLED".into(),
        ("deliver", _, true, _) => "COMPLETED".into(),
        ("deliver", _, false, _) => "COMMITTED".into(),
        ("control", "cancelled", _, _) => "CANCELLED".into(),
        ("control", _, _, "cancel") => "CANCELLED".into(),
        ("control", _, _, _) => "CANCELLED".into(),
        _ => "UNKNOWN".into(),
    }
}

/// Best-effort RFC3339 -> epoch seconds.
pub fn parse_rfc3339_to_epoch(s: &str) -> Option<u64> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .ok()
        .and_then(|dt| u64::try_from(dt.unix_timestamp()).ok())
}

pub fn to_epoch_seconds(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
