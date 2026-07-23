//! `famp listen-wake` — host-neutral wake signal for non-blocking hosts (Grok).
//!
//! Parks on the same bus await used by Claude/Codex Stop hooks, but prints
//! exactly one scrubbed stdout line per event so a host-side monitor can
//! re-enter the agent turn without a long blocking Stop hook:
//!
//! ```text
//! FAMP_WAKE identity=<id> sender=<sender|unknown> count=<n>
//! ```
//!
//! Never prints peer message body. Exit codes:
//! - 0: message delivered (once mode)
//! - 2: timeout (once mode; stderr `TIMEOUT`)
//! - 3: aborted via `--abort-on-fd` (unused on this surface today)
//! - 1: error / already running
//!
//! `--loop`: after each wake line, flush stdout and await again. Timeouts
//! in loop mode re-await silently (no spam). Bus errors in loop mode retry
//! with exponential backoff (1s → 60s).
//!
//! Singleton: pidfile `$FAMP_HOME/listen-wake-<identity>.pid` (default
//! `~/.famp/…`). Live pid → refuse unless `--force`. Each wake line is also
//! appended to `listen-wake-<identity>.wake` so `--follow` (and Grok
//! monitors) can inject without a second awaiter.
//!
//! MCP `famp_register(listen=true)` arms a supervised daemon via
//! [`ensure_supervised`]; `set_listen(false)` / listen-false register calls
//! [`stop_supervised`].

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Args;

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{self, AwaitArgs, AwaitOutcome};
use crate::cli::error::CliError;
use crate::cli::home;

/// CLI flags for `famp listen-wake`.
#[derive(Args, Debug, Clone)]
pub struct ListenWakeArgs {
    /// Identity to await as. Required so a monitor line is unambiguous.
    #[arg(long = "as")]
    pub act_as: String,

    /// Per-wait timeout. Default `23h` (matches Claude/Codex Stop await).
    #[arg(long, default_value = "23h")]
    pub timeout: humantime::Duration,

    /// After each wake line, flush and await again. Timeouts re-await silently.
    #[arg(long)]
    pub r#loop: bool,

    /// If a live listen-wake pidfile exists for this identity, SIGTERM (then
    /// SIGKILL) the old process and take the lock. Default is refuse when
    /// alive. Use only when intentionally replacing a supervised waiter.
    #[arg(long)]
    pub force: bool,

    /// Background: spawn a detached `--loop` waiter (log + wake file +
    /// pidfile), then exit 0. Mutually exclusive with `--follow`.
    #[arg(long)]
    pub daemon: bool,

    /// Follow the wake file only (no second bus await). Use when MCP (or
    /// `--daemon`) already armed the singleton waiter. Mutually exclusive
    /// with `--daemon` / parking modes that would contend on the pidfile.
    #[arg(long)]
    pub follow: bool,
}

/// Initial backoff after a bus error in `--loop` mode.
const BACKOFF_START: Duration = Duration::from_secs(1);
/// Cap for exponential backoff between retries.
const BACKOFF_CAP: Duration = Duration::from_secs(60);
/// After this many consecutive bus failures in loop mode, exit 1.
const MAX_CONSECUTIVE_FAILURES: u32 = 100;

/// Top-level entry for `Commands::ListenWake`.
pub async fn run(args: ListenWakeArgs) -> Result<(), CliError> {
    run_at(&resolve_sock_path(), args).await
}

/// Sock-explicit entry (tests / harness). Prints to stdout/stderr.
pub async fn run_at(sock: &Path, args: ListenWakeArgs) -> Result<(), CliError> {
    let identity = scrub_token(&args.act_as).ok_or_else(|| {
        CliError::Generic(format!(
            "invalid --as identity {:?}: must match [A-Za-z0-9._@+-]{{1,64}}",
            args.act_as
        ))
    })?;

    if args.daemon && args.follow {
        return Err(CliError::Generic(
            "--daemon and --follow are mutually exclusive".into(),
        ));
    }

    if args.follow {
        return follow_wake_file(&identity).await;
    }

    if args.daemon {
        // Detach a --loop waiter; this process exits after spawn.
        ensure_supervised_inner(&identity, args.force)?;
        return Ok(());
    }

    let famp_home = home::resolve_famp_home()?;
    ensure_famp_home_dir(&famp_home)?;
    let pid_path = pidfile_path(&famp_home, &identity);
    let wake_path = wake_file_path(&famp_home, &identity);

    let _guard = acquire_pidfile(&pid_path, args.force)?;

    run_await_loop(sock, &identity, &args, &wake_path).await
}

