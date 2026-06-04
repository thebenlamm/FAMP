//! Shared Linux helpers for the `famp daemon` subcommand.
//!
//! This module is cfg-gated to Linux at the `pub mod linux` declaration in
//! `daemon/mod.rs` — no need for `#![cfg(target_os = "linux")]` here.
//!
//! `parse_linger` is a shared helper used by both Plan 04 (install) and
//! Plan 05 (status) — placing it here (wave 2) means it exists before both
//! callers and neither defines its own copy (DAEMON-06).

/// Parse the output of `loginctl show-user <user> --property=Linger`.
///
/// Returns `true` iff the output contains a `Linger=yes` line.
/// `loginctl show-user` format is `Linger=yes` or `Linger=no` (one property per line).
pub(crate) fn parse_linger(loginctl_output: &str) -> bool {
    loginctl_output
        .lines()
        .any(|line| line.trim() == "Linger=yes")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn linger_parse_yes() {
        assert!(parse_linger("Linger=yes\n"));
        assert!(parse_linger("Linger=yes"));
        assert!(parse_linger("SomeOtherProp=foo\nLinger=yes\n"));
    }

    #[test]
    fn linger_parse_no() {
        assert!(!parse_linger("Linger=no\n"));
        assert!(!parse_linger("Linger=no"));
        assert!(!parse_linger(""));
        assert!(!parse_linger("SomeOtherProp=foo\n"));
    }
}
