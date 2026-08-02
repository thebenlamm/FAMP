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
//!              --tls-key <path> [--peer <domain>=<url>]...
//!              [--backs agent:<domain>/<name>]... [--trust-cert <path>]
//!              [--relay-fetch <url>] <principal-name>...
//! ```
//! `--socket` defaults to `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock`
//! (`famp::bus_client::resolve_sock_path`). `--listen`/`--tls-cert`/
//! `--tls-key` are required — a gateway with no inbound listener has no
//! way to relay. `--peer <domain>=<url>` (repeatable, no duplicate
//! domains — T-11-28) is the D-02 `to_domain` -> remote gateway base URL
//! map. `--backs agent:<domain>/<name>` (repeatable, F-3/T-11-27)
//! explicitly binds one locally-backed principal to its route; with
//! exactly one `--peer` configured, bare positional principal names may
//! still be used instead (the unambiguous single-peer topology this
//! milestone's two-machine UAT depends on) — with two or more `--peer`
//! entries, bare names are a startup-fatal ambiguity and `--backs` is
//! required. `--relay-fetch <url>` (17-03/D-26) is plan 05's
//! signed-fetch relay-polling URL — parsed here, unwired until plan 05.
//! There is no `--relay-token` flag; signed-fetch needs no shared-secret
//! credential. The gateway's own signing identity and pinned peers
//! keyring are loaded from `$FAMP_HOME` (or `$HOME/.famp` —
//! `famp::cli::home::resolve_famp_home`), isolated per process from
//! `--socket`'s bus/mailbox isolation (09-RESEARCH.md §7).
//!
//! **`own_domain` is UNCONDITIONALLY startup-fatal (17-03/D-30).** Every
//! gateway process — not merely a relay-reachable one — refuses to start
//! without a resolvable own-domain (`--domain` is not itself a gateway
//! flag; resolution is `FAMP_OWN_DOMAIN` env > `$FAMP_HOME/own-domain`
//! file, per `famp::cli::own_domain::resolve_own_domain`'s documented
//! precedence). See [`resolve_own_domain_or_exit`]'s doc comment for the
//! full rationale and 17-RESEARCH.md Pitfall 4.

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
// REACH-05 (17-06): `famp-fsm` is a dev-dep used only by
// `egress.rs`'s `ack_class_has_no_fsm_effect` test (the lib target's own
// test compile unit) — this bin's test compile unit has no direct
// reference.
#[cfg(test)]
use famp_fsm as _;

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
    /// `--peer <domain>=<url>` flag (repeatable, no duplicate domains —
    /// T-11-28).
    peers: Vec<(String, Url)>,
    /// F-3/T-11-27: explicit `agent:<domain>/<name>` route bindings, one
    /// per `--backs` flag (repeatable, no duplicate principals). Empty
    /// when the operator relies on the bare-positional-names + single-peer
    /// fallback instead.
    backs: Vec<Principal>,
    trust_cert: Option<PathBuf>,
    /// D-26/17-CONTEXT.md: plan 05's signed-fetch relay-polling URL.
    /// Parsed here (a related CLI surface addition, unrelated to the
    /// own-domain fatality change below) because it's a natural sibling
    /// of `--peer`/`--trust-cert` and plan 05 builds on top of an already
    /// -wired flag rather than adding its own parser. NO `--relay-token`
    /// flag exists or will be added here — signed-fetch needs no
    /// shared-secret credential (D-26); plan 05 owns whatever
    /// signed-fetch-specific flags it needs beyond this URL.
    relay_fetch: Option<Url>,
}

/// Parse `--socket <path>`, `--listen <addr>`, `--tls-cert <path>`,
/// `--tls-key <path>`, `--peer <domain>=<url>` (repeatable, no duplicate
/// domains), `--backs agent:<domain>/<name>` (repeatable, no duplicate
/// principals), `--trust-cert <path>`, plus one-or-more positional
/// principal names.
///
/// `--listen`/`--tls-cert`/`--tls-key` are required: a gateway with no
/// cross-host flags can back principals but has no way to relay them
/// anywhere, so positional-name-only invocations error clearly rather
/// than silently parking. A malformed `--peer` value (missing `=`, an
/// empty domain, or a DUPLICATE domain — T-11-28) is a parse error. A
/// malformed or duplicate `--backs` value is likewise a parse error.
/// Parse one `--peer <domain>=<url>` value into `peers`, mirroring
/// `KeyringError::KeyConflict`'s fail-closed shape (T-11-28): a repeated
/// domain is a startup error naming both URLs, never last-write-wins.
/// Extracted from `parse_args` solely to satisfy `clippy::too_many_lines`,
/// mirroring `main()`'s own `resolve_own_domain_or_exit`/`build_route_map`
/// extraction precedent.
fn parse_peer_flag(raw: &str, peers: &mut Vec<(String, Url)>) -> Result<(), String> {
    let (domain, url_str) = raw
        .split_once('=')
        .ok_or_else(|| format!("--peer: malformed value '{raw}', expected <domain>=<url>"))?;
    if domain.is_empty() {
        return Err(format!(
            "--peer: empty domain in '{raw}', expected <domain>=<url>"
        ));
    }
    let url = Url::parse(url_str).map_err(|e| format!("--peer: invalid url in '{raw}': {e}"))?;
    if let Some((_, existing_url)) = peers.iter().find(|(d, _)| d == domain) {
        return Err(format!(
            "--peer: duplicate domain '{domain}': already mapped to '{existing_url}', now \
             given '{url}' — remove one (ambiguous route configuration must fail closed, \
             never last-write-wins)"
        ));
    }
    peers.push((domain.to_owned(), url));
    Ok(())
}

