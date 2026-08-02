//! `famp inspect wake` -- end-to-end Codex automatic-wake readiness.
//!
//! Broker `listen_mode` is only intent. This command deliberately keeps it
//! separate from the host adapter, MCP binding, hook load state, and current
//! await waiter so a CLI `register --tail` process cannot look Codex-ready.

use std::path::{Path, PathBuf};

use clap::Args;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{
    IdentityRow, InspectIdentitiesReply, InspectIdentitiesRequest, InspectKind,
    InspectWaitersReply, InspectWaitersRequest,
};
use serde::Serialize;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::install::codex::{
    codex_command_hook_hash, codex_hook_key, find_git_root, CODEX_AWAIT_TIMEOUT_SEC,
    CODEX_STOP_EVENT_LABEL,
};
use crate::cli::sessions::{self, SessionsArgs};

#[derive(Args, Debug)]
pub struct InspectWakeArgs {
    /// Registered identity whose Codex wake path should be diagnosed.
    #[arg(long)]
    pub identity: String,

    /// Override the project whose `.codex/hooks.json` is inspected.
    /// Defaults to the identity's registered CWD (or its git root).
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Emit JSON output.
    #[arg(long)]
    pub json: bool,

    /// Override the home containing `.codex/config.toml` (tests only).
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TriState {
    True,
    False,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HolderKind {
    Mcp,
    CliRegister,
    Other,
    Unknown,
}

#[derive(Debug, Serialize)]
struct HookState {
    path: String,
    present: bool,
    trusted: TriState,
}

#[derive(Debug, Serialize)]
struct WakeReport {
    identity: String,
    broker: &'static str,
    delivery: &'static str,
    holder_kind: HolderKind,
    holder_pid: Option<u32>,
    broker_listen: bool,
    host_adapter: &'static str,
    hook: HookState,
    configured: bool,
    armed: TriState,
    loaded: TriState,
    parked: bool,
    wake_ready: TriState,
    reason: String,
    remediation: Option<String>,
}

pub async fn run(args: InspectWakeArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();
    let ProbeOutcome::Healthy { .. } = raw_connect_probe(&sock).await else {
        eprintln!("error: broker not running at {sock_str}");
        return Err(CliError::Exit(1));
    };

    let identity = fetch_identity(&sock, &args.identity).await?;
    let parked = fetch_parked(&sock, &args.identity).await?;
    let sessions = sessions::run_at_structured(
        &sock,
        &SessionsArgs {
            me: false,
            act_as: None,
        },
    )
    .await?;
    let holder_pid = sessions
        .rows
        .iter()
        .find(|row| row.name == args.identity)
        .map(|row| row.pid);
    let holder_kind = holder_pid.map_or(HolderKind::Unknown, classify_holder);

    let project = resolve_project(args.project.as_deref(), identity.cwd.as_deref())?;
    let home = match args.home {
        Some(path) => path,
        None => dirs::home_dir().ok_or_else(|| {
            CliError::Generic("could not resolve home directory for Codex hook trust".into())
        })?,
    };
    let hook = inspect_hook(&project, &home)?;
    let configured = hook.present && hook.trusted == TriState::True;
    let armed = match holder_kind {
        HolderKind::CliRegister => TriState::False,
        // An `famp mcp` holder proves the registration surface, but not that
        // this Codex window's transcript/PID correlation can resolve it.
        HolderKind::Mcp | HolderKind::Other | HolderKind::Unknown => TriState::Unknown,
    };
    let loaded = if hook.present {
        // A waiter has no provenance in the current inspector protocol: it
        // may be the Stop hook, manual CLI await, or direct MCP await.
        TriState::Unknown
    } else {
        TriState::False
    };
    let (wake_ready, reason, remediation) =
        readiness(identity.listen_mode, holder_kind, &hook, parked);

    let report = WakeReport {
        identity: args.identity,
        broker: "healthy",
        delivery: "healthy",
        holder_kind,
        holder_pid,
        broker_listen: identity.listen_mode,
        host_adapter: "codex",
        hook,
        configured,
        armed,
        loaded,
        parked,
        wake_ready,
        reason,
        remediation,
    };
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| CliError::Generic(format!("json serialize: {e}")))?
        );
    } else {
        print_report(&report);
    }
    Ok(())
}

