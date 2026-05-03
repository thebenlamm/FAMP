//! Slash-command asset writers (CC-02..08).
//!
//! Embeds the 7 markdown templates at compile time via `include_str!`.
//! `write_all` drops them into `commands_dir` (typically
//! `~/.claude/commands/`); `remove_all` reverses (uninstall - plan 03-04).

use std::path::Path;

use crate::cli::error::CliError;

/// Filename -> asset-content pairs for the 7 slash commands.
/// Order is stable for snapshot tests.
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "famp-register.md",
        include_str!("../../../assets/slash_commands/famp-register.md"),
    ),
    (
        "famp-send.md",
        include_str!("../../../assets/slash_commands/famp-send.md"),
    ),
    (
        "famp-channel.md",
        include_str!("../../../assets/slash_commands/famp-channel.md"),
    ),
    (
        "famp-join.md",
        include_str!("../../../assets/slash_commands/famp-join.md"),
    ),
    (
        "famp-leave.md",
        include_str!("../../../assets/slash_commands/famp-leave.md"),
    ),
    (
        "famp-who.md",
        include_str!("../../../assets/slash_commands/famp-who.md"),
    ),
    (
        "famp-inbox.md",
        include_str!("../../../assets/slash_commands/famp-inbox.md"),
    ),
];

/// Write all 7 markdown templates into `commands_dir`. Idempotent
/// (overwrites existing). Mode 0644 enforced post-write.
pub fn write_all(commands_dir: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(commands_dir).map_err(|source| CliError::Io {
        path: commands_dir.to_path_buf(),
        source,
    })?;
    for (filename, body) in TEMPLATES {
        let path = commands_dir.join(filename);
        // Use std::fs::write (overwrite-OK) instead of perms::write_public -
        // the latter uses O_CREAT|O_EXCL which fails on idempotent re-install.
        std::fs::write(&path, body).map_err(|source| CliError::Io {
            path: path.clone(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
        }
    }
    Ok(())
}

/// Remove all 7 markdown templates from `commands_dir`. Tolerates `NotFound`
/// per-file. Used by `uninstall-claude-code` (plan 03-04).
pub fn remove_all(commands_dir: &Path) -> Result<(), CliError> {
    for (filename, _) in TEMPLATES {
        let path = commands_dir.join(filename);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CliError::Io { path, source });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn templates_count_is_seven() {
        assert_eq!(TEMPLATES.len(), 7);
    }

    #[test]
    fn every_template_has_yaml_frontmatter() {
        for (name, body) in TEMPLATES {
            assert!(body.starts_with("---\n"), "{name} missing frontmatter open");
            let after_open = &body[4..];
            assert!(
                after_open.contains("\n---\n"),
                "{name} missing frontmatter close"
            );
        }
    }

    #[test]
    fn every_template_declares_allowed_tools() {
        for (name, body) in TEMPLATES {
            assert!(
                body.contains("allowed-tools:"),
                "{name} missing allowed-tools declaration"
            );
        }
    }

    #[test]
    fn famp_send_template_references_correct_tool() {
        let (_, body) = TEMPLATES
            .iter()
            .find(|(n, _)| *n == "famp-send.md")
            .unwrap();
        assert!(body.contains("mcp__famp__famp_send"));
    }

    #[test]
    fn write_all_creates_seven_files_at_mode_0644() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("commands");
        write_all(&cmd_dir).unwrap();
        let entries: Vec<_> = std::fs::read_dir(&cmd_dir).unwrap().collect();
        assert_eq!(entries.len(), 7);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for (filename, _) in TEMPLATES {
                let p = cmd_dir.join(filename);
                let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o644, "{filename} mode = {mode:o}");
            }
        }
    }

    #[test]
    fn write_all_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("commands");
        write_all(&cmd_dir).unwrap();
        write_all(&cmd_dir).unwrap();
        let entries: Vec<_> = std::fs::read_dir(&cmd_dir).unwrap().collect();
        assert_eq!(entries.len(), 7);
    }

    #[test]
    fn remove_all_after_write_all_leaves_directory_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("commands");
        write_all(&cmd_dir).unwrap();
        remove_all(&cmd_dir).unwrap();
        let entries: Vec<_> = std::fs::read_dir(&cmd_dir).unwrap().collect();
        assert_eq!(
            entries.len(),
            0,
            "directory should be empty after remove_all"
        );
    }

    #[test]
    fn remove_all_tolerates_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("commands");
        std::fs::create_dir_all(&cmd_dir).unwrap();
        remove_all(&cmd_dir).unwrap();
    }
}