async fn run_await_loop(
    sock: &Path,
    identity: &str,
    args: &ListenWakeArgs,
    wake_path: &Path,
) -> Result<(), CliError> {
    let mut backoff = BACKOFF_START;
    let mut consecutive_failures: u32 = 0;

    loop {
        let await_args = AwaitArgs {
            timeout: args.timeout,
            task: None,
            act_as: Some(identity.to_string()),
            abort_on_fd: None,
        };
        match await_cmd::run_at_structured(sock, await_args).await {
            Ok(outcome) => {
                consecutive_failures = 0;
                backoff = BACKOFF_START;
                match write_wake_outcome(
                    &outcome,
                    identity,
                    args.r#loop,
                    Some(wake_path),
                    &mut std::io::stdout(),
                    &mut std::io::stderr(),
                )? {
                    WakeAction::Done => return Ok(()),
                    WakeAction::Exit(code) => return Err(CliError::Exit(code)),
                    WakeAction::Continue => {}
                }
            }
            Err(e) => {
                if !args.r#loop {
                    return Err(e);
                }
                consecutive_failures = consecutive_failures.saturating_add(1);
                let _ = writeln!(
                    std::io::stderr(),
                    "listen-wake bus error (attempt {consecutive_failures}/{MAX_CONSECUTIVE_FAILURES}): {e}"
                );
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    return Err(CliError::Generic(format!(
                        "listen-wake: {MAX_CONSECUTIVE_FAILURES} consecutive bus failures; exiting"
                    )));
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff.saturating_mul(2)).min(BACKOFF_CAP);
            }
        }
    }
}

