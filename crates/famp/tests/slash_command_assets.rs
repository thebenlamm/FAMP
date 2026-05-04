#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 05 CC-07 regression gate.
//!
//! `crates/famp/assets/slash_commands/famp-who.md` must call only registered
//! MCP tools. The v0.9 MCP surface is exactly 8 tools (`famp_send`,
//! `famp_await`, `famp_inbox`, `famp_peers`, `famp_register`, `famp_whoami`,
//! `famp_join`, `famp_leave`); `famp_sessions` is NOT among them.
//!
//! These tests permanently lock the asset shape so a future edit cannot
//! re-introduce the broken `mcp__famp__famp_sessions` reference. See
//! `.planning/v0.9-MILESTONE-AUDIT.md` for the originating CC-07 evidence.

const FAMP_WHO_MD: &str = include_str!("../assets/slash_commands/famp-who.md");

#[test]
fn test_famp_who_does_not_reference_unregistered_tool() {
    assert!(
        !FAMP_WHO_MD.contains("famp_sessions"),
        "famp-who.md must not reference famp_sessions \
         (not a registered MCP tool — see CC-07 / v0.9-MILESTONE-AUDIT.md)"
    );
}

#[test]
fn test_famp_who_allowed_tools_lists_only_famp_peers() {
    let line = FAMP_WHO_MD
        .lines()
        .find(|l| l.starts_with("allowed-tools:"))
        .expect("allowed-tools frontmatter line missing");
    let tools: std::collections::BTreeSet<&str> = line
        .trim_start_matches("allowed-tools:")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let expected: std::collections::BTreeSet<&str> =
        ["mcp__famp__famp_peers"].into_iter().collect();
    assert_eq!(tools, expected, "CC-07: allowed-tools surface drift");
}

#[test]
fn test_famp_who_argument_hint_present() {
    assert!(
        FAMP_WHO_MD.contains("argument-hint: [#channel?]"),
        "argument-hint contract changed — CC-07 expects [#channel?]"
    );
}
