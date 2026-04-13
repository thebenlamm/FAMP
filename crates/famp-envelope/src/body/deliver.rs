//! `DeliverBody` — FAMP v0.5.1 §8a.3 deliver body with cross-field validation.
//!
//! The `interim` flag gates whether `terminal_status` can be present on the envelope
//! header, and `error_detail` / `provenance` are required in specific terminal states.

use crate::body::BodySchema;
use crate::{EnvelopeDecodeError, EnvelopeScope, MessageClass};
use famp_core::ArtifactId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliverBody {
    pub interim: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<Artifact>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metrics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<ErrorDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natural_language_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: ArtifactId,
    pub media_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::derive_partial_eq_without_eq)] // diagnostic is Option<serde_json::Value>
pub struct ErrorDetail {
    pub category: ErrorCategory,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ErrorCategory {
    Malformed,
    Unauthorized,
    UnsupportedVersion,
    Timeout,
    Internal,
    Other,
}

pub use famp_core::TerminalStatus;

impl DeliverBody {
    /// Cross-field validation tying `interim` + `terminal_status` + required fields.
    #[allow(clippy::missing_const_for_fn)]
    pub fn validate_against_terminal_status(
        &self,
        terminal_status: Option<&TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> {
        match (self.interim, terminal_status) {
            (true, Some(_)) => Err(EnvelopeDecodeError::InterimWithTerminalStatus),
            (false, None) => Err(EnvelopeDecodeError::TerminalWithoutStatus),
            (false, Some(TerminalStatus::Failed)) if self.error_detail.is_none() => {
                Err(EnvelopeDecodeError::MissingErrorDetail)
            }
            (false, Some(_)) if self.provenance.is_none() => {
                Err(EnvelopeDecodeError::MissingProvenance)
            }
            _ => Ok(()),
        }
    }
}

impl BodySchema for DeliverBody {
    const CLASS: MessageClass = MessageClass::Deliver;
    const SCOPE: EnvelopeScope = EnvelopeScope::Task;

    fn post_decode_validate(
        &self,
        envelope_terminal_status: Option<&TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> {
        self.validate_against_terminal_status(envelope_terminal_status)
    }
}
