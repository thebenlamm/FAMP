//! `famp-gateway` — killable process backing 1+ remote principals on the
//! local UDS bus (LIVE-02: a real OS process the broker's `kill(pid,0)`
//! liveness sweep can observe alive or dead), and (Phase 9) the live
//! bidirectional cross-host relay: it listens for inbound HTTPS deliveries
//! (`run_ingress`, 09-03) and drains+signs+POSTs outbound mailbox traffic
//! for every backed principal (`run_egress`, 09-02), sharing exactly ONE
//! `Arc<Mutex<GatewayRegistry>>` between both directions (the
//! shared-connection contract — see `egress`/`ingress` module docs).
//!
//! Usage:
//! ```text
//! famp-gateway [--socket <path>] --listen <addr> --tls-cert <path>
//!              --tls-key <path> [--peer <domain>=<url>]... [--trust-cert <path>]
//!              <principal-name>...
//! ```
//! `--socket` defaults to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock`
//! (`famp::bus_client::resolve_sock_path`). `--listen`/`--tls-cert`/
//! `--tls-key` are required — a gateway with no inbound listener has no
//! way to relay. `--peer <domain>=<url>` (repeatable) is the D-02
//! `to_domain` -> remote gateway base URL map. The gateway's own signing
//! identity and pinned peers keyring are loaded from `$FAMP_HOME` (or
//! `$HOME/.famp` — `famp::cli::home::resolve_famp_home`), isolated per
//! process from `--socket`'s bus/mailbox isolation (09-RESEARCH.md §7).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use famp::{FampSigningKey, Principal, TrustedVerifyingKey};
use famp_gateway::egress::run_egress;
use famp_gateway::ingress::run_ingress;
use famp_gateway::GatewayRegistry;
use famp_keyring::Keyring;
use famp_transport_http::HttpTransport;
use tokio::sync::Mutex;
use url::Url;

// Silencer: this bin doesn't reference famp-bus or thiserror types
// directly — those are used inside the famp-gateway lib (principal.rs /
// error.rs), not here.
use famp_bus as _;
use thiserror as _;

// Silencer: `famp-envelope` is a direct dep added for `verify_inbound`
// (08-03 Task 2, used only inside the lib's `verify.rs`/`ingress.rs`);
// this bin target doesn't reference it directly.
use famp_envelope as _;

// Silencer: `axum`/`famp-crypto`/`famp-transport`/`serde_json`/`time`/
// `tower`/`tower-http`/`uuid` back the relay implementation inside the
// lib's `egress.rs`/`ingress.rs` modules (09-02/09-03) — this bin target
// consumes those modules' public functions, not these crates, directly.
use axum as _;
use famp_crypto as _;
use famp_transport as _;
use serde_json as _;
use time as _;
use tower as _;
use tower_http as _;
use uuid as _;

// Silencer for dev-only dependencies: these are used exclusively by the
// `tests/liveness.rs` / `tests/no_cross_talk.rs` integration test
// binaries (07-03), separate compilation units from this bin's own
// unittest build.
#[cfg(test)]
use assert_cmd as _;
#[cfg(test)]
use famp_inspect_proto as _;
#[cfg(test)]
use tempfile as _;

/// Parsed cross-host CLI surface (09-04). Pure, I/O-free — `$FAMP_HOME`
/// resolution and the `--socket` env-var default both happen at `main()`
/// time (`resolve_famp_home`/`resolve_sock_path`), not inside this
/// struct or `parse_args` itself, so argument handling stays testable
/// without a live broker socket or filesystem.
#[derive(Debug, PartialEq)]
struct GatewayArgs {
    sock: PathBuf,
    names: Vec<String>,
    listen: SocketAddr,
    tls_cert: PathBuf,
    tls_key: PathBuf,
    /// D-02: `to_domain` -> remote gateway base URL, one entry per
    /// `--peer <domain>=<url>` flag (repeatable).
    peers: Vec<(String, Url)>,
    trust_cert: Option<PathBuf>,
}

