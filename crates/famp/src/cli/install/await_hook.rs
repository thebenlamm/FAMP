//! Listen-mode await shim asset.
//!
//! Embeds `crates/famp/assets/famp-await.sh` at compile time and provides
//! `install_shim` / `remove_shim` helpers used by
//! `install-claude-code` / `uninstall-claude-code`.
//!
//! Exit-0 fail-open design: the hook must never trap Claude.
//! shellcheck-clean is enforced by `just check-shellcheck`.

use std::path::Path;

use crate::cli::error::CliError;

/// The bash await-shim source, embedded at compile time.
pub const FAMP_AWAIT_SH: &str = include_str!("../../../assets/famp-await.sh");

/// Write the await shim to `path` at mode 0755. Idempotent (overwrites existing).
/// Creates parent directories if absent.
pub fn install_shim(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, FAMP_AWAIT_SH).map_err(|source| CliError::Io {
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

/// Remove the await shim. Tolerates `NotFound`. Used by `uninstall-claude-code`.
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
        assert!(FAMP_AWAIT_SH.starts_with("#!/usr/bin/env bash\n"));
    }

    #[test]
    fn shim_uses_set_uo_pipefail_not_e() {
        // Fail-open invariant: must NOT use set -e; every error path must exit 0.
        assert!(FAMP_AWAIT_SH.contains("set -uo pipefail"));
        assert!(!FAMP_AWAIT_SH.contains("set -euo pipefail"));
    }

    #[test]
    fn shim_calls_famp_await() {
        assert!(FAMP_AWAIT_SH.contains("famp await"));
    }

    /// Fix A (260721): the shipped shim MUST carry the broker fallback so a
    /// compacted transcript (which drops the famp_register marker out of the
    /// 2 MB scan window) can still resolve its identity. This is the whole
    /// point of shipping the fix in the asset rather than a hand-patched
    /// installed file — a reinstall must never silently revert it.
    #[test]
    fn shim_has_broker_fallback_via_inspect_json() {
        assert!(
            FAMP_AWAIT_SH.contains("inspect identities --json"),
            "shim lost the broker-fallback identity resolution"
        );
        // Must key the fallback on BOTH listen mode and this session's cwd.
        assert!(FAMP_AWAIT_SH.contains("listen_mode"));
        assert!(FAMP_AWAIT_SH.contains("SESSION_CWD"));
    }

    /// Fix E (260721): the shim MUST surface the disarm (a visible block
    /// warning) when it detects an ambiguous but clearly-listening state,
    /// rather than silently no-opping.
    #[test]
    fn shim_surfaces_disarm_on_ambiguity() {
        assert!(
            FAMP_AWAIT_SH.contains("!AMBIGUOUS"),
            "shim lost the ambiguity sentinel"
        );
        assert!(
            FAMP_AWAIT_SH.contains("DISARMED"),
            "shim lost the surfaced disarm warning"
        );
    }

    #[test]
    fn install_shim_creates_file_at_mode_0755() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude/hooks/famp-await.sh");
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
        let path = dir.path().join("famp-await.sh");
        install_shim(&path).unwrap();
        install_shim(&path).unwrap();
    }

    #[test]
    fn remove_shim_after_install_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("famp-await.sh");
        install_shim(&path).unwrap();
        remove_shim(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_shim_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent-famp-await.sh");
        remove_shim(&path).unwrap();
    }
}
