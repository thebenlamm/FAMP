//! Phase 4 Plan 04-03 — two-daemon test harness.
//!
//! Provides [`spawn_two_daemons`] which:
//! 1. Creates two `TempDir`s, each with a distinct `FAMP_HOME`.
//! 2. Initializes each home via `init_home_in_process`.
//! 3. Overwrites `config.toml` on each home to set a distinct self-principal
//!    (`agent:localhost/alice` and `agent:localhost/bob`).
//! 4. Binds two ephemeral `127.0.0.1:0` listeners.
//! 5. Performs mutual peer registration (A knows B, B knows A) with explicit
//!    principal strings so each daemon's keyring can verify the other's envelopes.
//! 6. Spawns both daemons in-process via `famp::cli::listen::run_on_listener`.
//! 7. Returns a [`TwoDaemons`] struct with all handles, addresses, and home
//!    paths needed by E2E tests.
//!
//! [`teardown`](TwoDaemons::teardown) sends both shutdown signals and awaits
//! both join handles with a bounded timeout.
//!
//! ## Principal assignment
//!
//! Daemon A: `agent:localhost/alice`
//! Daemon B: `agent:localhost/bob`
//!
//! T-04-20 mitigation: each daemon's keyring contains exactly one peer entry
//! (the other daemon) plus its own self-entry. Envelopes from any other
//! principal are rejected by `FampSigVerifyLayer` before the handler runs.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::conversation_harness::pubkey_b64;
use super::listen_harness::init_home_in_process;
use famp::cli::peer::add::run_add_at as peer_add_run_at;

/// Principal used for daemon A.
pub const ALICE_PRINCIPAL: &str = "agent:localhost/alice";
/// Principal used for daemon B.
pub const BOB_PRINCIPAL: &str = "agent:localhost/bob";

/// All handles and metadata for a live two-daemon setup.
pub struct TwoDaemons {
    pub a_home: TempDir,
    pub b_home: TempDir,
    pub a_addr: SocketAddr,
    pub b_addr: SocketAddr,
    pub a_handle: JoinHandle<()>,
    pub b_handle: JoinHandle<()>,
    pub a_shutdown: oneshot::Sender<()>,
    pub b_shutdown: oneshot::Sender<()>,
    pub a_principal: String,
    pub b_principal: String,
}

impl TwoDaemons {
    /// Signal shutdown to both daemons and await both join handles (2s timeout
    /// per daemon).
    pub async fn teardown(self) {
        let _ = self.a_shutdown.send(());
        let _ = self.b_shutdown.send(());
        let _ = tokio::time::timeout(Duration::from_secs(2), self.a_handle).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), self.b_handle).await;
    }
}

/// Write `config.toml` for `home` to set the daemon's self-principal.
///
/// `famp init` writes `listen_addr = "127.0.0.1:8443"`. We overwrite the file
/// with an additional `principal` line so `run_on_listener` picks it up.
fn write_config_principal(home: &Path, principal: &str) {
    let config_path = home.join("config.toml");
    let content = format!(
        "listen_addr = \"127.0.0.1:8443\"\nprincipal = \"{principal}\"\n"
    );
    std::fs::write(&config_path, content).expect("write config.toml");
}

/// Spawn a single daemon on a pre-bound listener and wait until it accepts
/// TCP connections.
async fn spawn_one(
    home: &Path,
    listener: std::net::TcpListener,
) -> (SocketAddr, JoinHandle<()>, oneshot::Sender<()>) {
    let addr: SocketAddr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let home_owned = home.to_path_buf();
    let handle = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(
            &home_owned,
            listener,
            async move {
                let _ = rx.await;
            },
        )
        .await
        .expect("run_on_listener");
    });

    // Wait until the daemon accepts TCP (up to 5s).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "daemon bind timed out at {addr}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    (addr, handle, tx)
}

/// Spawn two independent daemons with distinct principals and mutual peer
/// registration. Returns a [`TwoDaemons`] ready for E2E assertions.
///
/// ## Setup sequence
///
/// 1. Init both homes.
/// 2. Bind two ephemeral listeners (ports known before `peer_add`).
/// 3. Write distinct `config.toml` principals.
/// 4. Register B as a peer in A's home (with B's pubkey + `agent:localhost/bob`).
/// 5. Register A as a peer in B's home (with A's pubkey + `agent:localhost/alice`).
/// 6. Spawn both daemons.
pub async fn spawn_two_daemons() -> TwoDaemons {
    // 1. Create and init both FAMP_HOMEs.
    let a_home = TempDir::new().unwrap();
    let b_home = TempDir::new().unwrap();
    init_home_in_process(a_home.path());
    init_home_in_process(b_home.path());

    // 2. Bind ephemeral listeners now so both addrs are known before `peer_add`.
    let a_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    a_listener.set_nonblocking(true).unwrap();
    let a_addr: SocketAddr = a_listener.local_addr().unwrap();

    let b_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    b_listener.set_nonblocking(true).unwrap();
    let b_addr: SocketAddr = b_listener.local_addr().unwrap();

    // 3. Write distinct principals into each config.toml.
    write_config_principal(a_home.path(), ALICE_PRINCIPAL);
    write_config_principal(b_home.path(), BOB_PRINCIPAL);

    // 4. Mutual peer registration.
    //    A's home: register B as alias "bob" with B's pubkey + B's principal.
    let b_pubkey = pubkey_b64(b_home.path());
    peer_add_run_at(
        a_home.path(),
        "bob".to_string(),
        format!("https://{b_addr}"),
        b_pubkey.clone(),
        Some(BOB_PRINCIPAL.to_string()),
    )
    .expect("peer_add B into A");

    //    B's home: register A as alias "alice" with A's pubkey + A's principal.
    let a_pubkey = pubkey_b64(a_home.path());
    peer_add_run_at(
        b_home.path(),
        "alice".to_string(),
        format!("https://{a_addr}"),
        a_pubkey.clone(),
        Some(ALICE_PRINCIPAL.to_string()),
    )
    .expect("peer_add A into B");

    // 5. Spawn daemons.
    let (a_addr, a_handle, a_shutdown) = spawn_one(a_home.path(), a_listener).await;
    let (b_addr, b_handle, b_shutdown) = spawn_one(b_home.path(), b_listener).await;

    TwoDaemons {
        a_home,
        b_home,
        a_addr,
        b_addr,
        a_handle,
        b_handle,
        a_shutdown,
        b_shutdown,
        a_principal: ALICE_PRINCIPAL.to_string(),
        b_principal: BOB_PRINCIPAL.to_string(),
    }
}
