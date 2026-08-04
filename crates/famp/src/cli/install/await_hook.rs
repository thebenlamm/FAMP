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
use crate::cli::executable::{posix_shell_literal, FampExecutable};
use crate::cli::install::json_merge::MergeOutcome;

/// The bash await-shim source, embedded at compile time.
pub const FAMP_AWAIT_SH: &str = include_str!("../../../assets/famp-await.sh");

/// Write the await shim to `path` at mode 0755. Idempotent (overwrites existing).
/// Creates parent directories if absent.
pub(crate) fn install_shim(path: &Path, executable: &FampExecutable) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let rendered = FAMP_AWAIT_SH.replace("@FAMP_BIN@", &posix_shell_literal(executable.utf8()));
    std::fs::write(path, rendered).map_err(|source| CliError::Io {
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
/// Remove the shim, reporting whether it was actually there.
///
/// See `hook_runner::remove_shim` for why the outcome is returned rather than
/// swallowed: an uninstaller that claims removals it did not perform converts
/// a diagnostic into a false all-clear.
pub fn remove_shim(path: &Path) -> Result<MergeOutcome, CliError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(MergeOutcome::Removed),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MergeOutcome::NotPresent),
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
        assert!(FAMP_AWAIT_SH.contains("\"$FAMP_BIN\" await"));
    }

    /// 260721: the shipped shim MUST carry the compaction-resilience
    /// fallback so a compacted transcript (which drops the famp_register
    /// marker out of the 2 MB scan window) can still resolve its identity.
    /// Shipping this in the asset — not a hand-patched installed file — is
    /// the whole point: a reinstall must never silently revert it.
    #[test]
    fn shim_has_pid_correlated_fallback() {
        // Resolves identity by correlating THIS window's `famp mcp` server
        // via process ancestry, then mapping its pid to a name through
        // `famp sessions`, then confirming listen via `inspect`.
        assert!(FAMP_AWAIT_SH.contains("pid-correlated"));
        assert!(FAMP_AWAIT_SH.contains("SIBLING_MCP_PIDS"));
        assert!(FAMP_AWAIT_SH.contains("inspect identities --json"));
    }

    /// Anti-hijack invariant (260721): the fallback MUST NOT adopt an
    /// identity merely because it is registered in the same cwd — that would
    /// convert an innocent, never-registered window sharing the checkout
    /// into an awaiter on another agent's identity. Adoption keys on process
    /// ancestry, so the old cwd-matching heuristic must stay gone.
    #[test]
    fn shim_does_not_adopt_by_cwd() {
        assert!(
            FAMP_AWAIT_SH.contains("ANCESTORS"),
            "fallback must resolve identity via process ancestry"
        );
        assert!(
            !FAMP_AWAIT_SH.contains("!AMBIGUOUS"),
            "the cwd-ambiguity sentinel must be gone (it implied cwd-based adoption)"
        );
    }

    /// #26: agent-mailbox wake notification must prefer disk-ack
    /// `mailbox_unread` over the raw await-batch length so a re-arm that
    /// replays historical envelopes does not claim "N new messages" when
    /// `famp_inbox` is already past them.
    #[test]
    fn shim_prefers_disk_ack_unread_for_agent_count() {
        assert!(
            FAMP_AWAIT_SH.contains("mailbox_unread"),
            "hook must consult inspect identities mailbox_unread for agent wakes"
        );
        assert!(
            FAMP_AWAIT_SH.contains("disk-ack unread=0"),
            "hook must suppress wake when disk-ack unread is zero (#26)"
        );
        assert!(
            FAMP_AWAIT_SH.contains("AWAIT_BATCH_COUNT"),
            "hook must retain the await-batch count for diagnostics / channel path"
        );
    }

    /// Hermetic tests set this so a live host `famp mcp` cannot turn a
    /// deliberate no-op transcript into listen-mode via pid-correlation.
    #[test]
    fn shim_honors_disable_pid_fallback_env() {
        assert!(
            FAMP_AWAIT_SH.contains("FAMP_DISABLE_PID_FALLBACK"),
            "hook must gate the pid-correlated fallback behind an env opt-out"
        );
    }

    /// Dual-hook guard (B2): only one Stop await per identity when both
    /// Grok native and Claude-compat Stop entries fire.
    #[test]
    fn shim_has_stop_await_singleton_lock() {
        assert!(
            FAMP_AWAIT_SH.contains("stop-await-locks")
                || FAMP_AWAIT_SH.contains("stop-await singleton"),
            "hook must singleton-lock per-identity Stop await"
        );
    }

    /// Hosts that omit transcript_path (some Codex/Grok Stop payloads) must
    /// still try the PID-correlated fallback instead of immediate no-op.
    #[test]
    fn shim_tries_pid_fallback_when_transcript_missing() {
        assert!(
            FAMP_AWAIT_SH.contains("no transcript_path; trying pid-correlated fallback"),
            "missing transcript_path must fall through to pid-correlated fallback"
        );
        // Guard against regressing to the old immediate exit.
        assert!(
            !FAMP_AWAIT_SH.contains("no transcript_path; exiting no-op"),
            "must not immediately no-op on missing transcript_path"
        );
    }

    /// Grok stdin is camelCase; Claude/Codex use snake_case — both required.
    #[test]
    fn shim_accepts_snake_and_camel_case_keys() {
        assert!(
            FAMP_AWAIT_SH.contains("transcriptPath") && FAMP_AWAIT_SH.contains("transcript_path"),
            "must parse both transcript_path and transcriptPath"
        );
        assert!(
            FAMP_AWAIT_SH.contains("sessionId") && FAMP_AWAIT_SH.contains("session_id"),
            "must parse both session_id and sessionId"
        );
    }

    /// Grok fires Stop at session end with reason channel_closed/shutdown —
    /// must not park on those observe fires.
    #[test]
    fn shim_skips_session_end_observe_fire() {
        assert!(
            FAMP_AWAIT_SH.contains("session-end observe fire"),
            "must log and exit on non-end_turn reason"
        );
        assert!(
            FAMP_AWAIT_SH.contains("end_turn"),
            "must only park on empty reason or end_turn"
        );
    }

    #[test]
    fn shim_header_mentions_grok() {
        assert!(
            FAMP_AWAIT_SH.contains("Claude Code + Codex + Grok"),
            "header must list Grok alongside Claude/Codex"
        );
    }

    #[test]
    fn install_shim_creates_file_at_mode_0755() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude/hooks/famp-await.sh");
        let executable = FampExecutable::validate(std::env::current_exe().unwrap()).unwrap();
        install_shim(&path, &executable).unwrap();
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
        let executable = FampExecutable::validate(std::env::current_exe().unwrap()).unwrap();
        install_shim(&path, &executable).unwrap();
        install_shim(&path, &executable).unwrap();
    }

    #[test]
    fn remove_shim_after_install_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("famp-await.sh");
        let executable = FampExecutable::validate(std::env::current_exe().unwrap()).unwrap();
        install_shim(&path, &executable).unwrap();
        remove_shim(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_shim_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent-famp-await.sh");
        remove_shim(&path).unwrap();
    }

    // ── Template-render invariants (both shipped hook assets) ─────────────
    //
    // `famp-await.sh` and `hook-runner.sh` are rendered by the same
    // single-token substitution, so the matrix below covers both sources
    // from one place rather than duplicating it in `hook_runner.rs`.

    use crate::cli::install::hook_runner::HOOK_RUNNER_SH;

    const TEMPLATES: [(&str, &str); 2] = [
        ("famp-await.sh", FAMP_AWAIT_SH),
        ("hook-runner.sh", HOOK_RUNNER_SH),
    ];

    /// Every source template carries exactly one `@FAMP_BIN@` — more than one
    /// would mean a second, unreviewed interpolation site; zero would mean the
    /// resolved path never reaches the script.
    #[test]
    fn token_appears_exactly_once_in_each_template() {
        for (label, template) in TEMPLATES {
            assert_eq!(
                template.matches("@FAMP_BIN@").count(),
                1,
                "{label} must contain exactly one @FAMP_BIN@ token"
            );
            assert!(
                template.contains("\nFAMP_BIN=@FAMP_BIN@\n"),
                "{label}'s only token must be the FAMP_BIN assignment"
            );
        }
    }

    /// Stage an executable at `path`, creating parents. Returns the validated
    /// `FampExecutable`, proving the resolver accepts the hostile path too.
    fn stage(path: &Path) -> FampExecutable {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        FampExecutable::validate(path.to_path_buf()).unwrap()
    }

    /// The hostile file names every rendered hook must survive. Each is a
    /// real file created on disk, not just a string.
    fn hostile_names() -> Vec<&'static str> {
        let mut names = vec![
            "famp with space",
            "famp's binary",
            "famp$HOME${x}",
            "famp`touch CANARY`",
            "famp$(touch CANARY)",
            "famp;touch CANARY",
            "famp&&touch CANARY",
            "famp|touch CANARY",
            "famp\\backslash\\",
            "famp*?[glob]",
            "famp-ünïcode-日本語-🚀",
            "famp\"double\"quote",
            "famp\ttab",
        ];
        // A newline is a legal Unix path byte; Windows forbids it.
        #[cfg(unix)]
        names.push("famp\nsecond line");
        names
    }

    /// Render both templates with each hostile path and prove three things:
    /// the script still parses under its own interpreter (`bash -n`), the
    /// variable round-trips byte-for-byte when evaluated and quoted, and no
    /// second command ever runs (canary file never appears).
    #[cfg(unix)]
    #[test]
    fn hostile_executable_paths_render_safely_into_both_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let canary = std::env::current_dir().unwrap().join("CANARY");
        for (index, name) in hostile_names().into_iter().enumerate() {
            let executable = stage(&dir.path().join(format!("h{index}")).join(name));
            let expected_assignment = format!(
                "FAMP_BIN={}",
                crate::cli::executable::posix_shell_literal(executable.utf8())
            );

            for (label, _) in TEMPLATES {
                let rendered_path = dir.path().join(format!("rendered{index}")).join(label);
                if label == "famp-await.sh" {
                    install_shim(&rendered_path, &executable).unwrap();
                } else {
                    crate::cli::install::hook_runner::install_shim(&rendered_path, &executable)
                        .unwrap();
                }
                let rendered = std::fs::read_to_string(&rendered_path).unwrap();
                assert!(
                    rendered.contains(&expected_assignment),
                    "{label} must pin {name:?} as a single quoted word"
                );
                assert!(
                    !rendered.contains("@FAMP_BIN@"),
                    "{label} must not leave the token behind for {name:?}"
                );

                // 1. Syntactically valid under the script's real interpreter.
                let syntax = std::process::Command::new("bash")
                    .arg("-n")
                    .arg(&rendered_path)
                    .output()
                    .unwrap();
                assert!(
                    syntax.status.success(),
                    "bash -n failed for {label} with {name:?}: {}",
                    String::from_utf8_lossy(&syntax.stderr)
                );

                // 2/3. Round-trip + no injected second command.
                let probe = dir.path().join(format!("probe{index}"));
                let out = dir.path().join(format!("out{index}"));
                std::fs::write(
                    &probe,
                    format!(
                        "#!/usr/bin/env bash\nset -uo pipefail\n{expected_assignment}\nprintf '%s' \"$FAMP_BIN\" > {}\n",
                        crate::cli::executable::posix_shell_literal(out.to_str().unwrap())
                    ),
                )
                .unwrap();
                let status = std::process::Command::new("bash")
                    .arg(&probe)
                    .output()
                    .unwrap();
                assert!(
                    status.status.success(),
                    "probe failed for {label} with {name:?}: {}",
                    String::from_utf8_lossy(&status.stderr)
                );
                assert_eq!(
                    std::fs::read_to_string(&out).unwrap(),
                    executable.utf8(),
                    "{label}: $FAMP_BIN must round-trip byte-for-byte for {name:?}"
                );
                assert!(
                    !canary.exists() && !dir.path().join("CANARY").exists(),
                    "{label}: evaluating the pin for {name:?} executed a second command"
                );
            }
        }
    }

    /// The rendered hooks must depend on the pinned path alone: no PATH
    /// lookup, no `~/.cargo/bin/famp` fallback, no bare `famp` invocation.
    #[test]
    fn rendered_hooks_never_fall_back_to_path_or_cargo() {
        let dir = tempfile::tempdir().unwrap();
        let executable = stage(&dir.path().join("bin").join("famp"));
        let await_path = dir.path().join("famp-await.sh");
        let runner_path = dir.path().join("hook-runner.sh");
        install_shim(&await_path, &executable).unwrap();
        crate::cli::install::hook_runner::install_shim(&runner_path, &executable).unwrap();

        let bare = regex::Regex::new(r#"(^|[^"$/\w-])famp\s+(send|await)\b"#).unwrap();
        // Guard the guard: the pattern must actually catch the regression it
        // exists for, and must not flag the pinned form.
        assert!(bare.is_match("    famp send --to peer\n"));
        assert!(bare.is_match("famp await --as dk --timeout 23h"));
        assert!(!bare.is_match("\"$FAMP_BIN\" send --to peer"));
        assert!(!bare.is_match("\"$FAMP_BIN\" await --as dk"));
        for (label, source, path) in [
            ("famp-await.sh", FAMP_AWAIT_SH, await_path),
            ("hook-runner.sh", HOOK_RUNNER_SH, runner_path),
        ] {
            let rendered = std::fs::read_to_string(&path).unwrap();
            for (what, body) in [("source", source), ("rendered", rendered.as_str())] {
                assert!(
                    !body.contains("command -v famp"),
                    "{label} ({what}) must not probe PATH for famp"
                );
                assert!(
                    !body.contains(".cargo/bin/famp"),
                    "{label} ({what}) must not carry the cargo fallback"
                );
                assert!(
                    !bare.is_match(body),
                    "{label} ({what}) must not invoke a bare `famp send`/`famp await`"
                );
            }
            assert!(!rendered.contains("@FAMP_BIN@"));
            assert!(rendered.contains(&format!(
                "FAMP_BIN={}",
                crate::cli::executable::posix_shell_literal(executable.utf8())
            )));
        }
    }
}
