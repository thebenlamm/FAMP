//! `MessageClass` — the six v0.5.2 message classes.
//!
//! Wire form is `snake_case` per §7.1c.2. Narrowed per CONTEXT.md D-B2:
//! `announce`, `describe`, `propose`, `delegate`, `supersede`, `close` are
//! NOT variants in v0.7 — they defer to Federation Profile (v0.8+).

use std::fmt;

/// The six v0.5.2 message classes.
///
/// Sealed via CONTEXT.md D-B2: downstream crates cannot invent new classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MessageClass {
    Request,
    Commit,
    Deliver,
    Ack,
    Control,
    AuditLog,
}

impl fmt::Display for MessageClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Request => "request",
            Self::Commit => "commit",
            Self::Deliver => "deliver",
            Self::Ack => "ack",
            Self::Control => "control",
            Self::AuditLog => "audit_log",
        };
        f.write_str(s)
    }
}
