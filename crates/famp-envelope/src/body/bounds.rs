//! Shared `Bounds` struct from FAMP v0.5.1 §9.3.
//!
//! The §9.3 rule requires at least 2 keys from the 8-field set to be set; otherwise
//! the bounds are considered unenforceable. `Bounds::validate()` enforces that rule
//! at decode time, surfacing a typed `EnvelopeDecodeError::InsufficientBounds`.
//!
//! `Budget.amount` is STRING per PITFALLS P2 / §8a: JSON numbers cannot represent
//! all monetary amounts without loss above 2^53; a numeric amount is rejected.
//!
//! PITFALL P4: `confidence_floor` is `f64`, but must be finite and in `[0.0, 1.0]`.
//! NaN / Inf / out-of-range values are rejected by `validate()`.

use crate::EnvelopeDecodeError;
use famp_core::AuthorityScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<Budget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_scope: Option<AuthorityScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_artifact_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_floor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursion_depth: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// STRING not NUMBER — avoids 2^53 precision loss (PITFALLS P2, §8a).
    pub amount: String,
    pub unit: String,
}

impl Bounds {
    /// §9.3 validation: ≥2 keys from the 8-field set must be `Some`.
    /// Also rejects non-finite / out-of-range `confidence_floor` (PITFALL P4).
    #[allow(dead_code)] // wired progressively across Plans 02/03
    pub(crate) fn validate(&self) -> Result<(), EnvelopeDecodeError> {
        let count = [
            self.deadline.is_some(),
            self.budget.is_some(),
            self.hop_limit.is_some(),
            self.policy_domain.is_some(),
            self.authority_scope.is_some(),
            self.max_artifact_size.is_some(),
            self.confidence_floor.is_some(),
            self.recursion_depth.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count();

        if count < 2 {
            return Err(EnvelopeDecodeError::InsufficientBounds { count });
        }

        // PITFALL P4: reject NaN/Inf and out-of-range on confidence_floor.
        if let Some(cf) = self.confidence_floor {
            if !cf.is_finite() || !(0.0..=1.0).contains(&cf) {
                return Err(EnvelopeDecodeError::BodyValidation(format!(
                    "confidence_floor must be finite in [0.0, 1.0], got {cf}"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn empty() -> Bounds {
        Bounds {
            deadline: None,
            budget: None,
            hop_limit: None,
            policy_domain: None,
            authority_scope: None,
            max_artifact_size: None,
            confidence_floor: None,
            recursion_depth: None,
        }
    }

    #[test]
    fn one_key_fails_insufficient_bounds() {
        let b = Bounds {
            deadline: Some("2026-05-01T00:00:00Z".to_string()),
            ..empty()
        };
        let err = b.validate().unwrap_err();
        assert!(matches!(
            err,
            EnvelopeDecodeError::InsufficientBounds { count: 1 }
        ));
    }

    #[test]
    fn two_keys_ok() {
        let b = Bounds {
            deadline: Some("2026-05-01T00:00:00Z".to_string()),
            hop_limit: Some(3),
            ..empty()
        };
        b.validate().unwrap();
    }

    #[test]
    fn confidence_floor_roundtrip_and_nan_rejected() {
        let ok = Bounds {
            confidence_floor: Some(0.5),
            hop_limit: Some(1),
            ..empty()
        };
        ok.validate().unwrap();
        let json = serde_json::to_string(&ok).unwrap();
        let back: Bounds = serde_json::from_str(&json).unwrap();
        assert_eq!(ok, back);

        let nan = Bounds {
            confidence_floor: Some(f64::NAN),
            hop_limit: Some(1),
            ..empty()
        };
        assert!(matches!(
            nan.validate().unwrap_err(),
            EnvelopeDecodeError::BodyValidation(_)
        ));

        let inf = Bounds {
            confidence_floor: Some(f64::INFINITY),
            hop_limit: Some(1),
            ..empty()
        };
        assert!(matches!(
            inf.validate().unwrap_err(),
            EnvelopeDecodeError::BodyValidation(_)
        ));
    }

    #[test]
    fn budget_amount_string_roundtrip_numeric_rejected() {
        let b = Bounds {
            budget: Some(Budget {
                amount: "100".to_string(),
                unit: "usd".to_string(),
            }),
            hop_limit: Some(1),
            ..empty()
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"amount\":\"100\""));
        let back: Bounds = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);

        // Numeric amount must fail decode (serde type error: expected string).
        let bad = r#"{"budget":{"amount":100,"unit":"usd"},"hop_limit":1}"#;
        let result: Result<Bounds, _> = serde_json::from_str(bad);
        assert!(result.is_err());
    }
}
