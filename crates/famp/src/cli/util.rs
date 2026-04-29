//! Shared CLI helpers used across multiple subcommands.
//!
//! Currently exposes [`normalize_channel`] — the channel-name auto-prefix
//! and validation helper relocated here from `cli/send/mod.rs` (plan 02-04
//! authored it inline; plan 02-07 promotes it to a shared module so
//! `famp send`, `famp join`, and `famp leave` all parse channel arguments
//! identically).
//!
//! Behaviour (RESEARCH §2 Item 11):
//!
//! - Accepts `planning` and `#planning` equivalently — the leading `#`
//!   is auto-prefixed when omitted.
//! - Rejects `##planning` (double-hash is never legal).
//! - Validates against `famp_bus`'s channel regex
//!   (`^#[a-z0-9][a-z0-9_-]{0,31}$`), which is byte-equivalent to the
//!   constant inlined here so the CLI surface and the broker reject the
//!   same set of names.

use std::sync::LazyLock;

use crate::cli::error::CliError;

/// Channel name validation regex (mirrors `famp_bus::proto::CHANNEL_PATTERN`).
/// Locally inlined because `famp_bus` does not export the regex publicly.
const CHANNEL_PATTERN: &str = r"^#[a-z0-9][a-z0-9_-]{0,31}$";

/// WR-03: compile once at first use rather than on every call. The bus
/// side already does this in `famp_bus::proto`; we mirror the pattern
/// so MCP tool loops (`famp_send`, `famp_join`, `famp_leave`) do not
/// pay the regex-compile cost per invocation.
static CHANNEL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(CHANNEL_PATTERN).expect("static channel regex compiles")
});

/// Normalize a channel name: accept both `planning` and `#planning`;
/// reject `##planning`; validate against the bus channel regex.
///
/// Returns the normalized form (always begins with exactly one `#`)
/// on success. On any rejection returns
/// [`CliError::SendArgsInvalid`] — the variant is reused across
/// subcommands because the underlying validation failure (bad channel
/// argument) is the same regardless of which subcommand surfaced it.
pub fn normalize_channel(input: &str) -> Result<String, CliError> {
    let normalized = if input.starts_with('#') {
        input.to_string()
    } else {
        format!("#{input}")
    };
    if normalized.starts_with("##") {
        return Err(CliError::SendArgsInvalid {
            reason: format!("channel name '{input}' cannot start with ##"),
        });
    }
    if !CHANNEL_RE.is_match(&normalized) {
        return Err(CliError::SendArgsInvalid {
            reason: format!("invalid channel name '{normalized}': must match {CHANNEL_PATTERN}"),
        });
    }
    Ok(normalized)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn normalize_channel_adds_hash_prefix() {
        assert_eq!(normalize_channel("planning").unwrap(), "#planning");
    }

    #[test]
    fn normalize_channel_accepts_existing_hash() {
        assert_eq!(normalize_channel("#planning").unwrap(), "#planning");
    }

    #[test]
    fn normalize_channel_rejects_double_hash() {
        let err = normalize_channel("##planning").unwrap_err();
        match err {
            CliError::SendArgsInvalid { reason } => assert!(
                reason.contains("cannot start with ##"),
                "unexpected reason: {reason}"
            ),
            other => panic!("expected SendArgsInvalid, got {other:?}"),
        }
    }

    #[test]
    fn normalize_channel_rejects_uppercase() {
        let err = normalize_channel("BadCaps").unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }

    #[test]
    fn normalize_channel_rejects_overlong() {
        // 33 chars after the `#` exceeds the {0,31} bound.
        let long = format!("#a{}", "b".repeat(32));
        let err = normalize_channel(&long).unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }
}