/// Parse `--socket <path>`, `--listen <addr>`, `--tls-cert <path>`,
/// `--tls-key <path>`, `--peer <domain>=<url>` (repeatable),
/// `--trust-cert <path>`, plus one-or-more positional principal names.
///
/// `--listen`/`--tls-cert`/`--tls-key` are required: a gateway with no
/// cross-host flags can back principals but has no way to relay them
/// anywhere, so positional-name-only invocations error clearly rather
/// than silently parking. A malformed `--peer` value (missing `=`, or an
/// empty domain) is a parse error naming the expected `<domain>=<url>`
/// shape.
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<GatewayArgs, String> {
    let _bin = args.next();
    let mut sock: Option<PathBuf> = None;
    let mut names = Vec::new();
    let mut listen: Option<SocketAddr> = None;
    let mut tls_cert: Option<PathBuf> = None;
    let mut tls_key: Option<PathBuf> = None;
    let mut peers: Vec<(String, Url)> = Vec::new();
    let mut trust_cert: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--socket" => {
                let path = args.next().ok_or("--socket requires a path argument")?;
                sock = Some(PathBuf::from(path));
            }
            "--listen" => {
                let raw = args
                    .next()
                    .ok_or("--listen requires an address argument, e.g. 127.0.0.1:8443")?;
                let addr = raw
                    .parse::<SocketAddr>()
                    .map_err(|e| format!("--listen: invalid address '{raw}': {e}"))?;
                listen = Some(addr);
            }
            "--tls-cert" => {
                let path = args.next().ok_or("--tls-cert requires a path argument")?;
                tls_cert = Some(PathBuf::from(path));
            }
            "--tls-key" => {
                let path = args.next().ok_or("--tls-key requires a path argument")?;
                tls_key = Some(PathBuf::from(path));
            }
            "--peer" => {
                let raw = args
                    .next()
                    .ok_or("--peer requires a <domain>=<url> argument")?;
                let (domain, url_str) = raw.split_once('=').ok_or_else(|| {
                    format!("--peer: malformed value '{raw}', expected <domain>=<url>")
                })?;
                if domain.is_empty() {
                    return Err(format!(
                        "--peer: empty domain in '{raw}', expected <domain>=<url>"
                    ));
                }
                let url = Url::parse(url_str)
                    .map_err(|e| format!("--peer: invalid url in '{raw}': {e}"))?;
                peers.push((domain.to_owned(), url));
            }
            "--trust-cert" => {
                let path = args.next().ok_or("--trust-cert requires a path argument")?;
                trust_cert = Some(PathBuf::from(path));
            }
            _ => names.push(arg),
        }
    }

    if names.is_empty() {
        return Err(
            "usage: famp-gateway [--socket <path>] --listen <addr> --tls-cert <path> \
             --tls-key <path> [--peer <domain>=<url>]... [--trust-cert <path>] \
             <principal-name>..."
                .to_owned(),
        );
    }
    let listen = listen.ok_or("--listen <addr> is required, e.g. --listen 127.0.0.1:8443")?;
    let tls_cert = tls_cert.ok_or("--tls-cert <path> is required")?;
    let tls_key = tls_key.ok_or("--tls-key <path> is required")?;
    let sock = sock.unwrap_or_else(famp::bus_client::resolve_sock_path);

    Ok(GatewayArgs {
        sock,
        names,
        listen,
        tls_cert,
        tls_key,
        peers,
        trust_cert,
    })
}

