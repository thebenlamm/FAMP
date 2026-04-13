//! # v0.7 narrowing — ENV-12 cancel-only
//!
//! The full v0.5.1 §8a.4 catalog lists FIVE actions:
//! `{cancel, supersede, close, cancel_if_not_started, revert_transfer}`.
//! v0.7 Personal Profile exposes ONLY `cancel`. The other four are not variants
//! on `ControlAction` — they literally cannot be constructed or deserialized.
//! Adding one is a v0.8+ breaking change. Do NOT add `#[serde(other)]` or a
//! catch-all variant — that would silently re-open the narrowing.

use crate::body::BodySchema;
use crate::{EnvelopeScope, MessageClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlBody {
    pub target: ControlTarget,
    pub action: ControlAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<ControlDisposition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_ids: Option<Vec<String>>,
}

/// Single-variant enum. v0.8+ adds `supersede`, `close`, `cancel_if_not_started`,
/// `revert_transfer`. Do NOT add `#[serde(other)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlAction {
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlTarget {
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlDisposition {
    Accepted,
    Rejected,
}

impl BodySchema for ControlBody {
    const CLASS: MessageClass = MessageClass::Control;
    const SCOPE: EnvelopeScope = EnvelopeScope::Task;
}
