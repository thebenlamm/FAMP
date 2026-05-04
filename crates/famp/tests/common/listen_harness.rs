//! Shared helpers for Phase 2 Plan 02-03 `famp listen` integration tests.
//!
//! Scope:
//!
//! - Spawn `famp listen` as a subprocess with `FAMP_HOME` isolated to a
//!   `TempDir`, with stderr piped so tests can read the bound-addr beacon
//!   line (`listening on https://127.0.0.1:<port>`).
//! - Synchronize on bind via a TCP-connect poll so tests don't race the
//!   listener accepting its first connection.
//! - Construct a self-signed `SignedEnvelope<AckBody>` where
//!   `from == to == agent:localhost/self` — matching the single-entry
//!   keyring that `famp::cli::listen::run_on_listener` builds (Plan 02-02).
//!   This is the only principal the daemon's sig-verify middleware can
//!   resolve in Phase 2.
//! - Build a `reqwest::Client` that trusts the daemon's self-signed
//!   `tls.cert.pem` (loaded through `famp_transport_http::tls::build_client_config`
//!   and fed to `reqwest::ClientBuilder::use_preconfigured_tls`). The cert
//!   has SANs for both `localhost` and `127.0.0.1`, so tests connect to
//!   `127.0.0.1` directly and verify passes without a hostname override.
//! - Read the on-disk inbox via `famp_inbox::read::read_all` — the same
//!   code path the future `famp await` subcommand will take.
//!
//! Cleanup contract: every spawned child is wrapped in [`ChildGuard`] so a
//! panicking test still kills its daemon on unwind (T-02-31 mitigation).

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_const_for_fn,
    clippy::single_match_else
)]

use std::{
    io::{BufRead, BufReader},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::FampSigningKey;
use famp_envelope::{
    body::{AckBody, AckDisposition},
    SignedEnvelope, Timestamp, UnsignedEnvelope,
};

/// RAII guard that kills + waits the child on drop. Tests should hold this
/// for the duration of the test body and `mem::drop` it (or let scope end)
/// to clean up even on panic unwind.
pub struct ChildGuard(pub Option<Child>);

impl ChildGuard {
    #[must_use]
    pub fn new(child: Child) -> Self {
        Self(Some(child))
    }

    pub fn as_mut(&mut self) -> Option<&mut Child> {
        self.0.as_mut()
    }

    pub fn take(&mut self) -> Option<Child> {
        self.0.take()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Initialize a FAMP home in-process by calling `famp::cli::init::run_at`
/// directly. Faster than a subprocess and avoids inheriting env vars.
pub fn init_home_in_process(home: &Path) {
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::init::run_at(home, false, &mut out, &mut err).expect("famp init");
}

/// Spawn `famp listen --listen <addr>` as a subprocess under `FAMP_HOME=home`.
///
/// - stderr is piped so the caller can read the "listening on https://..."
///   beacon line (Plan 02-02 D-02 contract).
/// - stdout is inherited as null (nothing useful there).
/// - Does NOT wait for the daemon to bind; use [`wait_for_bind`] or
///   [`read_stderr_bound_addr`] to synchronize.
pub fn spawn_listen(home: &Path, listen_arg: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_famp"))
        .arg("listen")
        .arg("--listen")
        .arg(listen_arg)
        .env("FAMP_HOME", home)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn famp listen")
}

/// Read the child's stderr line-by-line until we find the beacon
/// `listening on https://<ip>:<port>` (Plan 02-02 D-02), parse the port,
/// and return the full `SocketAddr`.
///
/// Returns `Err` if the child exited before printing the line, or if
/// `timeout` elapsed. The error string includes any partial stderr we did
/// read, for actionable failure diagnostics.
pub fn read_stderr_bound_addr(child: &mut Child, timeout: Duration) -> Result<SocketAddr, String> {
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "child stderr not piped".to_string())?;

    // Read on a dedicated thread so we can enforce a wall-clock timeout
    // without blocking the test forever on a hung child. The thread
    // continues draining stderr to EOF AFTER finding the beacon so the
    // child's stderr pipe never fills up (a full stderr pipe would
    // eventually block the daemon on its next eprintln!). The drainer
    // thread is deliberately leaked — it exits on its own when the
    // child closes stderr.
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut collected = String::new();
        let mut beacon_sent = false;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if !beacon_sent {
                        let _ = tx.send(Err(format!(
                            "child stderr closed before printing beacon; collected: {collected}"
                        )));
                    }
                    return;
                }
                Ok(_) => {
                    collected.push_str(&line);
                    if !beacon_sent {
                        if let Some(addr) = parse_listening_line(&line) {
                            let _ = tx.send(Ok(addr));
                            beacon_sent = true;
                        }
                    }
                    // Keep looping so the pipe drains; discard further lines.
                }
                Err(e) => {
                    if !beacon_sent {
                        let _ = tx.send(Err(format!(
                            "stderr read failed: {e}; collected: {collected}"
                        )));
                    }
                    return;
                }
            }
        }
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(line)) => {
            let addr: SocketAddr = line
                .parse()
                .map_err(|e| format!("parse socket addr {line}: {e}"))?;
            Ok(addr)
        }
        Ok(Err(e)) => Err(e),
        Err(_) => {
            // Before giving up, include child exit status if available.
            let status = match child.try_wait() {
                Ok(Some(s)) => format!(" (child exited: {s})"),
                Ok(None) => " (child still running)".to_string(),
                Err(_) => String::new(),
            };
            Err(format!(
                "timed out waiting for `listening on https://...` beacon{status}"
            ))
        }
    }
}