async fn fetch_identity(sock: &Path, name: &str) -> Result<IdentityRow, CliError> {
    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(sock).await else {
        return Err(CliError::BrokerUnreachable);
    };
    let payload = call(
        &mut stream,
        InspectKind::Identities(InspectIdentitiesRequest::default()),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect identities call failed: {e}")))?;
    let reply: InspectIdentitiesReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("identities reply schema mismatch: {e}")))?;
    match reply {
        InspectIdentitiesReply::List(list) => list
            .rows
            .into_iter()
            .find(|row| row.name == name)
            .ok_or_else(|| CliError::Generic(format!("identity `{name}` is not registered"))),
        InspectIdentitiesReply::BudgetExceeded { elapsed_ms } => Err(CliError::Generic(format!(
            "inspect budget exceeded ({elapsed_ms}ms) — broker busy, retry"
        ))),
    }
}

async fn fetch_parked(sock: &Path, name: &str) -> Result<bool, CliError> {
    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(sock).await else {
        return Err(CliError::BrokerUnreachable);
    };
    let payload = call(
        &mut stream,
        InspectKind::Waiters(InspectWaitersRequest::default()),
    )
    .await
    .map_err(|e| CliError::Generic(format!("inspect waiters call failed: {e}")))?;
    let reply: InspectWaitersReply = serde_json::from_value(payload)
        .map_err(|e| CliError::Generic(format!("waiters reply schema mismatch: {e}")))?;
    match reply {
        InspectWaitersReply::List(list) => Ok(list.rows.iter().any(|row| row.name == name)),
        InspectWaitersReply::BudgetExceeded { elapsed_ms } => Err(CliError::Generic(format!(
            "inspect budget exceeded ({elapsed_ms}ms) — broker busy, retry"
        ))),
    }
}

fn resolve_project(
    explicit: Option<&Path>,
    registered_cwd: Option<&str>,
) -> Result<PathBuf, CliError> {
    let candidate = if let Some(path) = explicit {
        path.to_path_buf()
    } else if let Some(cwd) = registered_cwd {
        let cwd = PathBuf::from(cwd);
        find_git_root(&cwd).unwrap_or(cwd)
    } else {
        std::env::current_dir().map_err(|source| CliError::Io {
            path: PathBuf::from("."),
            source,
        })?
    };
    Ok(candidate.canonicalize().unwrap_or(candidate))
}

fn inspect_hook(project: &Path, home: &Path) -> Result<HookState, CliError> {
    let hooks_path = project.join(".codex").join("hooks.json");
    let config_path = home.join(".codex").join("config.toml");
    let body = match std::fs::read_to_string(&hooks_path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HookState {
                path: hooks_path.display().to_string(),
                present: false,
                trusted: TriState::False,
            });
        }
        Err(source) => {
            return Err(CliError::Io {
                path: hooks_path,
                source,
            });
        }
    };
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|source| CliError::JsonMergeParse {
            path: hooks_path.clone(),
            source,
        })?;
    let Some((group_index, handler_index, command, timeout)) = find_native_hook(&json) else {
        return Ok(HookState {
            path: hooks_path.display().to_string(),
            present: false,
            trusted: TriState::False,
        });
    };
    let trust = inspect_trust(
        &config_path,
        &hooks_path,
        group_index,
        handler_index,
        command,
        timeout,
    )?;
    Ok(HookState {
        path: hooks_path.display().to_string(),
        present: true,
        trusted: trust,
    })
}

fn find_native_hook(json: &serde_json::Value) -> Option<(usize, usize, &str, i64)> {
    let groups = json.get("hooks")?.get("Stop")?.as_array()?;
    for (group_index, group) in groups.iter().enumerate() {
        let Some(handlers) = group.get("hooks").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for (handler_index, handler) in handlers.iter().enumerate() {
            let Some(command) = handler.get("command").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let is_command = handler
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind == "command");
            if is_command && command.trim_end().ends_with(" hook codex-stop") {
                let timeout = handler
                    .get("timeout")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(CODEX_AWAIT_TIMEOUT_SEC);
                return Some((group_index, handler_index, command, timeout));
            }
        }
    }
    None
}

fn inspect_trust(
    config_path: &Path,
    hooks_path: &Path,
    group_index: usize,
    handler_index: usize,
    command: &str,
    timeout: i64,
) -> Result<TriState, CliError> {
    let body = match std::fs::read_to_string(config_path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(TriState::False),
        Err(source) => {
            return Err(CliError::Io {
                path: config_path.to_path_buf(),
                source,
            });
        }
    };
    let config: toml::Table = toml::from_str(&body).map_err(|source| CliError::TomlParse {
        path: config_path.to_path_buf(),
        source,
    })?;
    let key = codex_hook_key(
        hooks_path,
        CODEX_STOP_EVENT_LABEL,
        group_index,
        handler_index,
    );
    let Some(state) = config
        .get("hooks")
        .and_then(|v| v.get("state"))
        .and_then(|v| v.get(&key))
    else {
        return Ok(TriState::False);
    };
    let enabled = state
        .get("enabled")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let expected_hash = codex_command_hook_hash(CODEX_STOP_EVENT_LABEL, command, timeout);
    let hash_matches = state
        .get("trusted_hash")
        .and_then(toml::Value::as_str)
        .is_some_and(|hash| hash == expected_hash);
    Ok(if enabled && hash_matches {
        TriState::True
    } else {
        TriState::False
    })
}

