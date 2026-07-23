//! `famp hook codex-stop` — native Codex Stop-hook lifecycle.
//!
//! Fail-open: every uncertainty path exits 0 with no block decision.
//! Never shells out to `jq` / `python3` / `famp` on the critical path.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{self, AwaitArgs};
use crate::cli::error::CliError;

use super::codex_rollout;
use super::emit;
use super::lock::StopAwaitLock;
use super::log::log;
use super::pid_fallback;
use super::stdin::{self, StopHookInput};
use super::transcript::{self, validate_identity};

/// CLI args for `famp hook codex-stop`.
#[derive(Debug, Args)]
pub struct CodexStopArgs {
    /// Override await timeout (tests). Production default is 23h.
    /// Also overridable via `FAMP_HOOK_AWAIT_TIMEOUT` (humantime, e.g. `2s`).
    #[arg(long, hide = true)]
    pub timeout: Option<humantime::Duration>,
}

/// Production entry: read stdin, run the lifecycle, always return Ok (exit 0).
pub fn run(args: CodexStopArgs) -> Result<(), CliError> {
    // Fail-open wrapper: never surface non-zero to the host.
    if let Err(e) = run_inner(args) {
        log(&format!("codex-stop fail-open: {e}"));
    }
    Ok(())
}

fn run_inner(args: CodexStopArgs) -> Result<(), CliError> {
    log("hook invoked (native codex-stop)");
    let input = stdin::read_stop_hook_input();
    // Disconnect-equivalent: we already consumed stdin fully.

    if let Some(reason) = input.reason.as_deref() {
        if !reason.is_empty() && reason != "end_turn" {
            log(&format!(
                "session-end observe fire reason={reason}; exiting no-op"
            ));
            return Ok(());
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Io {
            path: PathBuf::new(),
            source: e,
        })?;
    rt.block_on(run_async(args, input))
}

async fn run_async(args: CodexStopArgs, input: StopHookInput) -> Result<(), CliError> {
    let transcript = resolve_transcript(&input);
    let mut identity = None;
    let mut explicitly_off = false;
    if let Some(ref path) = transcript {
        match transcript::extract_listen_state(path) {
            transcript::ListenState::Active(name) => identity = Some(name),
            transcript::ListenState::ExplicitlyOff => explicitly_off = true,
            transcript::ListenState::Unresolved => {
                log("transcript present but no listen registration found");
            }
        }
    } else {
        log("no transcript_path; trying pid-correlated fallback");
    }

    if explicitly_off {
        // Listen mode was explicitly turned off (register listen:false or
        // famp_set_listen(false)); do NOT let a stale pid-correlated broker
        // row re-arm listen mode against that explicit opt-out.
        log("listen mode explicitly off (opt-out); exiting no-op without pid fallback");
        return Ok(());
    }

    if identity.is_none() {
        identity = pid_fallback::resolve_via_pid(None).await;
    }

    let Some(identity) = identity else {
        log("no listen registration in transcript (and no pid-correlated listen identity); exiting no-op");
        return Ok(());
    };

    if !validate_identity(&identity) {
        log(&format!(
            "invalid identity from transcript: {identity}; exiting no-op"
        ));
        return Ok(());
    }

    log(&format!("listen mode active: identity={identity}"));

    let Some(_lock) = StopAwaitLock::try_acquire(&identity) else {
        return Ok(());
    };

    let timeout = resolve_timeout(args.timeout);
    let await_args = AwaitArgs {
        timeout: timeout.into(),
        task: None,
        act_as: Some(identity.clone()),
        // Codex does not arm the queue-watch abort path (parity item P14,
        // deferred — see docs/superpowers/specs/2026-07-23-codex-native-stop-hook-design.md).
        // `outcome.aborted` is therefore always false below; the check on it
        // is defensive only, kept in case abort is ever armed for Codex.
        abort_on_fd: None,
    };

    let sock = resolve_sock_path();
    let outcome = match await_cmd::run_at_structured(&sock, await_args).await {
        Ok(o) => o,
        Err(e) => {
            log(&format!(
                "await failed identity={identity}: {e}; fail-open exit 0"
            ));
            return Ok(());
        }
    };

    if outcome.aborted {
        log("aborted: host queue has pending input; fail-open exit 0 so host drains");
        return Ok(());
    }
    if outcome.timed_out {
        log("await timeout or empty; clean stop");
        return Ok(());
    }
    if outcome.envelopes.is_empty() {
        log("await empty envelopes; clean stop");
        return Ok(());
    }

    // Backup received envelope (best-effort, same as shell hook).
    backup_outcome(&outcome);

    let mut stdout = std::io::stdout().lock();
    let emitted = emit::emit_block_decision(&outcome, &identity, &mut stdout).await;
    if !emitted {
        log(&format!(
            "POST-WAKE EMIT FAILURE identity={identity} mailbox={:?} reason=emit_returned_false",
            outcome.mailbox
        ));
    }
    let _ = stdout.flush();
    Ok(())
}

fn resolve_transcript(input: &StopHookInput) -> Option<PathBuf> {
    if let Some(ref p) = input.transcript_path {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(ref sid) = input.session_id {
        return codex_rollout::resolve_rollout_path(sid);
    }
    None
}

fn resolve_timeout(cli: Option<humantime::Duration>) -> Duration {
    if let Some(t) = cli {
        return Duration::from(t);
    }
    if let Ok(s) = std::env::var("FAMP_HOOK_AWAIT_TIMEOUT") {
        if let Ok(d) = humantime::parse_duration(&s) {
            return d;
        }
    }
    Duration::from_secs(23 * 3600)
}

fn backup_outcome(outcome: &await_cmd::AwaitOutcome) {
    let state = if let Ok(p) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(p).join("famp")
    } else if let Ok(h) = std::env::var("HOME") {
        PathBuf::from(h).join(".local").join("state").join("famp")
    } else {
        return;
    };
    let backup_dir = state.join("received");
    if std::fs::create_dir_all(&backup_dir).is_err() {
        return;
    }
    let Ok(serialized) = serde_json::to_string(&serde_json::json!({
        "mailbox": match &outcome.mailbox {
            Some(famp_bus::MailboxName::Channel(n)) => serde_json::json!({"kind":"channel","name":n}),
            Some(famp_bus::MailboxName::Agent(n)) => serde_json::json!({"kind":"agent","name":n}),
            None => serde_json::Value::Null,
        },
        "envelopes": outcome.envelopes,
    })) else {
        return;
    };
    let fname = format!(
        "{}-{}-native.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        std::process::id()
    );
    let path = backup_dir.join(fname);
    if std::fs::write(&path, format!("{serialized}\n")).is_ok() {
        log(&format!("envelope backed up: {}", path.display()));
    }
}

/// Testable entry that takes pre-parsed input and writes to `out`.
#[cfg(test)]
pub async fn run_with_input_for_test(
    input: StopHookInput,
    timeout: Duration,
    sock: &std::path::Path,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let transcript = resolve_transcript(&input);
    let mut identity = None;
    let mut explicitly_off = false;
    if let Some(ref path) = transcript {
        match transcript::extract_listen_state(path) {
            transcript::ListenState::Active(name) => identity = Some(name),
            transcript::ListenState::ExplicitlyOff => explicitly_off = true,
            transcript::ListenState::Unresolved => {}
        }
    }
    if explicitly_off {
        // Same gating as run_async: an explicit opt-out must not be
        // overridden by a stale pid-correlated broker row.
        return Ok(());
    }
    if identity.is_none() {
        identity = pid_fallback::resolve_via_pid(Some(sock)).await;
    }
    let Some(identity) = identity else {
        return Ok(());
    };
    if !validate_identity(&identity) {
        return Ok(());
    }
    let await_args = AwaitArgs {
        timeout: timeout.into(),
        task: None,
        act_as: Some(identity.clone()),
        // Codex does not arm the queue-watch abort path (parity item P14,
        // deferred — see docs/superpowers/specs/2026-07-23-codex-native-stop-hook-design.md).
        // `outcome.aborted` is therefore always false below; the check on it
        // is defensive only, kept in case abort is ever armed for Codex.
        abort_on_fd: None,
    };
    let Ok(outcome) = await_cmd::run_at_structured(sock, await_args).await else {
        return Ok(());
    };
    if outcome.timed_out || outcome.aborted || outcome.envelopes.is_empty() {
        return Ok(());
    }
    let _ = emit::emit_block_decision(&outcome, &identity, out).await;
    Ok(())
}