/// Parse a line like `listening on https://127.0.0.1:12345\n` and return
/// the `SocketAddr` portion as a string (`127.0.0.1:12345`). Returns
/// `None` if the line is any other format.
fn parse_listening_line(line: &str) -> Option<String> {
    let prefix = "listening on https://";
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(prefix)?;
    Some(rest.to_string())
}

/// Give an in-process TLS listener a chance to enter its accept loop.
///
/// Do not probe HTTPS listeners with a raw TCP connect. A plaintext
/// connect-and-drop can occupy rustls handshake handling long enough to
/// make the first real HTTPS request time out on this server stack.
pub async fn wait_for_tls_listener_ready() {
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(75)).await;
}

/// Wait briefly after a subprocess prints its bound-address beacon.
///
/// The beacon proves the socket is bound. We intentionally do not poll the
/// TLS port with raw TCP because that can poison the first rustls accept.
/// Also checks the child hasn't already exited.
pub fn wait_for_bind(child: &mut Child, addr: SocketAddr, timeout: Duration) -> Result<(), String> {
    if let Ok(Some(status)) = child.try_wait() {
        return Err(format!("child exited before bind: {status}"));
    }
    std::thread::sleep(timeout.min(Duration::from_millis(150)));
    if let Ok(Some(status)) = child.try_wait() {
        return Err(format!(
            "child exited after bind beacon at {addr}: {status}"
        ));
    }
    Ok(())
}

/// Load the daemon's ed25519 seed from `<home>/key.ed25519` and return
/// the `FampSigningKey`.
pub fn load_self_signing_key(home: &Path) -> FampSigningKey {
    let seed_bytes = std::fs::read(home.join("key.ed25519")).expect("read key.ed25519");
    let seed: [u8; 32] = seed_bytes
        .as_slice()
        .try_into()
        .expect("key.ed25519 must be 32 bytes");
    FampSigningKey::from_bytes(seed)
}

/// Self-principal that Plan 02-02's `listen::run_on_listener` pins into
/// the single-entry keyring. All Phase 2 integration tests sign with this
/// principal for both `from` and `to`.
#[must_use]
pub fn self_principal() -> Principal {
    Principal::from_str("agent:localhost/self").expect("parse self principal")
}

/// Build canonical, signed envelope bytes for a minimal
/// `AckBody { Accepted }` message where `from == to == agent:localhost/self`,
/// signed by the daemon's own key.
///
/// Returned bytes are the exact wire payload to POST — byte-identical to
/// what the sig-verify middleware will re-canonicalize and compare.
pub fn build_signed_ack_bytes(home: &Path) -> Vec<u8> {
    let sk = load_self_signing_key(home);
    let me = self_principal();

    let id = MessageId::new_v7();
    let ts = Timestamp("2026-04-14T00:00:00Z".to_string());
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };

    let unsigned: UnsignedEnvelope<AckBody> =
        UnsignedEnvelope::new(id, me.clone(), me, AuthorityScope::Advisory, ts, body);
    let signed: SignedEnvelope<AckBody> = unsigned.sign(&sk).expect("sign ack");
    signed.encode().expect("encode canonical bytes")
}

/// Build a `reqwest::Client` that trusts `<home>/tls.cert.pem` as an extra
/// root anchor (in addition to the platform trust store).
pub fn build_trusting_reqwest_client(home: &Path) -> reqwest::Client {
    let cert_path: PathBuf = home.join("tls.cert.pem");
    let cert = std::fs::read(&cert_path).expect("read tls.cert.pem");
    let cert = reqwest::Certificate::from_pem(&cert).expect("parse tls.cert.pem");
    reqwest::Client::builder()
        .add_root_certificate(cert)
        .timeout(Duration::from_secs(5))
        .http1_only()
        .build()
        .expect("build reqwest client")
}

/// POST raw envelope bytes to `https://{addr}/famp/v0.5.1/inbox/{principal}`
/// using a client that trusts the daemon's self-signed cert.
///
/// Returns the raw `reqwest::Response` so callers can assert on status
/// (the listen handler returns 200 OK on durable commit).
pub async fn post_bytes(
    client: &reqwest::Client,
    addr: SocketAddr,
    principal: &Principal,
    bytes: Vec<u8>,
) -> reqwest::Result<reqwest::Response> {
    // Percent-encode the principal segment via `url::Url::path_segments_mut`
    // (same technique as famp_transport_http::transport — MED-03), so the
    // `:` / `/` bytes in `agent:localhost/self` become `%3A` / `%2F`.
    let mut url = url::Url::parse(&format!("https://{addr}/")).expect("parse base url");
    {
        let mut segs = url
            .path_segments_mut()
            .expect("https url has path segments");
        segs.pop_if_empty();
        segs.extend(["famp", "v0.5.1", "inbox"]);
        segs.push(&principal.to_string());
    }
    client
        .post(url)
        .header("content-type", "application/famp+json")
        .body(bytes)
        .send()
        .await
}

/// Read every complete JSONL line from `<home>/inbox.jsonl` via
/// `famp_inbox::read::read_all`. Panics if the file doesn't exist or
/// `read_all` returns an error.
pub fn read_inbox_lines(home: &Path) -> Vec<serde_json::Value> {
    famp_inbox::read::read_all(home.join("inbox.jsonl")).expect("read_all")
}

// ---- Silencers for deps the umbrella crate pulls in that this submodule
// doesn't reference directly. Prevents `unused_crate_dependencies` warnings
// when a specific test binary happens to pull only a subset.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_fsm as _;
use famp_keyring as _;
use famp_transport as _;
use rand as _;
use serde as _;
use serde_json as _;
use tempfile as _;
use thiserror as _;
use time as _;
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;

// `Arc` is reachable via the `std::sync::Arc` import above but not
// referenced inside this file; silence via a dead_code-allowed const.
#[allow(dead_code)]
const _UNUSED_ARC: Option<Arc<()>> = None;
