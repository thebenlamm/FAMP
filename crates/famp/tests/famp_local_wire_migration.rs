//! Static regression guard for the FAMP_HOME → register-first migration in
//! the archived `famp-local` script. Spec: 01-CONTEXT.md "Migration in
//! scripts/famp-local".
//!
//! The two behavioral tests that used to drive
//! `docs/history/v0.9-prep-sprint/famp-local/famp-local` via `Command::new`
//! bash were removed here (quick-260730-d9h). The FAMP_HOME →
//! register-first migration they validated completed in v0.9, and v1.0 has
//! shipped. CI `paths-ignore` excludes `docs/**`, so a change to the script
//! under test can never trigger a CI run — the tests could only ever fail
//! for environmental reasons, never for a real regression. Verified root
//! cause of the flake: the seeded port 58444 lies inside Linux's default
//! ephemeral port range 32768–60999, so on a loaded ubuntu runner a
//! transient outbound connection can already hold it, the python3 stub's
//! `bind()` raises, nothing ever listens, and the archived script's
//! 20 × 0.05s = 1s `lsof` readiness deadline fires with "bob: daemon failed
//! to bind port 58444 within 1s". That explains both the ubuntu-only
//! failure and the pass-on-rerun (CI run 30510970590).
//!
//! What remains is a pure static grep of the archived script — zero flake,
//! still a meaningful regression guard.

#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown
)]

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .to_path_buf()
}

fn script_path() -> PathBuf {
    workspace_root().join("docs/history/v0.9-prep-sprint/famp-local/famp-local")
}

#[test]
fn mcp_add_does_not_emit_famp_home() {
    let script = std::fs::read_to_string(script_path()).unwrap();
    // After 01-04, no `mcp add` invocation should pass FAMP_HOME via --env.
    // Static grep — duplicates the Task 1 acceptance gate inside CI.
    for (idx, line) in script.lines().enumerate() {
        let l = line.trim_start();
        if (l.starts_with("claude mcp add") || l.starts_with("codex mcp add"))
            && line.contains("FAMP_HOME")
        {
            panic!(
                "archived famp-local line {}: mcp-add invocation still emits FAMP_HOME:\n  {}",
                idx + 1,
                line
            );
        }
    }
}
