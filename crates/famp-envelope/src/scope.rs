//! `EnvelopeScope` — the three §7.1 scope values.
//!
//! Wire form is `snake_case` per §7.1c.2. Per CONTEXT.md D-C3, `request` is
//! locked to `Standalone` in v0.7 Personal Runtime.

use std::fmt;

/// FAMP envelope scope (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvelopeScope {
    Standalone,
    Conversation,
    Task,
}

impl fmt::Display for EnvelopeScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Standalone => "standalone",
            Self::Conversation => "conversation",
            Self::Task => "task",
        };
        f.write_str(s)
    }
}