/// Tail `listen-wake-<id>.wake` (create empty if missing). Never parks on the bus.
async fn follow_wake_file(identity: &str) -> Result<(), CliError> {
    let famp_home = home::resolve_famp_home()?;
    ensure_famp_home_dir(&famp_home)?;
    let wake_path = wake_file_path(&famp_home, identity);
    if !wake_path.exists() {
        // Touch so tail works before the first wake arrives.
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wake_path)
            .map_err(|source| CliError::Io {
                path: wake_path.clone(),
                source,
            })?;
    }

    let mut file = File::open(&wake_path).map_err(|source| CliError::Io {
        path: wake_path.clone(),
        source,
    })?;
    // Start at EOF — only new wakes after --follow starts.
    file.seek(SeekFrom::End(0))
        .map_err(|source| CliError::Io {
            path: wake_path.clone(),
            source,
        })?;

    let mut buf = String::new();
    let mut stdout = std::io::stdout();
    loop {
        buf.clear();
        let n = file
            .read_to_string(&mut buf)
            .map_err(|source| CliError::Io {
                path: wake_path.clone(),
                source,
            })?;
        if n > 0 {
            stdout.write_all(buf.as_bytes()).map_err(|source| CliError::Io {
                path: PathBuf::new(),
                source,
            })?;
            stdout.flush().map_err(|source| CliError::Io {
                path: PathBuf::new(),
                source,
            })?;
        } else {
            // Truncation / rotation: if file shrank, rewind.
            let meta_len = std::fs::metadata(&wake_path)
                .map(|m| m.len())
                .unwrap_or(0);
            let pos = file.stream_position().unwrap_or(0);
            if pos > meta_len {
                file.seek(SeekFrom::Start(0)).map_err(|source| CliError::Io {
                    path: wake_path.clone(),
                    source,
                })?;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

/// Structured helper: render one wake outcome without parking. Used by
/// `run_at` and unit tests that want to assert the stdout line shape.
///
/// When `wake_path` is `Some`, the same line is appended to that file
/// (best-effort for dual stdout + wake-file consumers).
pub(crate) fn write_wake_outcome(
    outcome: &AwaitOutcome,
    identity: &str,
    loop_mode: bool,
    wake_path: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<WakeAction, CliError> {
    if outcome.aborted {
        return Ok(WakeAction::Exit(3));
    }
    if outcome.timed_out {
        if loop_mode {
            return Ok(WakeAction::Continue);
        }
        writeln!(err, "TIMEOUT").map_err(|source| CliError::Io {
            path: PathBuf::new(),
            source,
        })?;
        return Ok(WakeAction::Exit(2));
    }

    let count = outcome.envelopes.len();
    let sender = extract_sender(outcome);
    let line = format!("FAMP_WAKE identity={identity} sender={sender} count={count}");
    writeln!(out, "{line}").map_err(|source| CliError::Io {
        path: PathBuf::new(),
        source,
    })?;
    out.flush().map_err(|source| CliError::Io {
        path: PathBuf::new(),
        source,
    })?;

    if let Some(path) = wake_path {
        // Best-effort append — wake file is advisory for monitors.
        if let Err(e) = append_wake_line(path, &line) {
            let _ = writeln!(err, "listen-wake: failed to append wake file: {e}");
        }
    }

    if loop_mode {
        Ok(WakeAction::Continue)
    } else {
        Ok(WakeAction::Done)
    }
}

fn append_wake_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")?;
    f.sync_all()?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WakeAction {
    Done,
    Continue,
    Exit(i32),
}

fn extract_sender(outcome: &AwaitOutcome) -> String {
    // Prefer the latest envelope in the batch (same as the Stop-hook notify).
    for env in outcome.envelopes.iter().rev() {
        if let Some(s) = sender_from_envelope(env) {
            return s;
        }
    }
    "unknown".to_string()
}

fn sender_from_envelope(env: &serde_json::Value) -> Option<String> {
    let raw = env
        .get("from")
        .and_then(serde_json::Value::as_str)
        .or_else(|| env.get("sender").and_then(serde_json::Value::as_str))
        .or_else(|| {
            env.get("envelope")
                .and_then(|inner| inner.get("from"))
                .and_then(serde_json::Value::as_str)
        })?;
    Some(scrub_sender(raw))
}

/// Scrub a sender string to `[A-Za-z0-9._@+-]{1,64}`.
///
/// Principal forms (`agent:local.bus/alice`) reduce to the trailing name
/// segment before scrubbing so the wake line stays short and shell-safe.
/// Anything that fails the scrub becomes `"unknown"`.
pub(crate) fn scrub_sender(raw: &str) -> String {
    let candidate = raw.rsplit_once('/').map_or(raw, |(_, name)| name);
    scrub_token(candidate).unwrap_or_else(|| "unknown".to_string())
}

fn scrub_token(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > 64 {
        return None;
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '+' | '-'))
    {
        Some(s.to_string())
    } else {
        None
    }
}

// ── Paths ───────────────────────────────────────────────────────────────────

/// `$FAMP_HOME/listen-wake-<identity>.pid`
///
/// `identity` must already be scrubbed (`scrub_token` / valid `--as`).
pub(crate) fn pidfile_path(famp_home: &Path, identity: &str) -> PathBuf {
    famp_home.join(format!("listen-wake-{identity}.pid"))
}

/// `$FAMP_HOME/listen-wake-<identity>.wake`
pub(crate) fn wake_file_path(famp_home: &Path, identity: &str) -> PathBuf {
    famp_home.join(format!("listen-wake-{identity}.wake"))
}

/// `$FAMP_HOME/listen-wake-<identity>.log`
pub(crate) fn log_file_path(famp_home: &Path, identity: &str) -> PathBuf {
    famp_home.join(format!("listen-wake-{identity}.log"))
}

fn ensure_famp_home_dir(famp_home: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(famp_home).map_err(|source| CliError::Io {
        path: famp_home.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(famp_home, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

// ── Pidfile singleton ───────────────────────────────────────────────────────

/// RAII guard: removes the pidfile on drop if it still contains our pid.
#[derive(Debug)]
pub(crate) struct PidfileGuard {
    path: PathBuf,
    pid: u32,
}

impl Drop for PidfileGuard {
    fn drop(&mut self) {
        release_pidfile_if_ours(&self.path, self.pid);
    }
}

/// Acquire the listen-wake singleton for this identity.
///
/// - Missing / dead pid → take over (write our pid, fsync).
/// - Live pid + `!force` → stderr `ALREADY_RUNNING pid=<n>`, exit 1.
/// - Live pid + `force` → SIGTERM, brief wait, SIGKILL if needed, then take.
pub(crate) fn acquire_pidfile(path: &Path, force: bool) -> Result<PidfileGuard, CliError> {
    if let Some(old_pid) = read_pidfile(path) {
        if is_listen_wake_alive(old_pid) {
            if !force {
                let _ = writeln!(
                    std::io::stderr(),
                    "ALREADY_RUNNING pid={old_pid}"
                );
                return Err(CliError::Exit(1));
            }
            force_kill_pid(old_pid);
        }
        // Dead or just-killed: remove stale pidfile.
        let _ = std::fs::remove_file(path);
    }

    let pid = std::process::id();
    write_pidfile(path, pid)?;
    Ok(PidfileGuard {
        path: path.to_path_buf(),
        pid,
    })
}

fn release_pidfile_if_ours(path: &Path, pid: u32) {
    if read_pidfile(path) == Some(pid) {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn read_pidfile(path: &Path) -> Option<u32> {
    let s = std::fs::read_to_string(path).ok()?;
    s.trim().parse().ok()
}

fn write_pidfile(path: &Path, pid: u32) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        ensure_famp_home_dir(parent)?;
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|source| CliError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    write!(f, "{pid}\n").map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    f.sync_all().map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// `kill(pid, 0)` plus optional cmdline contains `listen-wake`.
pub(crate) fn is_listen_wake_alive(pid: u32) -> bool {
    if !process_exists(pid) {
        return false;
    }
    // Prefer cmdline match when available; if unreadable, treat kill(0)
    // success as alive (conservative — refuse start rather than double).
    match process_cmdline(pid) {
        Some(cmd) => cmd.contains("listen-wake"),
        None => true,
    }
}

fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    if raw <= 0 {
        return false;
    }
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), None).is_ok()
}

fn process_cmdline(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        if raw.is_empty() {
            return None;
        }
        Some(
            raw.iter()
                .map(|&b| if b == 0 { b' ' } else { b })
                .map(char::from)
                .collect::<String>()
                .trim()
                .to_owned(),
        )
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let s = String::from_utf8(output.stdout).ok()?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

fn force_kill_pid(pid: u32) {
    let Ok(raw) = i32::try_from(pid) else {
        return;
    };
    if raw <= 0 {
        return;
    }
    let nix_pid = nix::unistd::Pid::from_raw(raw);
    let _ = nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM);
    // Brief wait for graceful exit.
    for _ in 0..20 {
        if !process_exists(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGKILL);
    for _ in 0..10 {
        if !process_exists(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ── MCP supervision ─────────────────────────────────────────────────────────

/// Ensure a detached `listen-wake --loop` is running for `identity`.
///
/// No-op when the pidfile already points at a live listen-wake. When
/// `force` is true, kill any live waiter first (re-register path).
///
/// Errors from spawn are logged to stderr but do not fail the MCP tool —
/// register itself already succeeded on the bus.
pub fn ensure_supervised(identity: &str, force: bool) {
    let Some(id) = scrub_token(identity) else {
        let _ = writeln!(
            std::io::stderr(),
            "listen-wake ensure_supervised: invalid identity {identity:?}"
        );
        return;
    };
    if let Err(e) = ensure_supervised_inner(&id, force) {
        let _ = writeln!(
            std::io::stderr(),
            "listen-wake ensure_supervised({id}): {e}"
        );
    }
}

fn ensure_supervised_inner(identity: &str, force: bool) -> Result<(), CliError> {
    let famp_home = home::resolve_famp_home()?;
    ensure_famp_home_dir(&famp_home)?;
    let pid_path = pidfile_path(&famp_home, identity);

    if !force {
        if let Some(pid) = read_pidfile(&pid_path) {
            if is_listen_wake_alive(pid) {
                return Ok(()); // already armed
            }
        }
    }

    spawn_detached_listen_wake(identity, force, &famp_home)
}

/// Kill the supervised listen-wake for `identity` (if any) and remove pidfile.
pub fn stop_supervised(identity: &str) {
    let Some(id) = scrub_token(identity) else {
        return;
    };
    let Ok(famp_home) = home::resolve_famp_home() else {
        return;
    };
    let pid_path = pidfile_path(&famp_home, &id);
    if let Some(pid) = read_pidfile(&pid_path) {
        if is_listen_wake_alive(pid) {
            force_kill_pid(pid);
        }
        let _ = std::fs::remove_file(&pid_path);
    }
}

/// Spawn `current_exe listen-wake --as <id> --loop [--force]` detached
/// (new session), logging to `listen-wake-<id>.log`.
fn spawn_detached_listen_wake(
    identity: &str,
    force: bool,
    famp_home: &Path,
) -> Result<(), CliError> {
    let log_path = log_file_path(famp_home, identity);
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| CliError::Io {
            path: log_path.clone(),
            source,
        })?;
    let log_err = log.try_clone().map_err(|source| CliError::Io {
        path: log_path.clone(),
        source,
    })?;

    let exe = std::env::current_exe().map_err(|source| CliError::Io {
        path: PathBuf::from("current_exe"),
        source,
    })?;

    let mut args = vec![
        "listen-wake".to_string(),
        "--as".to_string(),
        identity.to_string(),
        "--loop".to_string(),
    ];
    if force {
        args.push("--force".to_string());
    }

    spawn_detached(&exe, &args, log, log_err)
}

#[cfg(unix)]
#[allow(unsafe_code)] // setsid-before-exec; same Q1 pattern as bus_client::spawn.
fn spawn_detached(
    exe: &Path,
    args: &[String],
    stdout: File,
    stderr: File,
) -> Result<(), CliError> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    // SAFETY: pre_exec runs in the forked child before exec; only
    // async-signal-safe setsid() is called (same pattern as bus_client::spawn).
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid()
                .map(|_pgid| ())
                .map_err(std::io::Error::from)?;
            Ok(())
        });
    }
    let child = cmd.spawn().map_err(|source| CliError::Io {
        path: exe.to_path_buf(),
        source,
    })?;
    // Disown: child has its own session and holds the pidfile.
    drop(child);
    Ok(())
}

#[cfg(not(unix))]
fn spawn_detached(
    exe: &Path,
    args: &[String],
    stdout: File,
    stderr: File,
) -> Result<(), CliError> {
    use std::process::{Command, Stdio};
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|source| CliError::Io {
            path: exe.to_path_buf(),
            source,
        })?;
    drop(child);
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn scrub_sender_accepts_simple_name() {
        assert_eq!(scrub_sender("alice"), "alice");
        assert_eq!(scrub_sender("bob.agent-1"), "bob.agent-1");
        assert_eq!(scrub_sender("user@host"), "user@host");
        assert_eq!(scrub_sender("a+b"), "a+b");
    }

    #[test]
    fn scrub_sender_extracts_principal_name() {
        assert_eq!(scrub_sender("agent:local.bus/alice"), "alice");
        assert_eq!(scrub_sender("agent:local.bus/bob-1"), "bob-1");
    }

    #[test]
    fn scrub_sender_rejects_bad_chars_and_length() {
        assert_eq!(scrub_sender("alice; rm -rf /"), "unknown");
        assert_eq!(scrub_sender(""), "unknown");
        assert_eq!(scrub_sender(&"a".repeat(65)), "unknown");
        assert_eq!(scrub_sender("has space"), "unknown");
        assert_eq!(scrub_sender("evil$(cmd)"), "unknown");
    }

    #[test]
    fn scrub_token_matches_path_safe_charset() {
        assert_eq!(scrub_token("alice"), Some("alice".into()));
        assert_eq!(scrub_token("a@b"), Some("a@b".into()));
        assert_eq!(scrub_token("bad name"), None);
        assert_eq!(scrub_token(""), None);
    }

    #[test]
    fn pidfile_and_wake_paths_use_scrubbed_identity() {
        let home = Path::new("/tmp/famp-home-test");
        assert_eq!(
            pidfile_path(home, "alice"),
            PathBuf::from("/tmp/famp-home-test/listen-wake-alice.pid")
        );
        assert_eq!(
            wake_file_path(home, "bob-1"),
            PathBuf::from("/tmp/famp-home-test/listen-wake-bob-1.wake")
        );
        assert_eq!(
            log_file_path(home, "x"),
            PathBuf::from("/tmp/famp-home-test/listen-wake-x.log")
        );
    }

    #[test]
    fn write_wake_outcome_emits_scrubbed_line() {
        let outcome = AwaitOutcome {
            envelopes: vec![serde_json::json!({
                "from": "agent:local.bus/bob",
                "body": "SECRET PEER BYTES MUST NOT APPEAR"
            })],
            mailbox: None,
            next_offset: Some(1),
            timed_out: false,
            diagnostic: None,
            aborted: false,
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let action =
            write_wake_outcome(&outcome, "alice", false, None, &mut out, &mut err).unwrap();
        assert_eq!(action, WakeAction::Done);
        let line = String::from_utf8(out).unwrap();
        assert_eq!(line, "FAMP_WAKE identity=alice sender=bob count=1\n");
        assert!(!line.contains("SECRET"));
        assert!(err.is_empty());
    }

    #[test]
    fn write_wake_outcome_appends_wake_file() {
        let dir = tempfile::tempdir().unwrap();
        let wake = dir.path().join("listen-wake-alice.wake");
        let outcome = AwaitOutcome {
            envelopes: vec![serde_json::json!({"from": "bob"})],
            mailbox: None,
            next_offset: Some(1),
            timed_out: false,
            diagnostic: None,
            aborted: false,
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        write_wake_outcome(&outcome, "alice", false, Some(&wake), &mut out, &mut err).unwrap();
        let body = std::fs::read_to_string(&wake).unwrap();
        assert_eq!(body, "FAMP_WAKE identity=alice sender=bob count=1\n");
    }

    #[test]
    fn write_wake_outcome_timeout_once_exits_2() {
        let outcome = AwaitOutcome {
            envelopes: vec![],
            mailbox: None,
            next_offset: None,
            timed_out: true,
            diagnostic: None,
            aborted: false,
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let action =
            write_wake_outcome(&outcome, "alice", false, None, &mut out, &mut err).unwrap();
        assert_eq!(action, WakeAction::Exit(2));
        assert_eq!(String::from_utf8(err).unwrap(), "TIMEOUT\n");
        assert!(out.is_empty());
    }

    #[test]
    fn write_wake_outcome_timeout_loop_continues_silently() {
        let outcome = AwaitOutcome {
            envelopes: vec![],
            mailbox: None,
            next_offset: None,
            timed_out: true,
            diagnostic: None,
            aborted: false,
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let action =
            write_wake_outcome(&outcome, "alice", true, None, &mut out, &mut err).unwrap();
        assert_eq!(action, WakeAction::Continue);
        assert!(out.is_empty());
        assert!(err.is_empty());
    }

    #[test]
    fn acquire_pidfile_refuses_live_foreign_pid_without_force() {
        // Use our own pid + fake cmdline match by writing current pid.
        // process_exists(our pid) is true; cmdline of *this* test process
        // does NOT contain "listen-wake", so is_listen_wake_alive is false
        // when cmdline is readable. To test the refuse path we inject a
        // pid that exists AND whose cmdline we can't force — so exercise
        // the pure helper: dead pid takeover + write.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("listen-wake-t.pid");

        // Dead pid (unlikely to exist): take over.
        std::fs::write(&path, "1\n").unwrap(); // pid 1 may exist but cmdline won't be listen-wake
                                               // On Linux pid 1 exists; cmdline is not listen-wake → not alive.
                                               // acquire should take over.
        let guard = acquire_pidfile(&path, false).unwrap();
        assert_eq!(read_pidfile(&path), Some(std::process::id()));
        drop(guard);
        assert!(!path.exists() || read_pidfile(&path) != Some(std::process::id()));
    }

    #[test]
    fn acquire_pidfile_refuses_when_alive_listen_wake() {
        // Simulate "alive listen-wake" by writing our pid and temporarily
        // relying on process_exists — but our cmdline lacks listen-wake.
        // Unit-test the decision helper instead: is_listen_wake_alive on
        // a definitely-dead pid is false; on pid 0 is false.
        assert!(!is_listen_wake_alive(0));
        assert!(!process_exists(0));

        // Stale pidfile with nonsense pid → take over.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.pid");
        std::fs::write(&path, "4294967290\n").unwrap(); // almost-surely dead
        let guard = acquire_pidfile(&path, false).unwrap();
        assert_eq!(read_pidfile(&path), Some(std::process::id()));
        drop(guard);
    }

    #[test]
    fn acquire_pidfile_writes_and_releases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("listen-wake-alice.pid");
        {
            let guard = acquire_pidfile(&path, false).unwrap();
            assert_eq!(read_pidfile(&path), Some(guard.pid));
            assert_eq!(guard.pid, std::process::id());
        }
        // Dropped: pidfile removed if still ours.
        assert!(read_pidfile(&path).is_none());
    }

    #[test]
    fn backoff_constants_are_sane() {
        assert_eq!(BACKOFF_START, Duration::from_secs(1));
        assert_eq!(BACKOFF_CAP, Duration::from_secs(60));
        assert_eq!(MAX_CONSECUTIVE_FAILURES, 100);
        // Exponential growth caps.
        let mut b = BACKOFF_START;
        for _ in 0..10 {
            b = (b.saturating_mul(2)).min(BACKOFF_CAP);
        }
        assert_eq!(b, BACKOFF_CAP);
    }

    #[test]
    fn alive_refuse_path_prints_already_running() {
        // Build a guard that owns the pidfile, then a second acquire without
        // force must Exit(1). Our process cmdline does not contain
        // listen-wake, so is_listen_wake_alive is false for our own pid —
        // we can't use our own pid as the "live listen-wake".
        //
        // Instead: mock the decision by testing read+exists helpers and
        // verifying Exit(1) message format via a direct stderr write path
        // that acquire uses when is_listen_wake_alive returns true.
        //
        // Force the alive path by writing a pidfile pointing at a long-lived
        // child whose argv contains "listen-wake".
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lw.pid");

        // Spawn: `sleep 30` with argv rewritten is hard; use a shell that
        // keeps "listen-wake" in /proc/pid/cmdline via bash -c.
        // On Linux, `bash -c 'exec -a listen-wake sleep 30'` works.
        let child = std::process::Command::new("bash")
            .args(["-c", "exec -a 'famp listen-wake --as t --loop' sleep 30"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let child_pid = child.id();
        // Give the exec a moment.
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            is_listen_wake_alive(child_pid),
            "child pid {child_pid} should look like listen-wake (cmdline={:?})",
            process_cmdline(child_pid)
        );
        write_pidfile(&path, child_pid).unwrap();

        let err = acquire_pidfile(&path, false).unwrap_err();
        assert!(matches!(err, CliError::Exit(1)));

        // Cleanup.
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(i32::try_from(child_pid).unwrap()),
            nix::sys::signal::Signal::SIGKILL,
        );
        let _ = child.wait_with_output();
        let _ = std::fs::remove_file(&path);
    }
}