/// Parse one `--backs agent:<domain>/<name>` value into `backs`. Extracted
/// from `parse_args` for the same `too_many_lines` reason as
/// [`parse_peer_flag`].
fn parse_backs_flag(raw: &str, backs: &mut Vec<Principal>) -> Result<(), String> {
    let principal: Principal = raw
        .parse()
        .map_err(|e| format!("--backs: invalid principal '{raw}': {e}"))?;
    if backs.contains(&principal) {
        return Err(format!(
            "--backs: duplicate principal '{principal}' — each --backs binding must be unique"
        ));
    }
    backs.push(principal);
    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<GatewayArgs, String> {
    let _bin = args.next();
    let mut sock: Option<PathBuf> = None;
    let mut names = Vec::new();
    let mut listen: Option<SocketAddr> = None;
    let mut tls_cert: Option<PathBuf> = None;
    let mut tls_key: Option<PathBuf> = None;
    let mut peers: Vec<(String, Url)> = Vec::new();
    let mut backs: Vec<Principal> = Vec::new();
    let mut trust_cert: Option<PathBuf> = None;
    let mut relay_fetch: Option<Url> = None;

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
                parse_peer_flag(&raw, &mut peers)?;
            }
            "--backs" => {
                let raw = args
                    .next()
                    .ok_or("--backs requires a <principal> argument, e.g. agent:hostb.test/bob")?;
                parse_backs_flag(&raw, &mut backs)?;
            }
            "--trust-cert" => {
                let path = args.next().ok_or("--trust-cert requires a path argument")?;
                trust_cert = Some(PathBuf::from(path));
            }
            "--relay-fetch" => {
                let raw = args.next().ok_or("--relay-fetch requires a url argument")?;
                let url = Url::parse(&raw)
                    .map_err(|e| format!("--relay-fetch: invalid url '{raw}': {e}"))?;
                relay_fetch = Some(url);
            }
            _ => names.push(arg),
        }
    }

    if names.is_empty() {
        return Err(
            "usage: famp-gateway [--socket <path>] --listen <addr> --tls-cert <path> \
             --tls-key <path> [--peer <domain>=<url>]... [--backs agent:<domain>/<name>]... \
             [--trust-cert <path>] [--relay-fetch <url>] <principal-name>..."
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
        backs,
        trust_cert,
        relay_fetch,
    })
}

