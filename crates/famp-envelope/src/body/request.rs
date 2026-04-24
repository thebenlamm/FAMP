//! `RequestBody` — FAMP v0.5.1 §7.4 request body.
//!
//! Locked to `EnvelopeScope::Standalone` per CONTEXT.md D-C3: v0.7 Personal Profile
//! does not plumb conversation IDs through Phase 1 envelope logic.

use crate::body::{bounds::Bounds, BodySchema};
use crate::{EnvelopeDecodeError, EnvelopeScope, MessageClass};
use serde::{Deserialize, Serialize};

/// Key under `RequestBody.scope` where the sender's prose task content
/// is placed when `famp send --new-task --body <prose>` is invoked.
///
/// PROVISIONAL convention — see
/// `docs/adr/0001-request-body-scope-instructions-convention.md`.
/// Centralised here so a future rename is a one-line change. This is
/// NOT part of the v0.5.1 normative spec; it is the reference
/// implementation's default request-scope shape pending ~10 real
/// cross-agent exchanges.
pub const REQUEST_SCOPE_INSTRUCTIONS_KEY: &str = "instructions";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBody {
    pub scope: serde_json::Value,
    pub bounds: Bounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natural_language_summary: Option<String>,
}

impl RequestBody {
    pub(crate) fn validate(&self) -> Result<(), EnvelopeDecodeError> {
        self.bounds.validate()
    }
}

impl BodySchema for RequestBody {
    const CLASS: MessageClass = MessageClass::Request;
    // D-C3: v0.7 locks request to Standalone. v0.8+ negotiation/causality may re-scope.
    const SCOPE: EnvelopeScope = EnvelopeScope::Standalone;

    fn post_decode_validate(
        &self,
        _ts: Option<&crate::body::deliver::TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> {
        self.validate()
    }
}
