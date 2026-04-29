//! D-01 hybrid identity resolver.
//!
//! Walks the four-tier resolution chain that mirrors `scripts/famp-local`
//! `cmd_identity_of` byte-for-byte:
//!
//! 1. Explicit `--as <name>` flag (passed in as `as_flag`).
//! 2. `$FAMP_LOCAL_IDENTITY` environment variable.
//! 3. cwd → identity exact match against `~/.famp-local/wires.tsv`.
//! 4. Hard error — no identity bound.
//!
//! Tiers 1-3 succeed fast; tier-4 returns the literal hint message
//! `"no identity bound — pass --as, set $FAMP_LOCAL_IDENTITY, or run
//! `famp-local wire <dir>` first"` so users get a single-line nudge to
//! one of the three resolution paths.
//!
//! The wires.tsv lookup canonicalizes both sides (cwd and the row's
//! directory column) before comparing, matching the bash script's
//! exact-match semantics while dodging symlink and trailing-slash
//! mismatches.

use std::path::{Path, PathBuf};

use crate::cli::error::CliError;

const NO_IDENTITY_HINT: &str =
    "no identity bound — pass --as, set $FAMP_LOCAL_IDENTITY, or run `famp-local wire <dir>` first";

/// Resolve the active identity per the D-01 four-tier stack.
///
/// `as_flag` carries the value of the `--as` CLI flag (or `None` if the
/// flag was not passed). Empty strings pass through tiers 1 and 2 (i.e.
/// an explicit `""` is treated as "not provided").
pub fn resolve_identity(as_flag: Option<&str>) -> Result<String, CliError> {
    // Tier 1: explicit flag.
    if let Some(name) = as_flag {
        if !name.is_empty() {
            return Ok(name.to_string());
        }
    }
    // Tier 2: environment variable.
    if let Ok(name) = std::env::var("FAMP_LOCAL_IDENTITY") {
        if !name.is_empty() {
            return Ok(name);
        }
    }
    // Tier 3: cwd → wires.tsv exact match.
    if let Some(name) = lookup_wires_tsv()? {
        return Ok(name);
    }
    // Tier 4: hard error.
    Err(CliError::NoIdentityBound {
        reason: NO_IDENTITY_HINT.to_string(),
    })
}

fn wires_tsv_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".famp-local").join("wires.tsv"))
}

