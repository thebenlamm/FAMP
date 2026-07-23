//! `famp-gateway` — killable process backing 1+ remote principals on the
//! local UDS bus (LIVE-02: a real OS process the broker's `kill(pid,0)`
//! liveness sweep can observe alive or dead).
//!
//! Usage: `famp-gateway [--socket <path>] <principal-name>...`
//! `--socket` defaults to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock`
//! (`famp::bus_client::resolve_sock_path`).

use std::path::PathBuf;

use famp_gateway::GatewayRegistry;

// Silencer: this bin doesn't reference famp-bus or thiserror types
// directly — those are used inside the famp-gateway lib (principal.rs /
// error.rs), not here. `famp` and `tokio` and `famp_gateway` ARE used
// below (resolve_sock_path, #[tokio::main]/signal::ctrl_c, GatewayRegistry).
use famp_bus as _;
use thiserror as _;

// Silencer for dev-only dependencies: these are used exclusively by the
// `tests/liveness.rs` / `tests/no_cross_talk.rs` integration test
// binaries (07-03), separate compilation units from this bin's own
// unittest build.
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use famp_inspect_proto as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tempfile as _;
#[cfg(test)]
use uuid as _;

/// Parse `--socket <path>` plus one-or-more positional principal names.
/// Extracted as a pure function so argument handling is testable without
/// a live broker socket.
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<(PathBuf, Vec<String>), String> {
    let _bin = args.next();
    let mut sock: Option<PathBuf> = None;
    let mut names = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--socket" {
            match args.next() {
                Some(path) => sock = Some(PathBuf::from(path)),
                None => return Err("--socket requires a path argument".to_owned()),
            }
        } else {
            names.push(arg);
        }
    }
    if names.is_empty() {
        return Err("usage: famp-gateway [--socket <path>] <principal-name>...".to_owned());
    }
    let sock = sock.unwrap_or_else(famp::bus_client::resolve_sock_path);
    Ok((sock, names))
}

#[tokio::main]
async fn main() {
    let (sock, names) = match parse_args(std::env::args()) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("famp-gateway: {msg}");
            std::process::exit(1);
        }
    };

    let mut registry = GatewayRegistry::default();
    for name in names {
        if let Err(e) = registry.back(&sock, name.clone()).await {
            eprintln!("famp-gateway: failed to back principal '{name}': {e}");
            std::process::exit(1);
        }
    }

    {
        let backed: Vec<&str> = registry.names().collect();
        println!(
            "famp-gateway: ready, backing {} principal(s): {}",
            backed.len(),
            backed.join(", ")
        );
    }

    // Park until signalled/killed. Holding `registry` in scope keeps every
    // ProxiedPrincipal's UDS connection open — that is what keeps the
    // broker reporting this gateway's own PID as each principal's live
    // registration (Design A — LIVE-01/LIVE-02).
    let _ = tokio::signal::ctrl_c().await;
}
