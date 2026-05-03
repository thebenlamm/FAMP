//! Hook-runner shim asset (HOOK-04b runner).
//!
//! Embeds `crates/famp/assets/hook-runner.sh` at compile time and
//! provides `install_shim` / `remove_shim` helpers used by
//! `install-claude-code` / `uninstall-claude-code` (plans 03-03/03-04).
//!
//! D-08 invariant: shim is bash, NOT a Rust subcommand. D-08 also requires
//! shellcheck-clean - the asset itself is gated by `just check-shellcheck`.

use std::path::Path;

use crate::cli::error::CliError;

/// The bash shim source, embedded at compile time.
pub const HOOK_RUNNER_SH: &str = include_str!("../../../assets/hook-runner.sh");

/// Write the shim to `path` at mode 0755. Idempotent (overwrites existing).
/// Creates parent directories if absent.
pub fn install_shim(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, HOOK_RUNNER_SH).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(
            |source| CliError::Io {
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

/// Remove the shim. Tolerates NotFound. Used by `uninstall-claude-code`.
pub fn remove_shim(path: &Path) -> Result<(), CliError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(CliError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn shim_starts_with_bash_shebang() {
        assert!(HOOK_RUNNER_SH.starts_with("#!/usr/bin/env bash\n"));
    }

    #[test]
    fn shim_uses_set_uo_pipefail_not_e() {
        // D-08: must NOT use `set -e` because every error path needs to exit 0.
        assert!(HOOK_RUNNER_SH.contains("set -uo pipefail"));
        assert!(!HOOK_RUNNER_SH.contains("set -euo pipefail"));
    }

    #[test]
    fn shim_calls_famp_send() {
        assert!(HOOK_RUNNER_SH.contains("famp send"));
    }

    #[test]
    fn install_shim_creates_file_at_mode_0755() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".famp/hook-runner.sh");
        install_shim(&path).unwrap();
        assert!(path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o755, "mode = {mode:o}");
        }
    }

    #[test]
    fn install_shim_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hook-runner.sh");
        install_shim(&path).unwrap();
        install_shim(&path).unwrap();
    }

    #[test]
    fn remove_shim_after_install_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hook-runner.sh");
        install_shim(&path).unwrap();
        remove_shim(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_shim_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.sh");
        remove_shim(&path).unwrap();
    }
}