/// Read `~/.famp-local/wires.tsv` and return the identity bound to the
/// caller's current working directory, if any. Each line is two
/// tab-separated fields: `<absolute-canonical-dir>\t<identity-name>`.
/// Both the cwd and the row's directory are `canonicalize`'d before
/// comparison so symlinks and trailing slashes do not produce misses.
fn lookup_wires_tsv() -> Result<Option<String>, CliError> {
    let cwd = std::env::current_dir().map_err(|e| CliError::Io {
        path: PathBuf::new(),
        source: e,
    })?;
    // WR-02: keep both the canonical and the raw cwd. The fallback
    // comparison is `row_raw == cwd_raw` so a deleted-but-still-open
    // working directory (canonicalize fails) still matches a
    // verbatim-text row from `wires.tsv`. Comparing the raw row against
    // the canonical cwd (the previous code) was dead — for that to
    // match, the row's on-disk text would already equal the canonical
    // cwd path, in which case the canonical comparison already matched.
    let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
    let cwd_raw = cwd;

    let Some(wires_path) = wires_tsv_path() else {
        return Ok(None);
    };
    let Ok(content) = std::fs::read_to_string(&wires_path) else {
        return Ok(None);
    };
    for line in content.lines() {
        let mut parts = line.splitn(2, '\t');
        let (Some(dir_str), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if dir_str.is_empty() || name.is_empty() {
            continue;
        }
        let row_canon = Path::new(dir_str)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(dir_str));
        if row_canon == cwd_canon || Path::new(dir_str) == cwd_raw {
            return Ok(Some(name.to_string()));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Tier 1: explicit `--as` flag wins, no I/O happens.
    #[test]
    fn tier_1_as_flag_wins() {
        let got = resolve_identity(Some("alice")).unwrap();
        assert_eq!(got, "alice");
    }

    /// Tier 2: `$FAMP_LOCAL_IDENTITY` env var when no flag is passed.
    /// Env mutation in tests is process-global; we save and restore.
    #[test]
    fn tier_2_env_var_wins_when_no_flag() {
        let prev = std::env::var("FAMP_LOCAL_IDENTITY").ok();
        std::env::set_var("FAMP_LOCAL_IDENTITY", "bob");
        let got = resolve_identity(None).unwrap();
        assert_eq!(got, "bob");
        match prev {
            Some(v) => std::env::set_var("FAMP_LOCAL_IDENTITY", v),
            None => std::env::remove_var("FAMP_LOCAL_IDENTITY"),
        }
    }

    /// Tier 2: empty env var falls through (does not match).
    #[test]
    fn tier_2_empty_env_var_falls_through() {
        let prev_env = std::env::var("FAMP_LOCAL_IDENTITY").ok();
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("FAMP_LOCAL_IDENTITY", "");
        // Force tier 3 to fail by pointing HOME at a fresh empty tempdir
        // with no wires.tsv.
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        let res = resolve_identity(None);
        // Restore env BEFORE asserts so a panic doesn't leak state.
        match prev_env {
            Some(v) => std::env::set_var("FAMP_LOCAL_IDENTITY", v),
            None => std::env::remove_var("FAMP_LOCAL_IDENTITY"),
        }
        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let err = res.expect_err("tier 4 hard error expected");
        match err {
            CliError::NoIdentityBound { reason } => {
                assert!(reason.starts_with("no identity bound"), "{reason}");
            }
            other => panic!("expected NoIdentityBound, got {other:?}"),
        }
    }

    /// Tier 3: cwd matches a row in `wires.tsv`. We point `HOME` at a
    /// tempdir with a hand-rolled `.famp-local/wires.tsv` so the test
    /// is hermetic.
    #[test]
    fn tier_3_wires_tsv_match() {
        let prev_env = std::env::var("FAMP_LOCAL_IDENTITY").ok();
        let prev_home = std::env::var("HOME").ok();
        std::env::remove_var("FAMP_LOCAL_IDENTITY");
        let tmp = tempfile::tempdir().unwrap();
        let local_dir = tmp.path().join(".famp-local");
        std::fs::create_dir_all(&local_dir).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        std::fs::write(
            local_dir.join("wires.tsv"),
            format!("{}\tcharlie\n", cwd_canon.display()),
        )
        .unwrap();
        std::env::set_var("HOME", tmp.path());
        let res = resolve_identity(None);
        match prev_env {
            Some(v) => std::env::set_var("FAMP_LOCAL_IDENTITY", v),
            None => std::env::remove_var("FAMP_LOCAL_IDENTITY"),
        }
        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let got = res.expect("tier-3 hit expected");
        assert_eq!(got, "charlie");
    }

    /// Tier 4: nothing matches → hard error with the literal hint.
    #[test]
    fn tier_4_hard_error_with_hint() {
        let prev_env = std::env::var("FAMP_LOCAL_IDENTITY").ok();
        let prev_home = std::env::var("HOME").ok();
        std::env::remove_var("FAMP_LOCAL_IDENTITY");
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        let res = resolve_identity(None);
        match prev_env {
            Some(v) => std::env::set_var("FAMP_LOCAL_IDENTITY", v),
            None => std::env::remove_var("FAMP_LOCAL_IDENTITY"),
        }
        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let err = res.expect_err("tier-4 hard error expected");
        let msg = format!("{err}");
        assert!(msg.contains("no identity bound"), "{msg}");
        assert!(msg.contains("--as"), "{msg}");
        assert!(msg.contains("$FAMP_LOCAL_IDENTITY"), "{msg}");
        assert!(msg.contains("famp-local wire"), "{msg}");
    }
}
