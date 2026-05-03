//! Atomic structural-merge JSON helper for `~/.claude.json` and
//! `~/.claude/settings.json` (D-02, D-09 amended).
//!
//! Read-mutate-write with backup discipline:
//!  1. Read existing JSON (preserve every unrelated key).
//!  2. Mutate only the leaf at `root[parent_key][leaf_key]`.
//!  3. Backup pre-state to `<path>.bak.<unix-ts>` IF file existed.
//!  4. Atomic write via `tempfile::NamedTempFile::new_in(parent_dir)` +
//!     `persist()` - rename(2) is atomic only across files on the same
//!     filesystem.
//!
//! Pattern derives from `crates/famp/src/cli/config.rs::write_peers_atomic`
//! (TOML version of the same idiom). [Reused: tempfile + NamedTempFile + persist.]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use tempfile::NamedTempFile;

use crate::cli::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    Inserted,
    Updated,
    AlreadyMatches,
    Removed,
    NotPresent,
}

/// Idempotent upsert: `root[parent_key][leaf_key] = value`.
///
/// Creates `parent_key` as an empty object if absent. Reads file as
/// `serde_json::Value`, mutates only the leaf, atomically rewrites the
/// whole file (preserving every unrelated key).
///
/// Returns `AlreadyMatches` if the leaf already equals `value` (no write
/// occurs - D-02 idempotency).
pub fn upsert_user_json(
    path: &Path,
    parent_key: &str,
    leaf_key: &str,
    value: Value,
) -> Result<MergeOutcome, CliError> {
    let pb: PathBuf = path.to_path_buf();

    let mut root: Value = match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| CliError::JsonMergeParse {
            path: pb.clone(),
            source: e,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => {
            return Err(CliError::JsonMergeRead {
                path: pb,
                source: e,
            });
        }
    };

    let obj = root
        .as_object_mut()
        .ok_or_else(|| CliError::JsonMergeNotObject { path: pb.clone() })?;

    let parent = obj
        .entry(parent_key.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| CliError::JsonMergeNotObject {
            path: PathBuf::from(format!("{}#/{parent_key}", pb.display())),
        })?;

    let outcome = match parent.get(leaf_key) {
        Some(existing) if existing == &value => return Ok(MergeOutcome::AlreadyMatches),
        Some(_) => MergeOutcome::Updated,
        None => MergeOutcome::Inserted,
    };
    parent.insert(leaf_key.to_string(), value);

    backup_if_exists(path)?;
    persist_json(path, &pb, &root)?;
    Ok(outcome)
}

/// Symmetric remove: drop `root[parent_key][leaf_key]` if present.
///
/// Returns `NotPresent` if the leaf doesn't exist (no write occurs).
/// Same atomic + backup discipline as `upsert_user_json`.
pub fn remove_user_json(
    path: &Path,
    parent_key: &str,
    leaf_key: &str,
) -> Result<MergeOutcome, CliError> {
    let pb: PathBuf = path.to_path_buf();

    let mut root: Value = match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| CliError::JsonMergeParse {
            path: pb.clone(),
            source: e,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(MergeOutcome::NotPresent),
        Err(e) => {
            return Err(CliError::JsonMergeRead {
                path: pb,
                source: e,
            });
        }
    };

    let obj = root
        .as_object_mut()
        .ok_or_else(|| CliError::JsonMergeNotObject { path: pb.clone() })?;

    let Some(parent_val) = obj.get_mut(parent_key) else {
        return Ok(MergeOutcome::NotPresent);
    };
    let Some(parent) = parent_val.as_object_mut() else {
        return Err(CliError::JsonMergeNotObject {
            path: PathBuf::from(format!("{}#/{parent_key}", pb.display())),
        });
    };

    if parent.remove(leaf_key).is_none() {
        return Ok(MergeOutcome::NotPresent);
    }

    backup_if_exists(path)?;
    persist_json(path, &pb, &root)?;
    Ok(MergeOutcome::Removed)
}

fn backup_if_exists(path: &Path) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("json");
    let bak = path.with_extension(format!("{ext}.bak.{ts}"));
    std::fs::copy(path, &bak).map_err(|e| CliError::JsonMergeBackup {
        path: bak,
        source: e,
    })?;
    Ok(())
}

