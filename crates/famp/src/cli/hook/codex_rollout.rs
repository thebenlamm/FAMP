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
    // Open read-only by path, not by URI: a CODEX_HOME containing `?`, `#` or
    // a space would silently produce a malformed `file:...?mode=ro` URI and
    // degrade to the glob path with no signal. Any error → None (fail-open).
    let conn =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
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
    walk_sessions(session_root, needle, &suffix, &mut matches, MAX_WALK_DEPTH);
    matches.sort_by(|a, b| b.0.cmp(&a.0));
    matches.into_iter().map(|(_, p)| p).next()
}

/// Rollouts live at `sessions/YYYY/MM/DD/rollout-*.jsonl`, so 3 levels of
/// directory below the root suffice; 6 is generous headroom.
const MAX_WALK_DEPTH: u32 = 6;

/// Recursive scan for rollout files.
///
/// Bounded two ways, because this runs on the Stop-hook critical path against a
/// directory the hook does not own: `depth` caps recursion, and symlinked
/// *directories* are never descended into, so a symlink cycle under
/// `$CODEX_HOME/sessions` cannot hang or overflow the stack. Symlinked rollout
/// *files* are still matched — they carry no cycle risk.
fn walk_sessions(
    dir: &Path,
    needle: &str,
    suffix: &str,
    out: &mut Vec<(SystemTime, PathBuf)>,
    depth: u32,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // symlink_metadata does not follow the link, so a symlinked directory
        // is identified as a symlink here and never descended into.
        let is_symlink = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink());
        if path.is_dir() {
            if is_symlink {
                log(&format!(
                    "rollout scan: not descending into symlinked dir {}",
                    path.display()
                ));
            } else if depth > 0 {
                walk_sessions(&path, needle, suffix, out, depth - 1);
            } else {
                log(&format!(
                    "rollout scan depth cap reached; skipping {}",
                    path.display()
                ));
            }
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
    fn sqlite_opens_when_path_has_uri_metacharacters() {
        // Regression: the old `file:{}?mode=ro` URI form silently produced a
        // malformed URI for a CODEX_HOME containing `?`, `#` or a space, and
        // degraded to the glob path with no log line.
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("cod ex?home#1");
        let sessions = home.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let rollout = sessions.join("rollout-weird.jsonl");
        std::fs::write(&rollout, "{}\n").unwrap();
        let db = home.join("state_5.sqlite");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute(
                "create table threads (id text primary key, rollout_path text)",
                [],
            )
            .unwrap();
            conn.execute(
                "insert into threads (id, rollout_path) values (?1, ?2)",
                rusqlite::params!["sid-weird", rollout.display().to_string()],
            )
            .unwrap();
        }
        assert_eq!(
            resolve_rollout_path_with_home("sid-weird", &home, &home),
            Some(rollout)
        );
    }

    #[test]
    #[cfg(unix)]
    fn symlink_cycle_terminates() {
        // sessions/loop -> sessions   (a cycle the old is_dir() recursion
        // would have followed until stack exhaustion).
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions/2026/07/20");
        std::fs::create_dir_all(&sessions).unwrap();
        let root = dir.path().join("sessions");
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let sid = "cycle-sid";
        let path = sessions.join(format!("rollout-2026-07-20T00-00-00-{sid}.jsonl"));
        std::fs::write(&path, "{}\n").unwrap();

        // Isolate the symlink guard from the depth cap: collect the full match
        // set directly. With the guard, the real file is found exactly once and
        // no path is reached *through* the `loop` symlink. Without the guard the
        // walk would also collect `sessions/loop/2026/07/20/rollout-...` (and,
        // absent the depth cap, hang) — so a regression surfaces as a
        // loop-traversed duplicate here, not as readdir-order luck.
        let suffix = format!("{sid}.jsonl");
        let mut matches: Vec<(SystemTime, PathBuf)> = Vec::new();
        walk_sessions(&root, "rollout-", &suffix, &mut matches, MAX_WALK_DEPTH);
        assert_eq!(matches.len(), 1, "found: {matches:?}");
        assert!(
            !matches[0].1.components().any(|c| c.as_os_str() == "loop"),
            "descended into symlinked dir: {:?}",
            matches[0].1
        );
        assert_eq!(rollout_from_glob(&root, sid).unwrap(), path);
    }

    #[test]
    fn depth_cap_stops_recursion() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("sessions");
        // One level deeper than MAX_WALK_DEPTH allows.
        let mut deep = root.clone();
        for i in 0..=MAX_WALK_DEPTH {
            deep = deep.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&deep).unwrap();
        let sid = "too-deep";
        std::fs::write(deep.join(format!("rollout-x-{sid}.jsonl")), "{}\n").unwrap();
        assert_eq!(rollout_from_glob(&root, sid), None);
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
