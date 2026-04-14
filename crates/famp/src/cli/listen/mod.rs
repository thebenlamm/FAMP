//! `famp listen` subcommand — Phase 2 Plan 02-02.
//!
//! Wires Phase 1 identity loading, `famp-inbox` durable append, and
//! `famp-transport-http`'s signature-verification middleware into a single
//! axum router served over TLS. Graceful shutdown on SIGINT / SIGTERM.
//!
//! NOTE on shutdown semantics: the current `tls_server::serve_std_listener`
//! helper returns a `JoinHandle<io::Result<()>>` with no graceful-shutdown
//! handle exposed. On shutdown signal we therefore drop the `JoinHandle` —
//! `axum-server` stops accepting new connections when its future is dropped.
//! In-flight handlers that have already completed `inbox.append` (fsync) and
//! returned 200 are fine; handlers still mid-fsync have NOT yet reported 200
//! to the client, so the client sees a dropped connection. This is weaker
//! than an ideal flush-then-exit, but the durability invariant (INBOX-02)
//! still holds because `append` fsyncs before returning Ok. Plan 02-03 adds
//! a SIGINT durability test that locks this contract.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cli::config::Config;
use crate::cli::error::CliError;
use crate::cli::init::load_identity;

pub mod router;
pub mod signal;

/// Arguments to `famp listen`.
#[derive(clap::Args, Debug, Clone)]
pub struct ListenArgs {
    /// Override `config.listen_addr`. Format: `127.0.0.1:8443`.
    #[arg(long)]
    pub listen: Option<SocketAddr>,
}

/// Production entry point for `famp listen`.
///
/// Resolves `FAMP_HOME`, binds the listener from `config.listen_addr` (or
/// `--listen` override), prints the bound address to stderr, then hands
/// control to [`run_on_listener`] with the SIGINT/SIGTERM signal future.
#[allow(clippy::needless_pass_by_value)]
pub async fn run(args: ListenArgs) -> Result<(), CliError> {
    let home = crate::cli::home::resolve_famp_home()?;
    // Best-effort config parse for the default listen addr. If the config
    // file is missing or malformed, fall through — `load_identity` in
    // `run_on_listener` will surface a typed error for the missing file.
    let config_path = home.join(crate::cli::paths::CONFIG_TOML);
    let default_cfg = Config::default();
    let cfg: Config = match std::fs::read_to_string(&config_path) {
        Ok(s) => toml::from_str(&s).map_err(|e| CliError::TomlParse {
            path: config_path.clone(),
            source: e,
        })?,
        Err(_) => default_cfg,
    };

    let addr = args.listen.unwrap_or(cfg.listen_addr);
    let listener =
        std::net::TcpListener::bind(addr).map_err(|e| match e.kind() {
            std::io::ErrorKind::AddrInUse => CliError::PortInUse { addr },
            _ => CliError::Io {
                path: home.clone(),
                source: e,
            },
        })?;
    // axum-server 0.8 refuses to register a blocking socket with tokio
    // (see tokio-rs/tokio#7172). `std::net::TcpListener::bind` returns a
    // blocking socket, so flip it here before handing off.
    listener.set_nonblocking(true).map_err(|e| CliError::Io {
        path: home.clone(),
        source: e,
    })?;
    let bound = listener.local_addr().map_err(|e| CliError::Io {
        path: home.clone(),
        source: e,
    })?;
    eprintln!("listening on https://{bound}");

    run_on_listener(&home, listener, signal::shutdown_signal()).await
}

/// Test-facing entry point. Takes a pre-bound listener so integration tests
/// can use `127.0.0.1:0` for ephemeral ports and read `local_addr()` before
/// handing control to the server.
///
/// Shutdown resolves when `shutdown_signal` completes (SIGINT/SIGTERM in
/// production, an oneshot/mpsc in tests).
pub async fn run_on_listener(
    home: &Path,
    listener: std::net::TcpListener,
    shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), CliError> {
    // Ensure the listener is non-blocking regardless of caller path.
    // axum-server 0.8 panics if handed a blocking socket (tokio-rs/tokio#7172).
    listener.set_nonblocking(true).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;

    let layout = load_identity(home)?;

    // Load config (for future use — currently unused past bind, but parsing
    // it here means a malformed config after init fails loudly).
    let cfg_bytes = std::fs::read_to_string(&layout.config_toml).map_err(|e| CliError::Io {
        path: layout.config_toml.clone(),
        source: e,
    })?;
    let _cfg: Config = toml::from_str(&cfg_bytes).map_err(|e| CliError::TomlParse {
        path: layout.config_toml.clone(),
        source: e,
    })?;

    // Load the daemon's own signing key (raw 32-byte seed on disk).
    let seed_bytes = std::fs::read(&layout.key_ed25519).map_err(|e| CliError::Io {
        path: layout.key_ed25519.clone(),
        source: e,
    })?;
    let seed: [u8; 32] =
        <[u8; 32]>::try_from(seed_bytes.as_slice()).map_err(|_| CliError::Io {
            path: layout.key_ed25519.clone(),
            source: std::io::Error::other("key.ed25519 is not 32 bytes"),
        })?;
    let sk = famp_crypto::FampSigningKey::from_bytes(seed);
    let vk = sk.verifying_key();
    // Explicit drop of the seed bytes (no zeroization — Copy semantics
    // for the [u8; 32], the Vec is dropped here).
    drop(seed_bytes);

    // Build the self-keyring: a single entry (self-principal → own vk).
    // Phase 2 scope — peer keys land in Phase 3. Plan 02-03's integration
    // tests will sign with this same self-principal so the sig-verify
    // middleware can resolve them.
    let self_principal: famp_core::Principal = "agent:localhost/self"
        .parse()
        .map_err(|e: famp_core::ParsePrincipalError| CliError::Io {
            path: PathBuf::new(),
            source: std::io::Error::other(format!("self principal parse: {e}")),
        })?;
    let keyring = famp_keyring::Keyring::new()
        .with_peer(self_principal, vk)
        .map_err(|e| CliError::Io {
            path: PathBuf::new(),
            source: std::io::Error::other(format!("self keyring build: {e}")),
        })?;
    let keyring = Arc::new(keyring);

    // Open the inbox (creates the 0600 file on first call).
    let inbox_path = layout.home.join("inbox.jsonl");
    let inbox = Arc::new(famp_inbox::Inbox::open(&inbox_path).await?);

    // Load TLS material and build the server config.
    let certs = famp_transport_http::tls::load_pem_cert(&layout.tls_cert_pem)?;
    let key = famp_transport_http::tls::load_pem_key(&layout.tls_key_pem)?;
    let server_config = Arc::new(famp_transport_http::tls::build_server_config(certs, key)?);

    // Build the router (reuses FampSigVerifyLayer unmodified; custom handler
    // that appends to the inbox before returning 200).
    let router = router::build_listen_router(keyring, inbox);

    // Spawn the TLS server on the pre-bound listener.
    let join = famp_transport_http::tls_server::serve_std_listener(listener, router, server_config);

    tokio::select! {
        res = join => {
            match res {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(CliError::Io {
                    path: home.to_path_buf(),
                    source: e,
                }),
                Err(join_err) => Err(CliError::Io {
                    path: home.to_path_buf(),
                    source: std::io::Error::other(format!("server task panicked: {join_err}")),
                }),
            }
        }
        () = shutdown_signal => {
            eprintln!("shutdown signal received, exiting");
            Ok(())
        }
    }
}
