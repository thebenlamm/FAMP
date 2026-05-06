//! `famp install-*` subcommands.
//!
//! Phase 3 ships four install/uninstall pairs:
//!  - `install-claude-code` / `uninstall-claude-code` (D-04 symmetric pair)
//!  - `install-codex`        / `uninstall-codex`       (D-12 MCP-only pair)
//!
//! This module hosts the install-side handlers and the shared helper
//! modules. Uninstall handlers live in `crate::cli::uninstall` (see plan
//! 03-04 / 03-05).

pub mod await_hook;
pub mod claude_code;
pub mod codex;
pub mod hook_runner;
pub mod json_merge;
pub mod slash_commands;
pub mod toml_merge;
