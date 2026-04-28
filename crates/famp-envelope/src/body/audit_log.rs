//! `audit_log` body schema (v0.5.2 §8a.6).
//!
//! Fire-and-forget envelope class: receiver MUST store, MUST NOT emit `ack`
//! (Delta 31 normative). Non-FSM-firing; it joins the `Ack` precedent in
//! `famp-fsm::engine`. See spec §8a.6, §7.3a, §19 Delta 29-33.

use crate::body::BodySchema;
use crate::scope::EnvelopeScope;
use crate::EnvelopeDecodeError;
use famp_core::MessageClass;
use serde::{Deserialize, Serialize};

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditLogBody {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl BodySchema for AuditLogBody {
    const CLASS: MessageClass = MessageClass::AuditLog;
    const SCOPE: EnvelopeScope = EnvelopeScope::Standalone;

    fn post_decode_validate(
        &self,
        _terminal_status: Option<&crate::body::deliver::TerminalStatus>,
    ) -> Result<(), EnvelopeDecodeError> {
        if self.event.is_empty() {
            return Err(EnvelopeDecodeError::BodyValidation(
                "audit_log.event must be non-empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn roundtrip_minimum_event_only() {
        let body = AuditLogBody {
            event: "user_login".into(),
            subject: None,
            details: None,
        };
        let bytes = famp_canonical::canonicalize(&body).unwrap();
        let decoded: AuditLogBody = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn roundtrip_with_subject_and_details() {
        let body = AuditLogBody {
            event: "policy_update".into(),
            subject: Some("agent:alice".into()),
            details: Some(serde_json::json!({"old": 1, "new": 2})),
        };
        let bytes = famp_canonical::canonicalize(&body).unwrap();
        let decoded: AuditLogBody = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn rejects_unknown_field() {
        let json = br#"{"event":"x","extra_field":"nope"}"#;
        let res: Result<AuditLogBody, _> = famp_canonical::from_slice_strict(json);
        assert!(res.is_err(), "deny_unknown_fields must reject extra_field");
    }

    #[test]
    fn rejects_empty_event_via_post_decode() {
        let body = AuditLogBody {
            event: String::new(),
            subject: None,
            details: None,
        };
        let res = body.post_decode_validate(None);
        assert!(matches!(res, Err(EnvelopeDecodeError::BodyValidation(_))));
    }
}