/// Resolve this host's own-domain federation authority (plan 02's single
/// source) at startup.
///
/// **17-03/D-30/INGR-04: UNCONDITIONALLY startup-fatal.** Prior to this
/// phase, an unresolved own-domain fell back to a "sign/relay verbatim,
/// skip the domain check" posture (T-11-19/T-11-29) — reasonable before
/// REACH-04 shipped a real open-internet ingress path. own-domain being
/// optional was a vacuous security check that reports success, which is
/// worse than an absent one: [`crate::ingress_guard::audience_check`]'s
/// domain half is this gateway's ONLY defense against accepting envelopes
/// addressed to a domain it does not own, and egress's `FromDomainMismatch`
/// check (`egress.rs::relay_one`) is the ONLY defense against SIGNING as a
/// domain it does not own. Neither defense can be silently disabled
/// anymore — not even for the loopback/test topology (17-RESEARCH.md
/// Pitfall 4; famp-lead-730, 2026-07-31 19:32, task 019fb955: "make
/// own_domain MANDATORY at gateway startup in this phase... If that
/// breaks an existing test fixture or the e2e harness, tell me rather
/// than weakening the check").
///
/// Every resolution failure — unset (`CliError::OwnDomainNotSet`) OR
/// present-but-invalid (fails the probe `Principal` parse,
/// `CliError::OwnDomainInvalid`) — now exits the process with a message
/// naming the fix (`FAMP_OWN_DOMAIN` env var, or a single-line
/// `$FAMP_HOME/own-domain` file — this gateway has no `--domain` CLI
/// flag of its own; see `famp::cli::own_domain::resolve_own_domain`'s
/// documented precedence). There is no more warn-and-continue path, full
/// stop. The return type is plain `String`, not `Option<String>`: every
/// failure path above exits the process, so `None` is not a value this
/// function can produce, and every downstream consumer
/// (`egress.rs::relay_one`, `ingress.rs::GatewayIngressState`) now takes
/// the resolved domain as a non-optional `&str`/`String` — the
/// impossibility of an absent own-domain is enforced by the compiler, not
/// by convention.
fn resolve_own_domain_or_exit(home: &std::path::Path) -> String {
    match famp::cli::own_domain::resolve_own_domain(None, home) {
        Ok(domain) => domain,
        Err(famp::cli::error::CliError::OwnDomainNotSet) => {
            eprintln!(
                "famp-gateway: own-domain not set — this is now REQUIRED for every gateway \
                 (17-03/INGR-04). Set it via the FAMP_OWN_DOMAIN env var, or a single-line \
                 $FAMP_HOME/own-domain file."
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("famp-gateway: invalid own-domain configuration: {e}");
            std::process::exit(1);
        }
    }
}

/// F-3/F-4 (T-11-27/T-11-28): populate `transport`'s route map from ONLY
/// explicit operator-declared bindings — the peer x backed-name
/// cross-product that used to manufacture fabricated principal->route
/// bindings is gone. Extracted from `main()` solely to satisfy
/// `clippy::too_many_lines`, mirroring 11-07's `resolve_own_domain_or_exit`
/// extraction precedent.
///
/// Two paths:
///
///   1. `--backs agent:<domain>/<name>` given: register EXACTLY those
///      bindings, resolved against the matching `--peer` domain's URL. A
///      `--backs` principal whose authority has no matching `--peer` is
///      startup-fatal.
///   2. No `--backs` given: fall back to bare positional names, but ONLY
///      when exactly one `--peer` is configured (the unambiguous
///      single-peer topology this milestone's two-machine UAT depends on
///      — `e2e_cross_host_delivery.rs`/`no_cross_talk.rs` keep working
///      unchanged). Two or more peers with bare names is a startup-fatal
///      ambiguity naming the actionable fix. Zero peers is a no-op
///      (matches the legacy cross-product's behavior over an empty
///      `--peer` list — nothing to route yet).
async fn build_route_map(args: &GatewayArgs, backed_names: &[String], transport: &HttpTransport) {
    if args.backs.is_empty() {
        match args.peers.len() {
            0 => {}
            1 => {
                let (domain, url) = &args.peers[0];
                for name in backed_names {
                    // REL-02 (12-02): a `--peer` domain is validated only
                    // for non-emptiness (main.rs's `--peer` parser), never
                    // as a legal `Principal` authority — so a malformed
                    // domain here used to be silently skipped (`if let
                    // Ok(...)`), leaving this backed name with NO route
                    // while the process still printed "ready" and kept
                    // running. Fail loud instead, naming both the domain
                    // and the affected principal name, matching every
                    // other route-configuration failure in this function.
                    match format!("agent:{domain}/{name}").parse::<Principal>() {
                        Ok(principal) => transport.add_peer(principal, url.clone()).await,
                        Err(e) => {
                            eprintln!(
                                "famp-gateway: --peer domain '{domain}' combined with backed \
                                 principal name '{name}' does not form a valid principal \
                                 (agent:{domain}/{name}): {e} — fix the --peer domain or the \
                                 backed principal name"
                            );
                            std::process::exit(1);
                        }
                    }
                }
            }
            n => {
                eprintln!(
                    "famp-gateway: {n} --peer domains configured but bare positional \
                     principal names are ambiguous with more than one peer — use \
                     --backs agent:<domain>/<name> to bind each principal explicitly"
                );
                std::process::exit(1);
            }
        }
    } else {
        for principal in &args.backs {
            let domain = principal.authority();
            let Some((_, url)) = args.peers.iter().find(|(d, _)| d == domain) else {
                eprintln!(
                    "famp-gateway: --backs {principal} has no matching --peer domain '{domain}'"
                );
                std::process::exit(1);
            };
            transport.add_peer(principal.clone(), url.clone()).await;
        }
    }
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

    let home = match famp::cli::home::resolve_famp_home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("famp-gateway: failed to resolve FAMP_HOME: {e}");
            std::process::exit(1);
        }
    };

    // T-11-19/T-11-20: resolve this host's own-domain federation authority
    // ONCE at startup. Kept as an owned `String` local (not folded into a
    // one-shot closure) rather than moved-and-consumed immediately: the
    // per-egress-task loop below only *clones* it, so the same value is
    // still available afterward for a future ingress-side check (11-08)
    // to consume without restructuring this resolution site.
    let own_domain: String = resolve_own_domain_or_exit(&home);

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

    build_route_map(&args, &backed_names, &transport).await;

    // T-11-21: "ready" is printed here — AFTER home resolve, own-domain
    // resolve/validate, signing-key load, keyring load, transport build,
    // and the peer route-map, and BEFORE the ingress/egress spawn below —
    // so a truncated/failed init never prints a false "ready" (review
    // HIGH finding #5's code half). Previously this printed immediately
    // after the registry `back()` loop, before any of the above had run.
    println!(
        "famp-gateway: ready, backing {} principal(s): {}",
        backed_names.len(),
        backed_names.join(", ")
    );

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
            own_domain.clone(),
            Arc::clone(&transport),
            sk,
            vk,
        ));
    }

    // T-11-23/T-11-24: same resolved value threaded into egress above,
    // reused here for ingress's to-authority check — ONE resolution
    // site, two boundaries (11-08's prohibition on a second config
    // source). Converted to `Arc<str>` only at this call site so
    // `own_domain` itself stays an owned `String` local, per 11-07's
    // forward-compat note.
    let ingress_own_domain: Arc<str> = Arc::from(own_domain.clone());

    tokio::select! {
        result = run_ingress(args.listen, &args.tls_cert, &args.tls_key, Arc::clone(&registry), Arc::clone(&keyring), ingress_own_domain) => {
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
    fn duplicate_peer_domain_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--peer",
            "hostb.test=https://127.0.0.1:9443",
            "--peer",
            "hostb.test=https://127.0.0.1:9444",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("duplicate domain"), "got: {err}");
        assert!(err.contains("hostb.test"), "got: {err}");
        assert!(err.contains("9443"), "got: {err}");
        assert!(err.contains("9444"), "got: {err}");
    }

    #[test]
    fn backs_flag_parses_and_is_repeatable() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--backs",
            "agent:hostb.test/bob",
            "--backs",
            "agent:hostc.test/carol",
            "alice",
        ]))
        .expect("repeated --backs must parse");
        assert_eq!(parsed.backs.len(), 2);
        assert_eq!(
            parsed.backs[0],
            "agent:hostb.test/bob".parse::<Principal>().unwrap()
        );
        assert_eq!(
            parsed.backs[1],
            "agent:hostc.test/carol".parse::<Principal>().unwrap()
        );
    }

    #[test]
    fn backs_defaults_to_empty() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .expect("must parse without --backs");
        assert!(parsed.backs.is_empty());
    }

    #[test]
    fn duplicate_backs_principal_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--backs",
            "agent:hostb.test/bob",
            "--backs",
            "agent:hostb.test/bob",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("duplicate principal"), "got: {err}");
    }

    #[test]
    fn malformed_backs_value_is_a_parse_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--backs",
            "not-a-principal",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("invalid principal"), "got: {err}");
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

    #[test]
    fn relay_fetch_flag_parses() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--relay-fetch",
            "https://relay.example/fetch",
            "alice",
        ]))
        .expect("--relay-fetch must parse");
        assert_eq!(
            parsed.relay_fetch,
            Some(Url::parse("https://relay.example/fetch").unwrap())
        );
    }

    #[test]
    fn relay_fetch_defaults_to_none() {
        let parsed = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "alice",
        ]))
        .expect("must parse without --relay-fetch");
        assert_eq!(parsed.relay_fetch, None);
    }

    #[test]
    fn malformed_relay_fetch_url_is_a_distinct_usage_error() {
        let err = parse_args(args(&[
            "--listen",
            "127.0.0.1:8443",
            "--tls-cert",
            "/tmp/cert.pem",
            "--tls-key",
            "/tmp/key.pem",
            "--relay-fetch",
            "not a url",
            "alice",
        ]))
        .unwrap_err();
        assert!(err.contains("--relay-fetch"), "got: {err}");
        assert!(err.contains("invalid url"), "got: {err}");
    }

    /// D-26: signed-fetch needs no shared-secret credential — there is no
    /// `--relay-token` flag. Asserted at the source (not just plan 05's
    /// later grep) so a regression is caught here first.
    #[test]
    fn usage_error_names_relay_fetch_and_never_relay_token() {
        let err = parse_args(args(&[])).unwrap_err();
        assert!(err.contains("--relay-fetch"), "got: {err}");
        assert!(!err.contains("--relay-token"), "got: {err}");
    }
}
