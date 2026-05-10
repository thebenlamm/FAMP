//! `famp inspect broker` -- broker liveness + dead-broker diagnosis.
//!
//! INSP-BROKER-01: HEALTHY single-line render; exit 0.
//! INSP-BROKER-02: DOWN_CLEAN / STALE_SOCKET / ORPHAN_HOLDER /
//! PERMISSION_DENIED down-states; exit 1.
//! INSP-BROKER-03: ORPHAN_HOLDER includes holder_pid + pid_source.
//! INSP-BROKER-04: HEALTHY=0; down-states=1; diagnosis on stdout.
//! INSP-CLI-02: --json on every state.

use clap::Args;
use famp_inspect_client::{peer_pid, raw_connect_probe, PidSource, ProbeOutcome};
use famp_inspect_proto::{InspectBrokerReply, InspectBrokerRequest, InspectKind};
use serde::Serialize;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectBrokerArgs {
    /// Emit JSON output instead of a single human-readable line.
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
enum BrokerStateRender {
    Healthy {
        pid: u32,
        socket_path: String,
        started_at_unix_seconds: u64,
        build_version: String,
    },
    DownClean {
        socket_path: String,
        evidence: &'static str,
    },
    StaleSocket {
        socket_path: String,
        evidence: &'static str,
    },
    OrphanHolder {
        socket_path: String,
        holder_pid: Option<u32>,
        pid_source: PidSource,
        evidence: String,
    },
    PermissionDenied {
        socket_path: String,
        evidence: &'static str,
    },
}

pub async fn run(args: InspectBrokerArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let sock_str = sock.to_string_lossy().into_owned();
    let outcome = raw_connect_probe(&sock).await;

    let render = match outcome {
        ProbeOutcome::Healthy { mut stream } => {
            let kind = InspectKind::Broker(InspectBrokerRequest::default());
            match famp_inspect_client::call(&mut stream, kind).await {
                Ok(payload) => match serde_json::from_value::<InspectBrokerReply>(payload) {
                    Ok(reply) => BrokerStateRender::Healthy {
                        pid: reply.pid,
                        socket_path: reply.socket_path,
                        started_at_unix_seconds: reply.started_at_unix_seconds,
                        build_version: reply.build_version,
                    },
                    Err(e) => {
                        orphan_holder(&sock, &sock_str, format!("schema_mismatch: {e}")).await
                    }
                },
                Err(e) => {
                    orphan_holder(&sock, &sock_str, format!("inspect_call_failed: {e}")).await
                }
            }
        }
        ProbeOutcome::DownClean => BrokerStateRender::DownClean {
            socket_path: sock_str.clone(),
            evidence: "no_socket_file",
        },
        ProbeOutcome::StaleSocket => BrokerStateRender::StaleSocket {
            socket_path: sock_str.clone(),
            evidence: "connect_econnrefused",
        },
        ProbeOutcome::OrphanHolder {
            hello_reject_summary,
        } => {
            orphan_holder(
                &sock,
                &sock_str,
                format!("hello_rejected: {hello_reject_summary}"),
            )
            .await
        }
        ProbeOutcome::PermissionDenied => BrokerStateRender::PermissionDenied {
            socket_path: sock_str.clone(),
            evidence: "connect_eacces",
        },
    };

    if args.json {
        let s = serde_json::to_string_pretty(&render)
            .map_err(|e| CliError::Generic(format!("json serialize: {e}")))?;
        println!("{s}");
    } else {
        println!("{}", render_human(&render));
    }

    match render {
        BrokerStateRender::Healthy { .. } => Ok(()),
        _ => Err(CliError::Exit(1)),
    }
}

async fn orphan_holder(
    sock: &std::path::Path,
    sock_str: &str,
    evidence: String,
) -> BrokerStateRender {
    let (pid, source) = peer_pid(sock).await.unwrap_or((None, PidSource::Unknown));
    BrokerStateRender::OrphanHolder {
        socket_path: sock_str.to_string(),
        holder_pid: pid,
        pid_source: source,
        evidence,
    }
}

fn render_human(r: &BrokerStateRender) -> String {
    match r {
        BrokerStateRender::Healthy {
            pid,
            socket_path,
            started_at_unix_seconds,
            build_version,
        } => format!(
            "state: HEALTHY pid={pid} socket={socket_path} started_at={ts} build={build_version}",
            ts = format_unix(*started_at_unix_seconds),
        ),
        BrokerStateRender::DownClean {
            socket_path,
            evidence,
        } => format!("state: DOWN_CLEAN socket={socket_path} evidence={evidence}"),
        BrokerStateRender::StaleSocket {
            socket_path,
            evidence,
        } => format!("state: STALE_SOCKET socket={socket_path} evidence={evidence}"),
        BrokerStateRender::OrphanHolder {
            socket_path,
            holder_pid,
            pid_source,
            evidence,
        } => {
            let pid_str = holder_pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let source_str = match pid_source {
                PidSource::Peercred => "peercred",
                PidSource::Lsof => "lsof",
                PidSource::Unknown => "unknown",
            };
            format!(
                "state: ORPHAN_HOLDER socket={socket_path} holder_pid={pid_str} pid_source={source_str} evidence={evidence}"
            )
        }
        BrokerStateRender::PermissionDenied {
            socket_path,
            evidence,
        } => format!("state: PERMISSION_DENIED socket={socket_path} evidence={evidence}"),
    }
}

fn format_unix(secs: u64) -> String {
    match time::OffsetDateTime::from_unix_timestamp(secs as i64) {
        Ok(t) => t
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| secs.to_string()),
        Err(_) => secs.to_string(),
    }
}
