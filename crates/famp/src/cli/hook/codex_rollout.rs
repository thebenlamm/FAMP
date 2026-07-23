//! Resolve a Codex rollout path from `session_id` when `transcript_path` is absent.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::log::log;

/// Resolve a rollout JSONL path for `session_id` under Codex home.
///
/// Order:
/// 1. `state_5.sqlite` threads table (`CODEX_SQLITE_HOME` or `CODEX_HOME`)
/// 2. Glob `CODEX_HOME/sessions/**/rollout-*{session_id}.jsonl` (newest mtime wins)
///
/// Paths from sqlite must lie under `CODEX_HOME/sessions` (anti-hijack).
pub fn resolve_rollout_path(session_id: &str) -> Option<PathBuf> {
    if session_id.is_empty() {
        return None;
    }
    let home = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_default();
    let codex_home = std::env::var("CODEX_HOME").map_or_else(
        |_| {
            if home.as_os_str().is_empty() {
                PathBuf::new()
            } else {
                home.join(".codex")
            }
        },
        PathBuf::from,
    );
    if codex_home.as_os_str().is_empty() {
        return None;
    }
    let sqlite_home =
        std::env::var("CODEX_SQLITE_HOME").map_or_else(|_| codex_home.clone(), PathBuf::from);
    resolve_rollout_path_with_home(session_id, &codex_home, &sqlite_home)
}

/// Explicit-home variant (tests + callers that already resolved CODEX_HOME).
pub fn resolve_rollout_path_with_home(
    session_id: &str,
    codex_home: &Path,
    sqlite_home: &Path,
) -> Option<PathBuf> {
    if session_id.is_empty() || codex_home.as_os_str().is_empty() {
        return None;
    }
    let session_root = codex_home.join("sessions");

    for db in [
        sqlite_home.join("state_5.sqlite"),
        codex_home.join("state_5.sqlite"),
    ] {
        if !db.is_file() {
            continue;
        }
        if let Some(path) = rollout_from_sqlite(&db, session_id) {
            if allowed_rollout_path(&path, &session_root) {
                log(&format!(
                    "resolved transcript from Codex sqlite session_id={session_id}"
                ));
                return Some(path);
            }
            log(&format!(
                "sqlite rollout_path outside sessions root; ignored session_id={session_id}"
            ));
        }
    }

    if let Some(path) = rollout_from_glob(&session_root, session_id) {
        log(&format!(
            "resolved transcript from Codex session_id={session_id}"
        ));
        return Some(path);
    }
    None
}

fn rollout_from_sqlite(db: &Path, session_id: &str) -> Option<PathBuf> {
    // Open read-only; any error → None (fail-open).
    let uri = format!("file:{}?mode=ro", db.display());
    let conn = rusqlite::Connection::open_with_flags(
        &uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    let mut stmt = conn
        .prepare("select rollout_path from threads where id = ?1")
        .ok()?;
    let path: String = stmt.query_row([session_id], |row| row.get(0)).ok()?;
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn allowed_rollout_path(path: &Path, session_root: &Path) -> bool {
    if !path.is_file() || !session_root.exists() {
        return false;
    }
    let Ok(real_path) = path.canonicalize() else {
        return false;
    };
    let Ok(real_root) = session_root.canonicalize() else {
        return false;
    };
    real_path.starts_with(&real_root)
}

fn rollout_from_glob(session_root: &Path, session_id: &str) -> Option<PathBuf> {
    if !session_root.is_dir() {
        return None;
    }
    let needle = "rollout-";
    let suffix = format!("{session_id}.jsonl");
    let mut matches: Vec<(SystemTime, PathBuf)> = Vec::new();
    walk_sessions(session_root, needle, &suffix, &mut matches);
    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches.into_iter().map(|(_, p)| p).next()
}

fn walk_sessions(dir: &Path, needle: &str, suffix: &str, out: &mut Vec<(SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_sessions(&path, needle, suffix, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with(needle) && name.ends_with(suffix) && path.is_file() {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((mtime, path));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn glob_finds_session_rollout() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions/2026/07/20");
        std::fs::create_dir_all(&sessions).unwrap();
        let sid = "019f824d-971f-7ec1-8c9b-8929d3f97c7a";
        let path = sessions.join(format!("rollout-2026-07-20T21-32-30-{sid}.jsonl"));
        std::fs::write(&path, "{}\n").unwrap();
        let found = rollout_from_glob(&dir.path().join("sessions"), sid).unwrap();
        assert_eq!(found, path);
    }

    #[test]
    fn sqlite_resolves_when_under_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let rollout = sessions.join("db-only-rollout.jsonl");
        std::fs::write(&rollout, "{}\n").unwrap();
        let db = dir.path().join("state_5.sqlite");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "create table threads (id text primary key, rollout_path text)",
                [],
            )
            .unwrap();
            conn.execute(
                "insert into threads (id, rollout_path) values (?1, ?2)",
                rusqlite::params!["sid-1", rollout.display().to_string()],
            )
            .unwrap();
        }
        let got = rollout_from_sqlite(&db, "sid-1").unwrap();
        assert!(allowed_rollout_path(&got, &sessions));
    }

    #[test]
    fn rejects_outside_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let outside = dir.path().join("outside.jsonl");
        let mut f = std::fs::File::create(&outside).unwrap();
        writeln!(f, "{{}}").unwrap();
        assert!(!allowed_rollout_path(&outside, &sessions));
    }
}
