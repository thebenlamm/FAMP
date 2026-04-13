//! `AckBody` — FAMP v0.5.1 §7.1c.2 ack body.
//!
//! Scope locked to `Standalone` in v0.7 to match vector 0 byte-for-byte. Phase 2 FSM
//! may re-scope to `Task`; that is a v0.7 review item documented in CONTEXT.md D-C4.

use crate::body::BodySchema;
use crate::{EnvelopeScope, MessageClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AckBody {
    pub disposition: AckDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AckDisposition {
    Accepted,
    Rejected,
    Received,
    Completed,
    Failed,
    Cancelled,
}

impl BodySchema for AckBody {
    const CLASS: MessageClass = MessageClass::Ack;
    const SCOPE: EnvelopeScope = EnvelopeScope::Standalone;
}