fn persist_json(path: &Path, display_path: &Path, root: &Value) -> Result<(), CliError> {
    let parent_dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent_dir).map_err(|e| CliError::JsonMergePersist {
        path: parent_dir.to_path_buf(),
        source: e,
    })?;
    let mut tmp = NamedTempFile::new_in(parent_dir).map_err(|e| CliError::JsonMergePersist {
        path: parent_dir.to_path_buf(),
        source: e,
    })?;
    let serialized = serde_json::to_string_pretty(root).map_err(|e| CliError::JsonMergeParse {
        path: display_path.to_path_buf(),
        source: e,
    })?;
    tmp.write_all(serialized.as_bytes())
        .map_err(|e| CliError::JsonMergePersist {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;
    tmp.as_file_mut()
        .sync_all()
        .map_err(|e| CliError::JsonMergePersist {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;
    tmp.persist(path).map_err(|e| CliError::JsonMergePersist {
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
    use serde_json::json;

    fn fixture_with_other_keys() -> Value {
        json!({
            "numStartups": 42,
            "tipsHistory": ["a", "b"],
            "mcpServers": {
                "other-server": {"command": "/usr/bin/other", "args": []}
            }
        })
    }

    #[test]
    fn insert_into_existing_file_preserves_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&fixture_with_other_keys()).unwrap(),
        )
        .unwrap();

        let famp_value = json!({
            "type": "stdio",
            "command": "/Users/test/.cargo/bin/famp",
            "args": ["mcp"],
        });
        let outcome = upsert_user_json(&path, "mcpServers", "famp", famp_value.clone()).unwrap();
        assert_eq!(outcome, MergeOutcome::Inserted);

        let post: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(post["numStartups"], json!(42));
        assert_eq!(post["tipsHistory"], json!(["a", "b"]));
        assert_eq!(
            post["mcpServers"]["other-server"]["command"],
            "/usr/bin/other"
        );
        assert_eq!(post["mcpServers"]["famp"], famp_value);
    }

    #[test]
    fn second_upsert_with_same_value_is_no_op_already_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(&path, "{}").unwrap();
        let v = json!({"command": "/x"});
        assert_eq!(
            upsert_user_json(&path, "mcpServers", "famp", v.clone()).unwrap(),
            MergeOutcome::Inserted
        );
        assert_eq!(
            upsert_user_json(&path, "mcpServers", "famp", v).unwrap(),
            MergeOutcome::AlreadyMatches
        );
    }

    #[test]
    fn upsert_creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        let v = json!({"command": "/y"});
        let outcome = upsert_user_json(&path, "mcpServers", "famp", v.clone()).unwrap();
        assert_eq!(outcome, MergeOutcome::Inserted);
        assert!(path.exists());
        let post: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(post["mcpServers"]["famp"], v);
    }

    #[test]
    fn upsert_with_different_value_returns_updated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(&path, r#"{"mcpServers":{"famp":{"command":"/old"}}}"#).unwrap();
        let new = json!({"command": "/new"});
        let outcome = upsert_user_json(&path, "mcpServers", "famp", new.clone()).unwrap();
        assert_eq!(outcome, MergeOutcome::Updated);
        let post: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(post["mcpServers"]["famp"], new);
    }

    #[test]
    fn upsert_rejects_non_object_root() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        let err = upsert_user_json(&path, "mcpServers", "famp", json!({})).unwrap_err();
        assert!(matches!(err, CliError::JsonMergeNotObject { .. }));
    }

    #[test]
    fn remove_returns_not_present_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        let outcome = remove_user_json(&path, "mcpServers", "famp").unwrap();
        assert_eq!(outcome, MergeOutcome::NotPresent);
    }

    #[test]
    fn remove_drops_only_target_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            r#"{"numStartups":7,"mcpServers":{"famp":{"command":"/x"},"other":{"command":"/y"}}}"#,
        )
        .unwrap();
        let outcome = remove_user_json(&path, "mcpServers", "famp").unwrap();
        assert_eq!(outcome, MergeOutcome::Removed);
        let post: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(post["numStartups"], json!(7));
        assert_eq!(post["mcpServers"]["other"]["command"], "/y");
        assert!(post["mcpServers"]
            .as_object()
            .unwrap()
            .get("famp")
            .is_none());
    }

    #[test]
    fn upsert_creates_backup_when_file_existed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.json");
        std::fs::write(&path, r#"{"mcpServers":{}}"#).unwrap();
        let _ = upsert_user_json(&path, "mcpServers", "famp", json!({"command": "/x"})).unwrap();
        let baks: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("claude.json.bak.")
            })
            .collect();
        assert_eq!(baks.len(), 1, "exactly one .bak.<ts> file expected");
    }
}