#[tokio::main]
async fn main() {
    let args = match parse_args(std::env::args()) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("famp-gateway: {msg}");
            std::process::exit(1);
        }
    };

    let mut registry = GatewayRegistry::default();
    for name in &args.names {
        if let Err(e) = registry.back(&args.sock, name.clone()).await {
            eprintln!("famp-gateway: failed to back principal '{name}': {e}");
            std::process::exit(1);
        }
    }

    let backed_names: Vec<String> = registry.names().map(str::to_owned).collect();
    println!(
        "famp-gateway: ready, backing {} principal(s): {}",
        backed_names.len(),
        backed_names.join(", ")
    );

    let home = match famp::cli::home::resolve_famp_home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("famp-gateway: failed to resolve FAMP_HOME: {e}");
            std::process::exit(1);
        }
    };
    let identity_path = famp::cli::peer::identity::gateway_identity_path(&home);
    let peers_path = famp::cli::peer::identity::gateway_peers_keyring_path(&home);

    // Fail fast on a corrupt/unreadable identity file before any relay
    // task spawns. `FampSigningKey` deliberately does not implement
    // `Clone` (secret-key hygiene, famp-crypto), so per-egress-task keys
    // below are obtained via a fresh `load_or_generate` call each rather
    // than cloning this one — `load_or_generate` is idempotent (T-08-12:
    // the same path always yields the byte-identical key), so this is
    // behaviorally equivalent to loading once and sharing it.
    if let Err(e) = famp::cli::peer::identity::load_or_generate(&identity_path) {
        eprintln!("famp-gateway: failed to load gateway signing key: {e}");
        std::process::exit(1);
    }

    let keyring = match Keyring::load_from_file(&peers_path) {
        Ok(k) => Arc::new(k),
        Err(e) => {
            eprintln!(
                "famp-gateway: failed to load peers keyring at {}: {e}",
                peers_path.display()
            );
            std::process::exit(1);
        }
    };

    let transport = match HttpTransport::new_client_only(args.trust_cert.as_deref()) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            eprintln!("famp-gateway: failed to build outbound HTTPS client: {e}");
            std::process::exit(1);
        }
    };

    // D-02: resolve every backed principal's federation address
    // (`agent:{domain}/{name}`, for each `--peer` domain) to that peer
    // gateway's base URL. A backed name whose domain has no matching
    // `--peer` entry is simply never added to the transport's address
    // map — the resulting egress relay attempt then surfaces as a
    // transport `UnknownRecipient` error (logged, drain loop continues),
    // never a silent drop.
    for (domain, url) in &args.peers {
        for name in &backed_names {
            if let Ok(principal) = format!("agent:{domain}/{name}").parse::<Principal>() {
                transport.add_peer(principal, url.clone()).await;
            }
        }
    }

    // SHARED-CONNECTION CONTRACT (load-bearing for GW-02): exactly ONE
    // `Arc<Mutex<GatewayRegistry>>` is cloned into `run_ingress` and every
    // `run_egress` task below. See `egress.rs`/`ingress.rs`'s module docs
    // for why neither side ever holds this lock across a long await.
    let registry = Arc::new(Mutex::new(registry));

    let mut egress_tasks = tokio::task::JoinSet::new();
    for name in &backed_names {
        let sk: FampSigningKey = match famp::cli::peer::identity::load_or_generate(&identity_path) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("famp-gateway: failed to load signing key for egress[{name}]: {e}");
                std::process::exit(1);
            }
        };
        let vk: TrustedVerifyingKey = sk.verifying_key();
        egress_tasks.spawn(run_egress(
            name.clone(),
            Arc::clone(&registry),
            Arc::clone(&transport),
            sk,
            vk,
        ));
    }

    tokio::select! {
        result = run_ingress(args.listen, &args.tls_cert, &args.tls_key, Arc::clone(&registry), Arc::clone(&keyring)) => {
            if let Err(e) = result {
                eprintln!("famp-gateway: ingress server exited: {e}");
            }
        }
        () = async {
            while egress_tasks.join_next().await.is_some() {}
        } => {
            eprintln!("famp-gateway: all egress drain tasks exited");
        }
        _ = tokio::signal::ctrl_c() => {
            println!("famp-gateway: shutting down (ctrl_c)");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn args<'a>(v: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        std::iter::once("famp-gateway".to_owned()).chain(v.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn parses_full_cross_host_flag_surface() {
        let parsed = parse_args(args(&[
            "--socket",
            "/tmp/bus.sock",
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--trust-cert",
            "/tmp/ca.pem",
            "alice",
        ]))
        .expect("full flag surface must parse");

        assert_eq!(parsed.sock, PathBuf::from("/tmp/bus.sock"));
        assert_eq!(parsed.names, vec!["alice".to_owned()]);
        assert_eq!(
            parsed.listen,
            "127.0.0.1:8443".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(parsed.tls_cert, PathBuf::from("/tmp/cert.pem"));
        assert_eq!(parsed.tls_key, PathBuf::from("/tmp/key.pem"));
        assert_eq!(
            parsed.peers,
            vec![(
                "hostb.test".to_owned(),
                Url::parse("https://127.0.0.1:9443").unwrap()
            )]
        );
        assert_eq!(parsed.trust_cert, Some(PathBuf::from("/tmp/ca.pem")));
    }

    #[test]
    fn peer_flag_is_repeatable() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--peer",
            "hostc.test=https://127.0.0.1:9444",
            "alice",
        ]))
        .expect("repeated --peer must parse");
        assert_eq!(parsed.peers.len(), 2);
    }

    #[test]
    fn trust_cert_defaults_to_none() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .expect("must parse without --trust-cert");
        assert_eq!(parsed.trust_cert, None);
        assert!(parsed.peers.is_empty());
    }

    #[test]
    fn malformed_peer_missing_equals_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "hostb.test-no-equals-sign",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("<domain>=<url>"), "got: {err}");
    }

    #[test]
    fn malformed_peer_empty_domain_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "=https://127.0.0.1:9443",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("empty domain"), "got: {err}");
    }

    #[test]
    fn malformed_peer_invalid_url_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "hostb.test=not a url",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("invalid url"), "got: {err}");
    }

    #[test]
    fn missing_listen_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("--listen"), "got: {err}");
    }

    #[test]
    fn missing_tls_cert_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("--tls-cert"), "got: {err}");
    }

    #[test]
    fn missing_tls_key_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("--tls-key"), "got: {err}");
    }

    #[test]
    fn invalid_listen_address_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "not-an-address",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("--listen"), "got: {err}");
    }

    #[test]
    fn positional_names_only_still_errors_clearly() {
        let err = parse_args(args(&["alice"])).unwrap_err();
        assert!(
            err.contains("--listen"),
            "positional-only invocation must error on the missing cross-host surface, got: {err}"
        );
    }

    #[test]
    fn missing_names_is_a_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
        ]))
        .unwrap_err();
        assert!(err.contains("usage:"), "got: {err}");
    }

    #[test]
    fn socket_defaults_when_omitted() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .expect("must parse without --socket");
        assert_eq!(parsed.sock, famp::bus_client::resolve_sock_path());
    }
}
