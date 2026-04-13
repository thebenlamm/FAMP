//! `RequestBody` — FAMP v0.5.1 §7.4 request body.
//!
//! Locked to `EnvelopeScope::Standalone` per CONTEXT.md D-C3: v0.7 Personal Profile
//! does not plumb conversation IDs through Phase 1 envelope logic.

use crate::body::{bounds::Bounds, BodySchema};
use crate::{EnvelopeDecodeError, EnvelopeScope, MessageClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBody {
    pub scope: serde_json::Value,
    pub bounds: Bounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natural_language_summary: Option<String>,
}

impl RequestBody {
    #[allow(dead_code)] // wired by Plan 03 decode pipeline
    pub(crate) fn validate(&self) -> Result<(), EnvelopeDecodeError> {
        self.bounds.validate()
    }
}

impl BodySchema for RequestBody {
    const CLASS: MessageClass = MessageClass::Request;
    // D-C3: v0.7 locks request to Standalone. v0.8+ negotiation/causality may re-scope.
    const SCOPE: EnvelopeScope = EnvelopeScope::Standalone;
}