fn classify_holder(pid: u32) -> HolderKind {
    let Some(cmdline) = process_cmdline(pid) else {
        return HolderKind::Unknown;
    };
    classify_cmdline(&cmdline)
}

fn classify_cmdline(cmdline: &str) -> HolderKind {
    let words: Vec<&str> = cmdline.split_whitespace().collect();
    for pair in words.windows(2) {
        let executable_word = pair[0].trim_matches(['\'', '"']);
        let executable = Path::new(executable_word)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or(executable_word)
            .trim_matches(['\'', '"']);
        if executable == "famp" || executable.starts_with("famp-") {
            return match pair[1] {
                "mcp" => HolderKind::Mcp,
                "register" => HolderKind::CliRegister,
                _ => HolderKind::Other,
            };
        }
    }
    HolderKind::Other
}

#[cfg(target_os = "linux")]
fn process_cmdline(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    Some(
        String::from_utf8_lossy(&raw)
            .replace('\0', " ")
            .trim()
            .to_string(),
    )
}

#[cfg(not(target_os = "linux"))]
fn process_cmdline(pid: u32) -> Option<String> {
    for ps in ["/bin/ps", "/usr/bin/ps"] {
        let Ok(output) = std::process::Command::new(ps)
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
        else {
            continue;
        };
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }
    None
}

fn readiness(
    listen: bool,
    holder: HolderKind,
    hook: &HookState,
    parked: bool,
) -> (TriState, String, Option<String>) {
    // Deliberate current design: this function cannot return `True` until the
    // inspector protocol can attribute a waiter to `famp hook codex-stop` and
    // correlate that hook with the target Codex session. File configuration,
    // an MCP holder, and an unprovenanced waiter are insufficient evidence.
    if !listen {
        return (
            TriState::False,
            "broker listen intent is disabled".into(),
            Some("call MCP famp_set_listen({ listen: true })".into()),
        );
    }
    if holder == HolderKind::CliRegister {
        return (
            TriState::False,
            "standalone `famp register --tail` can display events but cannot bind a Codex MCP session".into(),
            Some("stop the CLI holder, restart Codex if needed, then call MCP famp_register".into()),
        );
    }
    if !hook.present {
        return (
            TriState::False,
            "the project has no native FAMP Codex Stop hook".into(),
            Some("run `famp install-codex --project <path>`, then restart Codex".into()),
        );
    }
    if hook.trusted != TriState::True {
        return (
            TriState::False,
            "the Codex Stop hook is present but its enabled trust entry is missing or stale".into(),
            Some("re-run `famp install-codex --project <path>`, then restart Codex".into()),
        );
    }
    if holder != HolderKind::Mcp {
        return (
            TriState::Unknown,
            "the hook is configured, but the holder could not be confirmed as this Codex MCP session".into(),
            Some("call MCP famp_register in the target Codex window".into()),
        );
    }
    if parked {
        return (
            TriState::Unknown,
            "configuration, an MCP holder, and an active waiter are present, but waiter provenance and Codex hook load cannot be attributed".into(),
            Some("confirm the waiter was created by a freshly restarted Codex Stop hook; manual CLI/MCP awaits do not prove host wake readiness".into()),
        );
    }
    (
        TriState::Unknown,
        "configuration and MCP binding look correct, but no waiter is parked; the turn may be active or this window may need a restart".into(),
        Some("end the turn and re-check; restart Codex if the hook was installed after this window opened".into()),
    )
}

