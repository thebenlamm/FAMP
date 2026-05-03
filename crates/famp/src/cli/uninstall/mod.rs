//! `famp uninstall-*` subcommands. Symmetric inverse of `crate::cli::install`.
//!
//! D-04 invariant: every install mutation has a paired remove in the
//! uninstall path. Helpers (`json_merge::remove_user_json`,
//! `slash_commands::remove_all`, `hook_runner::remove_shim`) are owned by
//! the install module; uninstall handlers are pure orchestrators.

pub mod claude_code;
pub mod codex;
