# Phase 04 Deferred Items

## Plan 04-01

- `just ci` reaches clippy and fails on pre-existing `clippy::option_if_let_else`
  findings in `crates/famp/src/cli/install/claude_code.rs:200` and
  `crates/famp/src/cli/uninstall/claude_code.rs:212`. These files are outside
  Plan 04-01 scope and were not modified by this plan.
