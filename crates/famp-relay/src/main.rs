//! `famp-relay` — the store-and-forward relay Phase 13 chose for
//! REACH-04.
//!
//! Usage:
//! ```text
//! famp-relay --listen <addr> --tls-cert <path> --tls-key <path>
//!            --public-url <url> --domain <domain>=<pubkey> [--domain ...]
//! ```
//!
//! `--domain <domain>=<pubkey>` is repeatable and D-27's whole
//! operator-facing surface: the value after `=` is the SECOND
//! whitespace-separated field of the peer's `famp peer export` output.
//! Adding a peer means adding a `--domain` argument for the peer's
//! domain and restarting this process — a manual step per new peer,
//! accepted friction (D-27) because queue ownership must never be
//! learned first-come; automating it is Phase 16's business, not this
//! crate's.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// Silencers: these back the relay implementation inside the lib's
// `http.rs`/`fetch_auth.rs` modules — this bin target consumes those
// modules' public functions, not these crates, directly.
use axum as _;
use base64 as _;
use serde as _;
use serde_json as _;
use thiserror as _;
use tower as _;
use tower_http as _;
use uuid as _;

use famp_crypto::TrustedVerifyingKey;
use famp_relay::fetch_auth::{normalize_audience, RELAY_MAX_KEYS_PER_DOMAIN};
use famp_relay::http::{build_relay_router, RelayState};
use time::OffsetDateTime;
use url::Url;

/// Interval between background sweep passes over the queue and
/// fetch-auth state, reclaiming TTL-expired entries even for a domain
/// nobody actively fetches.
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct RelayArgs {
    listen: SocketAddr,
    tls_cert: PathBuf,
    tls_key: PathBuf,
    public_url: Url,
    domains: HashMap<String, Vec<TrustedVerifyingKey>>,
}

/// Parse `--listen <addr>`, `--tls-cert <path>`, `--tls-key <path>`,
/// `--public-url <url>`, and a repeatable `--domain <domain>=<pubkey>`.
///
/// All five flag families are required — a relay with no listener, no
/// TLS material, no public URL, or no served domain has nothing useful
/// to do, and each says so at startup rather than accepting traffic it
/// will only 404 or fail to authenticate.
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<RelayArgs, String> {
    let _bin = args.next();
    let mut listen: Option<SocketAddr> = None;
    let mut tls_cert: Option<PathBuf> = None;
    let mut tls_key: Option<PathBuf> = None;
    let mut public_url: Option<Url> = None;
    let mut domains: HashMap<String, Vec<TrustedVerifyingKey>> = HashMap::new();
    let mut seen_keys_b64: HashMap<String, Vec<String>> = HashMap::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
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
            "--public-url" => {
                let raw = args.next().ok_or(
                    "--public-url requires a url argument, e.g. https://relay.example.com",
                )?;
                let url = Url::parse(&raw)
                    .map_err(|e| format!("--public-url: invalid url '{raw}': {e}"))?;
                public_url = Some(url);
            }
            "--domain" => {
                let raw = args
                    .next()
                    .ok_or("--domain requires a <domain>=<pubkey> argument")?;
                let (domain, key_str) = raw.split_once('=').ok_or_else(|| {
                    format!("--domain: malformed value '{raw}', expected <domain>=<pubkey>")
                })?;
                if domain.is_empty() {
                    return Err(format!(
                        "--domain: empty domain in '{raw}', expected <domain>=<pubkey>"
                    ));
                }
                let key = TrustedVerifyingKey::from_b64url(key_str).map_err(|e| {
                    format!(
                        "--domain: the value after '=' for domain '{domain}' is not a valid \
                         public key ({e}) — this must be the SECOND whitespace-separated field \
                         of the peer's `famp peer export` output"
                    )
                })?;

                let seen = seen_keys_b64.entry(domain.to_owned()).or_default();
                if seen.iter().any(|k| k == key_str) {
                    return Err(format!(
                        "--domain: duplicate domain-and-key pair for domain '{domain}' — each \
                         --domain entry must be a distinct key"
                    ));
                }
                if seen.len() >= RELAY_MAX_KEYS_PER_DOMAIN {
                    return Err(format!(
                        "--domain: domain '{domain}' already has {RELAY_MAX_KEYS_PER_DOMAIN} \
                         configured keys (RELAY_MAX_KEYS_PER_DOMAIN) — remove an old key before \
                         adding another"
                    ));
                }
                seen.push(key_str.to_owned());
                domains.entry(domain.to_owned()).or_default().push(key);
            }
            other => {
                return Err(format!("famp-relay: unrecognized argument '{other}'"));
            }
        }
    }

    let listen = listen.ok_or("--listen <addr> is required, e.g. --listen 0.0.0.0:8443")?;
    let tls_cert = tls_cert.ok_or("--tls-cert <path> is required")?;
    let tls_key = tls_key.ok_or("--tls-key <path> is required")?;
    let public_url = public_url.ok_or(
        "--public-url <url> is required — it is the audience every fetch signature is bound \
         to, so a wrong or absent value fails every fetch with an audience mismatch",
    )?;
    if domains.is_empty() {
        return Err(
            "at least one --domain <domain>=<pubkey> is required — a relay serving no domains \
             has nothing to do"
                .to_owned(),
        );
    }

    Ok(RelayArgs {
        listen,
        tls_cert,
        tls_key,
        public_url,
        domains,
    })
}