fn print_report(report: &WakeReport) {
    println!("identity: {}", report.identity);
    println!("broker: {}", report.broker);
    println!("delivery: {}", report.delivery);
    println!(
        "holder: {} (pid {})",
        enum_label(report.holder_kind),
        report
            .holder_pid
            .map_or_else(|| "unknown".into(), |pid| pid.to_string())
    );
    println!("broker_listen: {}", report.broker_listen);
    println!("host_adapter: {}", report.host_adapter);
    println!(
        "stop_hook: {} ({})",
        if report.hook.present {
            "present"
        } else {
            "missing"
        },
        report.hook.path
    );
    println!("hook_trusted: {}", enum_label(report.hook.trusted));
    println!("configured: {}", report.configured);
    println!("armed: {}", enum_label(report.armed));
    println!("loaded: {}", enum_label(report.loaded));
    println!("parked: {}", report.parked);
    println!("wake_ready: {}", enum_label(report.wake_ready));
    println!("reason: {}", report.reason);
    if let Some(remediation) = &report.remediation {
        println!("remediation: {remediation}");
    }
}

fn enum_label<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn classifies_mcp_and_cli_holders() {
        assert_eq!(classify_cmdline("/opt/bin/famp mcp"), HolderKind::Mcp);
        assert_eq!(
            classify_cmdline("/opt/bin/famp register alice --tail"),
            HolderKind::CliRegister
        );
    }

    #[test]
    fn cli_tail_is_never_codex_ready() {
        let hook = HookState {
            path: "hooks.json".into(),
            present: true,
            trusted: TriState::True,
        };
        let (ready, reason, _) = readiness(true, HolderKind::CliRegister, &hook, false);
        assert_eq!(ready, TriState::False);
        assert!(reason.contains("cannot bind"));
    }

    #[test]
    fn configured_mcp_without_waiter_is_unknown_not_false() {
        let hook = HookState {
            path: "hooks.json".into(),
            present: true,
            trusted: TriState::True,
        };
        let (ready, reason, _) = readiness(true, HolderKind::Mcp, &hook, false);
        assert_eq!(ready, TriState::Unknown);
        assert!(reason.contains("turn may be active"));
    }

    #[test]
    fn generic_waiter_cannot_prove_codex_hook_loaded() {
        let hook = HookState {
            path: "hooks.json".into(),
            present: true,
            trusted: TriState::True,
        };
        let (ready, reason, _) = readiness(true, HolderKind::Mcp, &hook, true);
        assert_eq!(ready, TriState::Unknown);
        assert!(reason.contains("waiter provenance"));
    }

    #[test]
    fn listen_false_is_explicitly_disabled() {
        let hook = HookState {
            path: "hooks.json".into(),
            present: true,
            trusted: TriState::True,
        };
        let (ready, reason, _) = readiness(false, HolderKind::Mcp, &hook, true);
        assert_eq!(ready, TriState::False);
        assert!(reason.contains("disabled"));
    }

    #[test]
    fn non_command_handler_is_not_a_native_hook() {
        let hook = serde_json::json!({
            "hooks": { "Stop": [{
                "hooks": [{
                    "type": "prompt",
                    "command": "/opt/famp hook codex-stop",
                    "timeout": CODEX_AWAIT_TIMEOUT_SEC
                }]
            }]}
        });
        assert!(find_native_hook(&hook).is_none());
    }

    #[test]
    fn native_hook_and_matching_trust_are_configured() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let home = temp.path().join("home");
        std::fs::create_dir_all(project.join(".codex")).unwrap();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let project = project.canonicalize().unwrap();
        let home = home.canonicalize().unwrap();
        let hooks_path = project.join(".codex/hooks.json");
        let command = "/opt/famp hook codex-stop";
        std::fs::write(
            &hooks_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "hooks": { "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": command,
                        "timeout": CODEX_AWAIT_TIMEOUT_SEC
                    }]
                }]}
            }))
            .unwrap(),
        )
        .unwrap();

        let key = codex_hook_key(&hooks_path, CODEX_STOP_EVENT_LABEL, 0, 0);
        let hash =
            codex_command_hook_hash(CODEX_STOP_EVENT_LABEL, command, CODEX_AWAIT_TIMEOUT_SEC);
        let mut trust = toml::Table::new();
        trust.insert("enabled".into(), toml::Value::Boolean(true));
        trust.insert("trusted_hash".into(), toml::Value::String(hash));
        // Keep the fixture production-shaped: Codex config contains the MCP
        // table alongside the project hook-trust table.
        std::fs::write(
            home.join(".codex/config.toml"),
            "[mcp_servers.famp]\ncommand = \"/opt/famp\"\n",
        )
        .unwrap();
        crate::cli::install::toml_merge::upsert_nested_table(
            &home.join(".codex/config.toml"),
            &["hooks", "state"],
            &key,
            trust,
        )
        .unwrap();

        let hook = inspect_hook(&project, &home).unwrap();
        assert!(hook.present);
        assert_eq!(hook.trusted, TriState::True);
    }
}
