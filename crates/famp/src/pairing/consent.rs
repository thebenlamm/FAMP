//! The single authored source of the QUAR-15 consent warning (T-18-17).
//!
//! [`CONSENT_WARNING`] is rendered into TWO surfaces — the invite artifact
//! (`crates/famp/src/cli/pair/invite.rs`, strictly before the code line,
//! PAIR-08) and `docs/QUARANTINE.md` (under [`CONSENT_WARNING_HEADING`]) —
//! and consumed as-is by Phase 19's auto-wake gate. Any change to the
//! wording MUST be made HERE and nowhere else:
//! `consent_warning_matches_quarantine_doc`
//! (`crates/famp/tests/pair_cli.rs`) fails the build if the two surfaces
//! drift, so a re-typed copy in `docs/QUARANTINE.md` cannot silently go
//! stale.

/// The QUAR-15 consent warning.
///
/// The fact a person agrees to the moment they redeem a pairing code —
/// that their agent's messages will be read by, and their machine's
/// commands run by, the peer they are about to trust. Shown in the
/// invite artifact strictly before the five-word code (PAIR-08, T-18-17),
/// so it is read while the decision is still open.
pub const CONSENT_WARNING: &str = "Pairing with someone means their agent's messages will be read by your agent, which can run commands on your machine. Pair only with someone you would let type into your terminal.";

/// The `docs/QUARANTINE.md` section heading the doc-sync test anchors on.
pub const CONSENT_WARNING_HEADING: &str = "## Consent warning (QUAR-15)";

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{CONSENT_WARNING, CONSENT_WARNING_HEADING};

    #[test]
    fn consent_warning_is_non_empty_and_heading_matches_quar_15() {
        assert!(CONSENT_WARNING.contains("run commands on your machine"));
        assert_eq!(CONSENT_WARNING_HEADING, "## Consent warning (QUAR-15)");
    }
}