/// Background sweep loop: reclaims TTL-expired queue entries and nonce
/// records on an interval, so a domain nobody actively fetches still
/// gets reclaimed rather than growing unbounded between fetches.
async fn run_sweep_loop(state: RelayState) {
    let mut interval = tokio::time::interval(SWEEP_INTERVAL);
    loop {
        interval.tick().await;
        let now = OffsetDateTime::now_utc();
        state.queues().lock().await.sweep(now);
        state.auth().lock().await.sweep(now);
    }
}

#[tokio::main]
async fn main() {
    let args = match parse_args(std::env::args()) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("famp-relay: {msg}");
            eprintln!(
                "usage: famp-relay --listen <addr> --tls-cert <path> --tls-key <path> \
                 --public-url <url> --domain <domain>=<pubkey> [--domain ...]"
            );
            std::process::exit(1);
        }
    };

    // Public by construction: this is the audience every fetch signature
    // is bound to, and it is what the operator publishes in the
    // follower doc. Printed on the readiness line below for exactly
    // that reason — there is no secret in this process to leak into a
    // log or a process listing (D-26 reason 3's operational payoff).
    let audience: Arc<str> = Arc::from(normalize_audience(&args.public_url));

    let domain_summary: Vec<String> = {
        let mut names: Vec<&String> = args.domains.keys().collect();
        names.sort();
        names
            .into_iter()
            .map(|d| format!("{d} ({} key(s))", args.domains[d].len()))
            .collect()
    };

    let state = RelayState::new(args.domains, Arc::clone(&audience));

    let cert = match famp_transport_http::tls::load_pem_cert(&args.tls_cert) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("famp-relay: failed to load TLS cert: {e}");
            std::process::exit(1);
        }
    };
    let key = match famp_transport_http::tls::load_pem_key(&args.tls_key) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("famp-relay: failed to load TLS key: {e}");
            std::process::exit(1);
        }
    };
    let server_config = match famp_transport_http::tls::build_server_config(cert, key) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("famp-relay: failed to build TLS server config: {e}");
            std::process::exit(1);
        }
    };

    let listener = match std::net::TcpListener::bind(args.listen) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("famp-relay: failed to bind {}: {e}", args.listen);
            std::process::exit(1);
        }
    };
    if let Err(e) = listener.set_nonblocking(true) {
        eprintln!("famp-relay: failed to set listener non-blocking: {e}");
        std::process::exit(1);
    }

    // T-11-21-style readiness contract: print AFTER every fallible setup
    // step above (parse, TLS load, bind) has already succeeded, so a
    // truncated/failed init never prints a false "ready".
    println!(
        "famp-relay: ready, serving domain(s): {} — audience {audience}",
        domain_summary.join(", ")
    );

    let router = build_relay_router(state.clone());

    tokio::spawn(run_sweep_loop(state));

    let handle = famp_transport_http::tls_server::serve_std_listener(
        listener,
        router,
        Arc::new(server_config),
    );

    match handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("famp-relay: server exited: {e}");
            std::process::exit(1);
        }
        Err(join_err) => {
            eprintln!("famp-relay: server task panicked: {join_err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp_crypto::FampSigningKey;

    // Silencer: `reqwest`/`assert_cmd` are dev-dependencies consumed
    // only by `tests/relay_store_and_forward.rs`, a separate
    // compilation unit from this bin target's own `#[cfg(test)]` unit
    // tests.
    use assert_cmd as _;
    use reqwest as _;

    fn args<'a>(v: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        std::iter::once("famp-relay".to_owned()).chain(v.iter().map(|s| (*s).to_owned()))
    }

    fn test_key() -> String {
        FampSigningKey::generate().verifying_key().to_b64url()
    }

    #[test]
    fn parses_full_flag_surface() {
        let key = test_key();
        let domain_flag = format!("hosta.test={key}");
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &domain_flag,
        ]))
        .expect("full flag surface must parse");

        assert_eq!(parsed.listen, "127.0.0.1:8443".parse().unwrap());
        assert_eq!(parsed.tls_cert, PathBuf::from("/tmp/cert.pem"));
        assert_eq!(parsed.tls_key, PathBuf::from("/tmp/key.pem"));
        assert_eq!(parsed.public_url.as_str(), "https://relay.example.com/");
        assert_eq!(parsed.domains.len(), 1);
        assert_eq!(parsed.domains["hosta.test"].len(), 1);
    }

    #[test]
    fn repeated_domain_accumulates_keys() {
        let key1 = test_key();
        let key2 = test_key();
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &format!("hosta.test={key1}"),
            "--domain",
            &format!("hosta.test={key2}"),
        ]))
        .expect("accumulating --domain must parse");
        assert_eq!(parsed.domains["hosta.test"].len(), 2);
    }

    #[test]
    fn duplicate_domain_and_key_pair_is_a_parse_error() {
        let key = test_key();
        let domain_flag = format!("hosta.test={key}");
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &domain_flag,
            "--domain",
            &domain_flag,
        ]))
        .unwrap_err();
        assert!(err.contains("duplicate domain-and-key pair"), "got: {err}");
    }

    #[test]
    fn more_than_max_keys_per_domain_is_a_parse_error() {
        let mut a = vec![
            "--listen".to_owned(),
            "127.0.0.1:8443".to_owned(),
            "--tls-cert".to_owned(),
            "/tmp/cert.pem".to_owned(),
            "--tls-key".to_owned(),
            "/tmp/key.pem".to_owned(),
            "--public-url".to_owned(),
            "https://relay.example.com".to_owned(),
        ];
        for _ in 0..=RELAY_MAX_KEYS_PER_DOMAIN {
            a.push("--domain".to_owned());
            a.push(format!("hosta.test={}", test_key()));
        }
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        let err = parse_args(args(&refs)).unwrap_err();
        assert!(err.contains("RELAY_MAX_KEYS_PER_DOMAIN"), "got: {err}");
    }

    #[test]
    fn domain_value_with_no_equals_sign_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            "hosta.test-no-equals",
        ]))
        .unwrap_err();
        assert!(err.contains("<domain>=<pubkey>"), "got: {err}");
    }

    #[test]
    fn domain_value_with_undecodable_key_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            "hosta.test=not-a-valid-key",
        ]))
        .unwrap_err();
        assert!(err.contains("not a valid public key"), "got: {err}");
    }

    #[test]
    fn missing_listen_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &format!("hosta.test={}", test_key()),
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
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &format!("hosta.test={}", test_key()),
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
            "--public-url",
            "https://relay.example.com",
            "--domain",
            &format!("hosta.test={}", test_key()),
        ]))
        .unwrap_err();
        assert!(err.contains("--tls-key"), "got: {err}");
    }

    #[test]
    fn missing_public_url_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--domain",
            &format!("hosta.test={}", test_key()),
        ]))
        .unwrap_err();
        assert!(err.contains("--public-url"), "got: {err}");
    }

    #[test]
    fn missing_domain_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--public-url",
            "https://relay.example.com",
        ]))
        .unwrap_err();
        assert!(err.contains("--domain"), "got: {err}");
    }
}
