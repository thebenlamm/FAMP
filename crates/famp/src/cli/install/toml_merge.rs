//! Atomic structural-merge TOML helper for `~/.codex/config.toml` (D-02
//! invariant carried to TOML target; D-12 Codex MCP-only).
//!
//! Mirrors `json_merge.rs`: read existing `toml::Table`, mutate only the
//! leaf table at `[parent_key.leaf_key]`, back up pre-state, then atomically
//! persist with a same-directory tempfile.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::NamedTempFile;
use toml::Value;

use crate::cli::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum TomlMergeOutcome {
    Inserted,
    Updated,
    AlreadyMatches,
    Removed,
    NotPresent,
}

/// Idempotent upsert of `[parent_key.leaf_key]` table within a TOML file.
/// Preserves every other section.
pub fn upsert_codex_table(
    path: &Path,
    parent_key: &str,
    leaf_key: &str,
    value: toml::Table,
) -> Result<TomlMergeOutcome, CliError> {
    let display_path = path.to_path_buf();
    let mut root = read_toml_table(path, &display_path, true)?;

    let parent_entry = root
        .entry(parent_key.to_string())
        .or_insert_with(|| Value::Table(toml::Table::new()));
    let parent = parent_entry
        .as_table_mut()
        .ok_or_else(|| CliError::TomlTableExpected {
            path: PathBuf::from(format!("{}#/{parent_key}", display_path.display())),
        })?;

    let outcome = match parent.get(leaf_key) {
        Some(Value::Table(existing)) if *existing == value => {
            return Ok(TomlMergeOutcome::AlreadyMatches);
        }
        Some(_) => TomlMergeOutcome::Updated,
        None => TomlMergeOutcome::Inserted,
    };
    parent.insert(leaf_key.to_string(), Value::Table(value));

    backup_if_exists(path)?;
    persist_toml(path, &display_path, &root)?;
    Ok(outcome)
}

/// Symmetric remove: drop `[parent_key.leaf_key]` only if present.
pub fn remove_codex_table(
    path: &Path,
    parent_key: &str,
    leaf_key: &str,
) -> Result<TomlMergeOutcome, CliError> {
    let display_path = path.to_path_buf();
    let mut root = match read_toml_table(path, &display_path, false) {
        Ok(root) => root,
        Err(CliError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TomlMergeOutcome::NotPresent);
        }
        Err(e) => return Err(e),
    };

    let Some(parent_val) = root.get_mut(parent_key) else {
        return Ok(TomlMergeOutcome::NotPresent);
    };
    let parent = parent_val
        .as_table_mut()
        .ok_or_else(|| CliError::TomlTableExpected {
            path: PathBuf::from(format!("{}#/{parent_key}", display_path.display())),
        })?;

    if parent.remove(leaf_key).is_none() {
        return Ok(TomlMergeOutcome::NotPresent);
    }

    if parent.is_empty() {
        root.remove(parent_key);
    }

    backup_if_exists(path)?;
    persist_toml(path, &display_path, &root)?;
    Ok(TomlMergeOutcome::Removed)
}

fn read_toml_table(
    path: &Path,
    display_path: &Path,
    missing_is_empty: bool,
) -> Result<toml::Table, CliError> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(toml::Table::new()),
        Ok(s) => toml::from_str::<toml::Table>(&s).map_err(|source| CliError::TomlParse {
            path: display_path.to_path_buf(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && missing_is_empty => {
            Ok(toml::Table::new())
        }
        Err(source) => Err(CliError::Io {
            path: display_path.to_path_buf(),
            source,
        }),
    }
}

fn backup_if_exists(path: &Path) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("toml");
    let bak = path.with_extension(format!("{ext}.bak.{ts}"));
    std::fs::copy(path, &bak).map_err(|source| CliError::Io { path: bak, source })?;
    Ok(())
}

fn persist_toml(path: &Path, display_path: &Path, root: &toml::Table) -> Result<(), CliError> {
    let parent_dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent_dir).map_err(|source| CliError::Io {
        path: parent_dir.to_path_buf(),
        source,
    })?;
    let mut tmp = NamedTempFile::new_in(parent_dir).map_err(|source| CliError::Io {
        path: parent_dir.to_path_buf(),
        source,
    })?;
    let serialized = toml::to_string(root).map_err(CliError::TomlSerialize)?;
    tmp.write_all(serialized.as_bytes())
        .map_err(|source| CliError::Io {
            path: tmp.path().to_path_buf(),
            source,
        })?;
    tmp.as_file_mut()
        .sync_all()
        .map_err(|source| CliError::Io {
            path: tmp.path().to_path_buf(),
            source,
        })?;
    tmp.persist(path).map_err(|e| CliError::Io {
        path: display_path.to_path_buf(),
        source: e.error,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn famp_value() -> toml::Table {
        let mut t = toml::Table::new();
        t.insert(
            "command".into(),
            Value::String("/Users/test/.cargo/bin/famp".into()),
        );
        t.insert(
            "args".into(),
            Value::Array(vec![Value::String("mcp".into())]),
        );
        t.insert("startup_timeout_sec".into(), Value::Integer(10));
        t
    }

    #[test]
    fn upsert_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let outcome = upsert_codex_table(&path, "mcp_servers", "famp", famp_value()).unwrap();
        assert_eq!(outcome, TomlMergeOutcome::Inserted);
        assert!(path.exists());
        let post: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            post["mcp_servers"]["famp"]["command"].as_str().unwrap(),
            "/Users/test/.cargo/bin/famp"
        );
    }

    #[test]
    fn upsert_preserves_unrelated_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[other_section]\nkey = \"value\"\n[mcp_servers.github]\ncommand = \"/x\"\n",
        )
        .unwrap();
        upsert_codex_table(&path, "mcp_servers", "famp", famp_value()).unwrap();
        let post: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(post["other_section"]["key"].as_str().unwrap(), "value");
        assert_eq!(
            post["mcp_servers"]["github"]["command"].as_str().unwrap(),
            "/x"
        );
        assert_eq!(
            post["mcp_servers"]["famp"]["command"].as_str().unwrap(),
            "/Users/test/.cargo/bin/famp"
        );
    }

    #[test]
    fn second_upsert_with_same_value_is_already_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        upsert_codex_table(&path, "mcp_servers", "famp", famp_value()).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let outcome = upsert_codex_table(&path, "mcp_servers", "famp", famp_value()).unwrap();
        assert_eq!(outcome, TomlMergeOutcome::AlreadyMatches);
        assert_eq!(first, std::fs::read_to_string(&path).unwrap());
    }

    #[test]
    fn remove_drops_only_target_subtable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[mcp_servers.famp]\ncommand = \"/y\"\n[mcp_servers.github]\ncommand = \"/x\"\n",
        )
        .unwrap();
        let outcome = remove_codex_table(&path, "mcp_servers", "famp").unwrap();
        assert_eq!(outcome, TomlMergeOutcome::Removed);
        let post: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(post["mcp_servers"]
            .as_table()
            .unwrap()
            .get("famp")
            .is_none());
        assert_eq!(
            post["mcp_servers"]["github"]["command"].as_str().unwrap(),
            "/x"
        );
    }

    #[test]
    fn remove_returns_not_present_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let outcome = remove_codex_table(&path, "mcp_servers", "famp").unwrap();
        assert_eq!(outcome, TomlMergeOutcome::NotPresent);
    }
}
