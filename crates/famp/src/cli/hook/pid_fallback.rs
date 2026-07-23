//! PID-correlated identity fallback (compaction resilience).
//!
//! When the transcript scan finds no listen identity, recover by correlating
//! this hook's ancestor chain with a live `famp mcp` process, mapping that
//! pid to a registered name via `famp sessions`, and confirming `listen_mode`.
//!
//! Never adopts an identity merely because it shares a cwd (anti-hijack).
//! Disabled when `FAMP_DISABLE_PID_FALLBACK=1`.

use std::collections::HashSet;
use std::path::Path;

use famp_bus::BusMessage;
use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectIdentitiesReply, InspectIdentitiesRequest, InspectKind};

use crate::bus_client::{resolve_sock_path, BusClient};

use super::log::log;

/// Try to resolve a listen-mode identity via process ancestry.
pub async fn resolve_via_pid(sock: Option<&Path>) -> Option<String> {
    if std::env::var("FAMP_DISABLE_PID_FALLBACK").ok().as_deref() == Some("1") {
        log("pid-correlated fallback disabled (FAMP_DISABLE_PID_FALLBACK=1)");
        return None;
    }

    let ancestors = ancestor_pids(6);
    if ancestors.is_empty() {
        return None;
    }
    let sibling_mcp = sibling_mcp_pids(&ancestors);
    if sibling_mcp.is_empty() {
        return None;
    }

    let sock_path = sock.map_or_else(resolve_sock_path, Path::to_path_buf);
    let candidate = unique_session_name_for_pids(&sock_path, &sibling_mcp).await?;
    if identity_is_listen(&sock_path, &candidate).await {
        log(&format!(
            "transcript had no register; pid-correlated fallback resolved identity={candidate} (sibling mcp pids:{sibling_mcp:?})"
        ));
        Some(candidate)
    } else {
        log(&format!(
            "pid-correlated candidate '{candidate}' is not listen=true; no-op"
        ));
        None
    }
}

fn ancestor_pids(max_depth: usize) -> Vec<u32> {
    let mut out = Vec::new();
    let mut current = std::process::id();
    for _ in 0..max_depth {
        let Some(ppid) = parent_pid(current) else {
            break;
        };
        if ppid == 0 || ppid == 1 {
            break;
        }
        out.push(ppid);
        current = ppid;
    }
    out
}

#[cfg(target_os = "linux")]
fn parent_pid(pid: u32) -> Option<u32> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn parent_pid(pid: u32) -> Option<u32> {
    // Prefer absolute ps paths so a minimal PATH still works.
    for ps in ["/bin/ps", "/usr/bin/ps"] {
        let out = std::process::Command::new(ps)
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !out.status.success() {
            continue;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        if let Ok(ppid) = s.trim().parse::<u32>() {
            return Some(ppid);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn sibling_mcp_pids(ancestors: &[u32]) -> Vec<u32> {
    let anc: HashSet<u32> = ancestors.iter().copied().collect();
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid_str) = name.to_str() else {
            continue;
        };
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
            continue;
        };
        let Some(ppid) = stat.lines().find_map(|l| {
            l.strip_prefix("PPid:")
                .and_then(|r| r.trim().parse::<u32>().ok())
        }) else {
            continue;
        };
        if !anc.contains(&ppid) {
            continue;
        }
        // cmdline is NUL-separated.
        let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let joined = cmdline
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        if joined.contains("famp mcp") {
            found.push(pid);
        }
    }
    found
}

#[cfg(not(target_os = "linux"))]
fn sibling_mcp_pids(ancestors: &[u32]) -> Vec<u32> {
    let anc: HashSet<u32> = ancestors.iter().copied().collect();
    let mut found = Vec::new();
    for ps in ["/bin/ps", "/usr/bin/ps"] {
        let Ok(out) = std::process::Command::new(ps)
            .args(["-eo", "pid=,ppid=,args="])
            .output()
        else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let Some(pid_s) = parts.next() else { continue };
            let Some(ppid_s) = parts.next() else { continue };
            let Ok(pid) = pid_s.parse::<u32>() else {
                continue;
            };
            let Ok(ppid) = ppid_s.parse::<u32>() else {
                continue;
            };
            if !anc.contains(&ppid) {
                continue;
            }
            let rest = parts.collect::<Vec<_>>().join(" ");
            if rest.contains("famp mcp") {
                found.push(pid);
            }
        }
        if !found.is_empty() {
            break;
        }
    }
    found
}

async fn unique_session_name_for_pids(sock: &Path, pids: &[u32]) -> Option<String> {
    let mut bus = BusClient::connect(sock, None).await.ok()?;
    let reply = bus.send_recv(BusMessage::Sessions {}).await.ok()?;
    let famp_bus::BusReply::SessionsOk { rows } = reply else {
        return None;
    };
    let pid_set: HashSet<u32> = pids.iter().copied().collect();
    let mut names: HashSet<String> = HashSet::new();
    for row in rows {
        if pid_set.contains(&row.pid) {
            names.insert(row.name);
        }
    }
    if names.len() == 1 {
        names.into_iter().next()
    } else {
        None
    }
}

async fn identity_is_listen(sock: &Path, name: &str) -> bool {
    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(sock).await else {
        return false;
    };
    let Ok(payload) = call(
        &mut stream,
        InspectKind::Identities(InspectIdentitiesRequest::default()),
    )
    .await
    else {
        return false;
    };
    let Ok(reply) = serde_json::from_value::<InspectIdentitiesReply>(payload) else {
        return false;
    };
    match reply {
        InspectIdentitiesReply::List(list) => {
            list.rows.iter().any(|r| r.name == name && r.listen_mode)
        }
        InspectIdentitiesReply::BudgetExceeded { .. } => false,
    }
}
