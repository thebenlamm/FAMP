//! `famp install-*` subcommands.
//!
//! Install/uninstall pairs:
//!  - `install-claude-code` / `uninstall-claude-code` (MCP + Stop hook pair)
//!  - `install-codex`        / `uninstall-codex`       (MCP + Stop hook pair)
//!  - `install-grok`         / `uninstall-grok`        (MCP + Stop hook + skill)
//!
//! This module hosts the install-side handlers and the shared helper
//! modules. Uninstall handlers live in `crate::cli::uninstall` (see plan
//! 03-04 / 03-05).

pub mod await_hook;
pub mod claude_code;
pub mod codex;
pub mod grok;
pub mod hook_runner;
pub mod json_merge;
pub mod slash_commands;
pub mod stop_entry;
pub mod toml_merge;
