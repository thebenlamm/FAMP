//! TLS serve helper wrapping `axum_server::bind_rustls` / `from_tcp_rustls`.
//!
//! Isolates Plan 04-04's example binary from the axum-server API: the example
//! calls [`serve`] (or [`serve_std_listener`] for ephemeral-port subprocess
//! tests) and gets back a `JoinHandle<io::Result<()>>` it can store for
//! graceful shutdown on drop. Closes checker B-3 (no more `todo!()` for the
//! TLS bind in Plan 04-04).

use std::{net::SocketAddr, sync::Arc};

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use rustls::ServerConfig;
use tokio::task::JoinHandle;

/// Bind `addr` with the supplied rustls `ServerConfig` and spawn the server
/// on the current tokio runtime.
///
/// Use this when the caller knows the bind address up front. For ephemeral
/// `127.0.0.1:0` scenarios where the caller needs to read `local_addr()`
/// **before** spawning the server task, prefer [`serve_std_listener`] which
/// accepts a pre-bound `std::net::TcpListener`.
pub fn serve(
    addr: SocketAddr,
    router: Router,
    server_config: Arc<ServerConfig>,
) -> JoinHandle<std::io::Result<()>> {
    let rustls_config = RustlsConfig::from_config(server_config);
    tokio::spawn(async move {
        axum_server::bind_rustls(addr, rustls_config)
            .serve(router.into_make_service())
            .await
    })
}

/// Spawn an HTTPS server on a pre-bound `std::net::TcpListener`.
///
/// Use this for ephemeral-port scenarios (Plan 04-04 subprocess test) where
/// you bind the listener yourself, read `listener.local_addr()` to discover
/// the actual port, print it to stdout for the peer process, and *then* hand
/// the listener to the server.
///
/// The listener **must** already be in non-blocking mode before being passed
/// here. `run_on_listener` ensures this; `axum_server::from_tcp_rustls`
/// delegates to `tokio::net::TcpListener::from_std` which requires a
/// non-blocking socket (tokio-rs/tokio#7172 — registering a blocking socket
/// with the tokio runtime panics).
pub fn serve_std_listener(
    listener: std::net::TcpListener,
    router: Router,
    server_config: Arc<ServerConfig>,
) -> JoinHandle<std::io::Result<()>> {
    let rustls_config = RustlsConfig::from_config(server_config);
    tokio::spawn(async move {
        // `axum_server::from_tcp_rustls` delegates to
        // `tokio::net::TcpListener::from_std`, which panics if the socket is
        // in blocking mode. Do NOT call `set_nonblocking(false)` here.
        // The listener arrives already non-blocking from `run_on_listener`.
        let server = match axum_server::from_tcp_rustls(listener, rustls_config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        server.serve(router.into_make_service()).await
    })
}
