//! AUDIT-05 RED-first gate (pure Rust, no fixture dependency).
//!
//! Fails to compile at HEAD~1: no `MessageClass::AuditLog` variant.
//! Passes at HEAD once the v0.5.2 `audit_log` bump lands atomically.

#![allow(unused_crate_dependencies)]

use famp_core::MessageClass;
use famp_envelope::FAMP_SPEC_VERSION;

#[test]
fn audit_log_variant_exists_at_v0_5_2() {
    assert!(matches!(MessageClass::AuditLog, MessageClass::AuditLog));
    assert_eq!(FAMP_SPEC_VERSION, "0.5.2");
}
