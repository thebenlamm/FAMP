# Phase 04 Deferred Items

## Plan 04-01

- `just ci` reaches clippy and fails on pre-existing `clippy::option_if_let_else`
  findings in `crates/famp/src/cli/install/claude_code.rs:200` and
  `crates/famp/src/cli/uninstall/claude_code.rs:212`. These files are outside
  Plan 04-01 scope and were not modified by this plan.

## Plan 04-02

- `just ci` is still blocked by the same pre-existing
  `clippy::option_if_let_else` findings in
  `crates/famp/src/cli/install/claude_code.rs:200` and
  `crates/famp/src/cli/uninstall/claude_code.rs:212`. Plan 04-02 modified
  only the deferred federation test archive, so these clippy fixes remain
  out of scope.
- Broad compile-coupling scan still finds fully-qualified
  `famp::cli::init::*` calls in `crates/famp/tests/mcp_malformed_input.rs`
  and helper files under `crates/famp/tests/common/`. They do not match
  Plan 04-02's active acceptance grep after the expected moves, but Plan
  04-08 should account for them before deleting `cli/init`.
