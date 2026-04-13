//! # v0.7 narrowing — ENV-09
//!
//! `capability_snapshot` is intentionally ABSENT from `CommitBody`. The full v0.5.1
//! spec §8a.2 requires it, but v0.7 Personal Profile does not have Agent Cards yet;
//! capability binding lands with v0.8 §11.2a Identity & Cards.
//!
//! Adding a `capability_snapshot: Option<...>` field to this struct would silently
//! break the narrowing because `deny_unknown_fields` would then accept the key.
//! Do NOT add it until v0.8. See RESEARCH.md Pitfall 1 for the drive-by PR risk.

use crate::body::{bounds::Bounds, BodySchema};
use crate::{EnvelopeDecodeError, EnvelopeScope, MessageClass};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitBody {
    pub scope: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_subset: Option<bool>,
    pub bounds: Bounds,
    pub accepted_policies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation_permissions: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_obligations: Option<serde_json::Value>,
    pub terminal_condition: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natural_language_summary: Option<String>,
    // INTENTIONALLY ABSENT: capability_snapshot (ENV-09 narrowing, v0.8 §11.2a).
}

impl CommitBody {
    pub(crate) fn validate(&self) -> Result<(), EnvelopeDecodeError> {
        self.bounds.validate()
    }
}

impl BodySchema for CommitBody {
    const CLASS: MessageClass = MessageClass::Commit;
    const SCOPE: EnvelopeScope = EnvelopeScope::Task;

    fn post_decode_validate(
        &self,
        _ts: Option<&crate::body::deliver::TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> {
        self.validate()
    }
}
