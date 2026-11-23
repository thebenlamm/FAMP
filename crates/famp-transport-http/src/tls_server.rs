//! TLS serve helpers wrapping `axum_server`, with slow-client and connection limits.
//!
//! Isolates callers from the axum-server API. [`serve`] and
//! [`serve_std_listener`] return a `JoinHandle<io::Result<()>>` the caller can
//! store for graceful shutdown on drop.
//!
//! Limits (see [`ServeLimits`]), all applied BEFORE any request is parsed:
//! - a cap on concurrent connections, overall and per source IP;
//! - the TLS handshake timeout (axum-server's own default is 10 s);
//! - HTTP/1 only, so hyper-util's protocol sniffing (which waits for the first
//!   bytes with no timer at all) never runs, plus an explicit hyper timer so
//!   `header_read_timeout` actually fires. Without a timer hyper silently drops
//!   its 30 s default (hyper-1.9.0 `common/time.rs`).
//!
//! HTTP/2 is not served: every FAMP client is HTTP/1 (`transport.rs` uses
//! `http1_only`) and the server config advertises no ALPN.

use std::collections::HashMap;
use std::future::{ready, Ready};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::Router;
use axum_server::accept::Accept;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use hyper_util::rt::TokioTimer;
use rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

/// Slow-client and connection limits for [`serve_std_listener_with_limits`].
#[derive(Clone, Copy, Debug)]
pub struct ServeLimits {
    /// Max time to receive a complete HTTP/1 request head after the handshake.
    pub header_read_timeout: Duration,
    /// Max time for the TLS handshake.
    pub handshake_timeout: Duration,
    /// Max concurrent connections across all sources. Keep it well below the
    /// process file-descriptor limit (a launchd job's default soft limit is low).
    pub max_connections: usize,
    /// Max concurrent connections from one source IP, so one host cannot hold
    /// every slot and lock the pinned peers out.
    pub max_per_ip: usize,
}

impl Default for ServeLimits {
    fn default() -> Self {
        Self {
            header_read_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(10),
            max_connections: 128,
            max_per_ip: 16,
        }
    }
}

#[derive(Debug)]
struct Slots {
    total: usize,
    per_ip: HashMap<IpAddr, usize>,
}

/// Releases a connection slot when the connection (and its stream) drops.
#[derive(Debug)]
struct SlotGuard {
    slots: Arc<Mutex<Slots>>,
    ip: IpAddr,
}

impl Drop for SlotGuard {
    fn drop(&mut self) {
        if let Ok(mut slots) = self.slots.lock() {
            slots.total = slots.total.saturating_sub(1);
            if let Some(n) = slots.per_ip.get_mut(&self.ip) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    slots.per_ip.remove(&self.ip);
                }
            }
        }
    }
}

/// Acceptor that refuses connections past [`ServeLimits`]; wraps the raw TCP
/// stream so TLS runs on top of it and the slot is held for the connection's life.
#[derive(Clone, Debug)]
pub struct ConnLimitAcceptor {
    slots: Arc<Mutex<Slots>>,
    max_connections: usize,
    max_per_ip: usize,
}

impl ConnLimitAcceptor {
    #[must_use]
    pub fn new(limits: &ServeLimits) -> Self {
        Self {
            slots: Arc::new(Mutex::new(Slots {
                total: 0,
                per_ip: HashMap::new(),
            })),
            max_connections: limits.max_connections,
            max_per_ip: limits.max_per_ip,
        }
    }

    fn acquire(&self, ip: IpAddr) -> Option<SlotGuard> {
        // IPv4-mapped IPv6 is unmapped; other IPv6 is grouped by /64, matching the
        // gateway's per-source request budget, so one host cannot multiply its
        // allowance by rotating through addresses in its own prefix.
        let ip = match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or_else(
                || {
                    let [a, b, c, d, ..] = v6.segments();
                    IpAddr::V6(std::net::Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
                },
                IpAddr::V4,
            ),
            v4 @ IpAddr::V4(_) => v4,
        };
        let mut slots = self.slots.lock().ok()?;
        let from_ip = slots.per_ip.get(&ip).copied().unwrap_or(0);
        if slots.total >= self.max_connections || from_ip >= self.max_per_ip {
            return None;
        }
        slots.total += 1;
        *slots.per_ip.entry(ip).or_insert(0) += 1;
        drop(slots);
        Some(SlotGuard {
            slots: Arc::clone(&self.slots),
            ip,
        })
    }
}

/// A TCP stream that owns a connection slot.
#[derive(Debug)]
pub struct LimitedStream {
    inner: TcpStream,
    _slot: SlotGuard,
}

impl AsyncRead for LimitedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for LimitedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> Accept<TcpStream, S> for ConnLimitAcceptor {
    type Stream = LimitedStream;
    type Service = S;
    type Future = Ready<io::Result<(LimitedStream, S)>>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let result = stream
            .peer_addr()
            .map_err(|e| io::Error::new(e.kind(), format!("peer address unavailable: {e}")))
            .and_then(|peer| {
                self.acquire(peer.ip())
                    .map(|slot| LimitedStream {
                        inner: stream,
                        _slot: slot,
                    })
                    .ok_or_else(|| io::Error::other("connection limit reached"))
            })
            .map(|stream| (stream, service));
        ready(result)
    }
}

/// Bind `addr` with the supplied rustls `ServerConfig` and spawn the server
/// on the current tokio runtime, with default [`ServeLimits`].
///
/// Use this when the caller knows the bind address up front. For ephemeral
/// `127.0.0.1:0` scenarios where the caller needs to read `local_addr()`
/// **before** spawning the server task, prefer [`serve_std_listener`] which
/// accepts a pre-bound `std::net::TcpListener`.
pub fn serve(
    addr: SocketAddr,
    router: Router,
    server_config: Arc<ServerConfig>,
) -> JoinHandle<io::Result<()>> {
    match std::net::TcpListener::bind(addr).and_then(|l| l.set_nonblocking(true).map(|()| l)) {
        Ok(listener) => serve_std_listener(listener, router, server_config),
        Err(e) => tokio::spawn(async move { Err(e) }),
    }
}

/// Spawn an HTTPS server on a pre-bound `std::net::TcpListener`, with the
/// default [`ServeLimits`].
///
/// Use this for ephemeral-port scenarios (Plan 04-04 subprocess test) where
/// you bind the listener yourself, read `listener.local_addr()` to discover
/// the actual port, print it to stdout for the peer process, and *then* hand
/// the listener to the server.
///
/// The listener **must** already be in non-blocking mode before being passed
/// here (tokio-rs/tokio#7172: registering a blocking socket panics).
pub fn serve_std_listener(
    listener: std::net::TcpListener,
    router: Router,
    server_config: Arc<ServerConfig>,
) -> JoinHandle<io::Result<()>> {
    serve_std_listener_with_limits(listener, router, server_config, ServeLimits::default())
}

/// [`serve_std_listener`] with explicit [`ServeLimits`].
pub fn serve_std_listener_with_limits(
    listener: std::net::TcpListener,
    router: Router,
    server_config: Arc<ServerConfig>,
    limits: ServeLimits,
) -> JoinHandle<io::Result<()>> {
    let rustls_config = RustlsConfig::from_config(server_config);
    tokio::spawn(async move {
        let acceptor = RustlsAcceptor::new(rustls_config)
            .handshake_timeout(limits.handshake_timeout)
            .acceptor(ConnLimitAcceptor::new(&limits));
        let mut server = axum_server::from_tcp(listener)?
            .acceptor(acceptor)
            .http1_only();
        // The timer MUST be set before header_read_timeout, or hyper panics on
        // every connection ("timeout `header_read_timeout` set, but no timer set").
        server
            .http_builder()
            .http1()
            .timer(TokioTimer::new())
            .header_read_timeout(limits.header_read_timeout)
            // One request per connection: otherwise a client could hold a
            // connection slot indefinitely by sending one cheap request just
            // inside each header timeout. FAMP clients send one POST at a time.
            .keep_alive(false);
        server
            .serve(router.into_make_service_with_connect_info::<SocketAddr>())
            .await
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
mod tests {
    use std::io::Read as _;
    use std::time::Instant;

    use axum::routing::post;
    use rustls::pki_types::ServerName;

    use super::*;

    /// A throwaway end-entity cert for 127.0.0.1, made with the openssl CLI.
    fn test_server_config(dir: &std::path::Path) -> (Arc<ServerConfig>, std::path::PathBuf) {
        let (crt, key) = (dir.join("c.pem"), dir.join("k.pem"));
        let out = std::process::Command::new("openssl")
            .args([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
            ])
            .args(["-subj", "/CN=test"])
            .args(["-addext", "subjectAltName=IP:127.0.0.1"])
            .args(["-addext", "basicConstraints=critical,CA:FALSE"])
            .args(["-addext", "extendedKeyUsage=serverAuth"])
            .arg("-keyout")
            .arg(&key)
            .arg("-out")
            .arg(&crt)
            .output()
            .unwrap();
        assert!(out.status.success(), "openssl failed");
        let cert = crate::tls::load_pem_cert(&crt).unwrap();
        let key = crate::tls::load_pem_key(&key).unwrap();
        (
            Arc::new(crate::tls::build_server_config(cert, key).unwrap()),
            crt,
        )
    }

    fn start(limits: ServeLimits) -> (SocketAddr, std::path::PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (cfg, crt) = test_server_config(dir.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let router = Router::new().route("/", post(|| async { "ok" }));
        // Detach: the server lives as long as the test runtime.
        drop(serve_std_listener_with_limits(
            listener, router, cfg, limits,
        ));
        (addr, crt, dir)
    }

    /// Open a TCP connection and finish the TLS handshake, then send nothing.
    fn handshake_then_silence(
        addr: SocketAddr,
        crt: &std::path::Path,
    ) -> rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream> {
        let tls = crate::tls::build_client_config(Some(crt)).unwrap();
        let name = ServerName::IpAddress(addr.ip().into());
        let mut conn = rustls::ClientConnection::new(Arc::new(tls), name).unwrap();
        let mut sock = std::net::TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        while conn.is_handshaking() {
            conn.complete_io(&mut sock).unwrap();
        }
        rustls::StreamOwned::new(conn, sock)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_client_that_finishes_tls_and_sends_nothing_is_cut_off() {
        let (addr, crt, _dir) = start(ServeLimits {
            header_read_timeout: Duration::from_secs(1),
            ..ServeLimits::default()
        });
        let elapsed = tokio::task::spawn_blocking(move || {
            let mut stream = handshake_then_silence(addr, &crt);
            let started = Instant::now();
            let mut buf = [0u8; 16];
            let result = stream.read(&mut buf);
            // Closed by the server (EOF or reset), NOT our own 8 s read timeout.
            if let Err(e) = &result {
                assert!(
                    !matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ),
                    "server never closed the idle connection: {e}"
                );
            }
            started.elapsed()
        })
        .await
        .unwrap();
        assert!(
            elapsed >= Duration::from_millis(700) && elapsed < Duration::from_secs(5),
            "cut-off took {elapsed:?}; expected about the 1 s header timeout"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_connection_closes_after_one_request_so_it_cannot_hold_a_slot() {
        // A long header timeout, so only keep-alive being off can close it promptly.
        let (addr, crt, _dir) = start(ServeLimits {
            header_read_timeout: Duration::from_secs(30),
            ..ServeLimits::default()
        });
        let elapsed = tokio::task::spawn_blocking(move || {
            use std::io::Write as _;
            let mut stream = handshake_then_silence(addr, &crt);
            stream
                .write_all(b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let started = Instant::now();
            let mut all = Vec::new();
            // read_to_end returns only at EOF: the server must close after answering.
            let result = stream.read_to_end(&mut all);
            assert!(
                !matches!(&result, Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)),
                "server kept the connection open: {result:?}"
            );
            assert!(String::from_utf8_lossy(&all).starts_with("HTTP/1.1 200"));
            started.elapsed()
        })
        .await
        .unwrap();
        assert!(elapsed < Duration::from_secs(5), "closed after {elapsed:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connections_over_the_per_ip_cap_are_refused_and_slots_are_released() {
        let (addr, _crt, _dir) = start(ServeLimits {
            handshake_timeout: Duration::from_secs(30),
            max_connections: 100,
            max_per_ip: 2,
            ..ServeLimits::default()
        });
        tokio::task::spawn_blocking(move || {
            let short = Some(Duration::from_millis(400));
            let alive = |s: &mut std::net::TcpStream| {
                s.set_read_timeout(short).unwrap();
                let mut b = [0u8; 1];
                matches!(
                    s.read(&mut b),
                    Err(ref e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
                )
            };
            let mut first = std::net::TcpStream::connect(addr).unwrap();
            let mut second = std::net::TcpStream::connect(addr).unwrap();
            // Let the server's accept tasks take both slots.
            std::thread::sleep(Duration::from_millis(300));
            assert!(alive(&mut first) && alive(&mut second), "held slots");
            let mut third = std::net::TcpStream::connect(addr).unwrap();
            third.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            let mut b = [0u8; 1];
            let refused = third.read(&mut b);
            assert!(
                matches!(refused, Ok(0))
                    || matches!(&refused, Err(e) if !matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)),
                "third connection must be closed by the server, got {refused:?}"
            );
            // Control: the limit is not permanent. Free a slot and a new one is kept.
            drop(first);
            let started = Instant::now();
            loop {
                let mut fourth = std::net::TcpStream::connect(addr).unwrap();
                std::thread::sleep(Duration::from_millis(200));
                if alive(&mut fourth) {
                    break;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "slot was never released"
                );
            }
        })
        .await
        .unwrap();
    }

    fn openssl_ok(args: &[&std::ffi::OsStr]) {
        let out = std::process::Command::new("openssl")
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "openssl {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A CA (CA:TRUE) and a leaf for 127.0.0.1 signed by it. Returns (ca.pem, leaf.pem, leaf.key).
    fn ca_signed_leaf(
        dir: &std::path::Path,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        use std::ffi::OsStr;
        let p = |n: &str| dir.join(n);
        openssl_ok(&[
            "req".as_ref(),
            "-x509".as_ref(),
            "-newkey".as_ref(),
            "rsa:2048".as_ref(),
            "-nodes".as_ref(),
            "-days".as_ref(),
            "2".as_ref(),
            "-subj".as_ref(),
            "/CN=test-ca".as_ref(),
            "-addext".as_ref(),
            "basicConstraints=critical,CA:TRUE".as_ref(),
            "-keyout".as_ref(),
            p("ca.key").as_os_str(),
            "-out".as_ref(),
            p("ca.pem").as_os_str(),
        ]);
        openssl_ok(&[
            "req".as_ref(),
            "-newkey".as_ref(),
            "rsa:2048".as_ref(),
            "-nodes".as_ref(),
            "-subj".as_ref(),
            "/CN=leaf".as_ref(),
            "-keyout".as_ref(),
            p("leaf.key").as_os_str(),
            "-out".as_ref(),
            p("leaf.csr").as_os_str(),
        ]);
        std::fs::write(
            p("ext.cnf"),
            "subjectAltName=IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n",
        )
        .unwrap();
        let _: &OsStr = "".as_ref();
        openssl_ok(&[
            "x509".as_ref(),
            "-req".as_ref(),
            "-in".as_ref(),
            p("leaf.csr").as_os_str(),
            "-CA".as_ref(),
            p("ca.pem").as_os_str(),
            "-CAkey".as_ref(),
            p("ca.key").as_os_str(),
            "-CAcreateserial".as_ref(),
            "-days".as_ref(),
            "2".as_ref(),
            "-extfile".as_ref(),
            p("ext.cnf").as_os_str(),
            "-out".as_ref(),
            p("leaf.pem").as_os_str(),
        ]);
        (p("ca.pem"), p("leaf.pem"), p("leaf.key"))
    }

    fn serve_cert(crt: &std::path::Path, key: &std::path::Path) -> SocketAddr {
        let cert = crate::tls::load_pem_cert(crt).unwrap();
        let key = crate::tls::load_pem_key(key).unwrap();
        let cfg = Arc::new(crate::tls::build_server_config(cert, key).unwrap());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let router = Router::new().route("/", post(|| async { "ok" }));
        drop(serve_std_listener_with_limits(
            listener,
            router,
            cfg,
            ServeLimits::default(),
        ));
        addr
    }

    fn handshake_ok(addr: SocketAddr, tls: rustls::ClientConfig) -> bool {
        let name = ServerName::IpAddress(addr.ip().into());
        let mut conn = rustls::ClientConnection::new(Arc::new(tls), name).unwrap();
        let mut sock = std::net::TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        while conn.is_handshaking() {
            if conn.complete_io(&mut sock).is_err() {
                return false;
            }
        }
        true
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pinned_client_trusts_only_the_exact_pinned_certificate() {
        let dir = tempfile::tempdir().unwrap();
        let sub = |n: &str| {
            let d = dir.path().join(n);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        // Two unrelated self-signed leaves for the SAME address, and a CA-issued leaf.
        let (cfg_a, crt_a) = test_server_config(&sub("a"));
        let (_cfg_b, crt_b) = test_server_config(&sub("b"));
        drop(cfg_a);
        let (ca, leaf, leaf_key) = ca_signed_leaf(&sub("ca"));
        let addr_a = serve_cert(&crt_a, &sub("a").join("k.pem"));
        let addr_b = serve_cert(&crt_b, &sub("b").join("k.pem"));
        let addr_leaf = serve_cert(&leaf, &leaf_key);

        tokio::task::spawn_blocking(move || {
            use crate::tls::{build_client_config, build_pinned_client_config};
            // The pinned certificate is accepted.
            assert!(handshake_ok(
                addr_a,
                build_pinned_client_config(&crt_a).unwrap()
            ));
            // A DIFFERENT self-signed certificate for the same address is rejected...
            assert!(!handshake_ok(
                addr_b,
                build_pinned_client_config(&crt_a).unwrap()
            ));
            // ...although the old verifier accepts it once it is an extra root (control).
            assert!(handshake_ok(
                addr_b,
                build_client_config(Some(&crt_b)).unwrap()
            ));
            // Pinning only a CA does not let that CA's other certificates in...
            assert!(!handshake_ok(
                addr_leaf,
                build_pinned_client_config(&ca).unwrap()
            ));
            // ...although the old verifier trusts everything under an extra-root CA (control).
            assert!(handshake_ok(
                addr_leaf,
                build_client_config(Some(&ca)).unwrap()
            ));
            // A CA-issued leaf is pinned by listing it WITH its issuer (webpki needs the
            // issuer in the store); only the leaf's exact bytes are then accepted.
            let both = leaf.with_file_name("leaf_and_ca.pem");
            let mut pem = std::fs::read_to_string(&leaf).unwrap();
            pem.push_str(&std::fs::read_to_string(&ca).unwrap());
            std::fs::write(&both, pem).unwrap();
            assert!(handshake_ok(
                addr_leaf,
                build_pinned_client_config(&both).unwrap()
            ));
        })
        .await
        .unwrap();
    }

    /// A throwaway certificate for `sans` (e.g. `IP:127.0.0.1,IP:127.0.0.2`), CA:FALSE.
    fn cert_with_sans(
        dir: &std::path::Path,
        sans: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let (crt, key) = (dir.join("s.pem"), dir.join("s.key"));
        let out = std::process::Command::new("openssl")
            .args([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
            ])
            .args(["-subj", "/CN=sans"])
            .args(["-addext", &format!("subjectAltName={sans}")])
            .args(["-addext", "basicConstraints=critical,CA:FALSE"])
            .args(["-addext", "extendedKeyUsage=serverAuth"])
            .arg("-keyout")
            .arg(&key)
            .arg("-out")
            .arg(&crt)
            .output()
            .unwrap();
        assert!(out.status.success());
        (crt, key)
    }

    /// Dial `addr` but verify the server as if it were `as_name` (what a client does
    /// when it believes it is talking to a different peer).
    fn handshake_as(
        addr: SocketAddr,
        as_name: std::net::IpAddr,
        tls: rustls::ClientConfig,
    ) -> bool {
        let name = ServerName::IpAddress(as_name.into());
        let mut conn = rustls::ClientConnection::new(Arc::new(tls), name).unwrap();
        let mut sock = std::net::TcpStream::connect(addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        while conn.is_handshaking() {
            if conn.complete_io(&mut sock).is_err() {
                return false;
            }
        }
        true
    }

    /// Peer A's certificate presented on peer B's address must be rejected, even when
    /// A's certificate is pinned and ALSO lists B's address: a pinned certificate is
    /// good for exactly the one IP it declares.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_pinned_certificate_is_valid_only_for_its_own_single_address() {
        let dir = tempfile::tempdir().unwrap();
        let sub = |n: &str| {
            let d = dir.path().join(n);
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let (plain, plain_key) = cert_with_sans(&sub("plain"), "IP:127.0.0.1");
        let (greedy, greedy_key) = cert_with_sans(&sub("greedy"), "IP:127.0.0.1,IP:127.0.0.2");
        let addr_plain = serve_cert(&plain, &plain_key);
        let addr_greedy = serve_cert(&greedy, &greedy_key);
        let a: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        let b: std::net::IpAddr = "127.0.0.2".parse().unwrap();
        tokio::task::spawn_blocking(move || {
            use crate::tls::{build_client_config, build_pinned_client_config};
            // Control: A's single-address certificate works on A's own address...
            assert!(handshake_as(
                addr_plain,
                a,
                build_pinned_client_config(&plain).unwrap()
            ));
            // ...and is rejected when presented on B's address.
            assert!(!handshake_as(
                addr_plain,
                b,
                build_pinned_client_config(&plain).unwrap()
            ));
            // A certificate that lists B's address too is rejected on B's address, and
            // also on its own (it declares more than one address)...
            assert!(!handshake_as(
                addr_greedy,
                b,
                build_pinned_client_config(&greedy).unwrap()
            ));
            assert!(!handshake_as(
                addr_greedy,
                a,
                build_pinned_client_config(&greedy).unwrap()
            ));
            // ...although the unrestricted client would have accepted it on B's address
            // (this is what a flat pin plus webpki alone allowed).
            assert!(handshake_as(
                addr_greedy,
                b,
                build_client_config(Some(&greedy)).unwrap()
            ));
        })
        .await
        .unwrap();
    }

    /// A pinned-only client must never send to a plain `http://` peer: the URL would
    /// bypass the pinned verifier entirely and the message would travel in clear.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_pinned_client_never_dials_a_plain_http_peer() {
        use famp_transport::{Transport, TransportMessage};
        let dir = tempfile::tempdir().unwrap();
        let (_cfg, crt) = test_server_config(dir.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = url::Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let from: famp_core::Principal = "agent:a.test/alice".parse().unwrap();
        let to: famp_core::Principal = "agent:b.test/bob".parse().unwrap();
        let msg = || TransportMessage {
            sender: from.clone(),
            recipient: to.clone(),
            bytes: b"{}".to_vec(),
        };

        let pinned = crate::transport::HttpTransport::new_client_only_pinned(&crt).unwrap();
        pinned.add_peer(to.clone(), url.clone()).await;
        assert!(pinned.send(msg()).await.is_err());
        assert!(
            matches!(listener.accept(), Err(e) if e.kind() == io::ErrorKind::WouldBlock),
            "the pinned client must not even connect to a plain http peer"
        );

        // Control: the unrestricted client DOES dial it, so the check above can fail.
        let open = crate::transport::HttpTransport::new_client_only(None).unwrap();
        open.add_peer(to.clone(), url).await;
        let _ = tokio::time::timeout(Duration::from_millis(500), open.send(msg())).await;
        assert!(
            listener.accept().is_ok(),
            "control: the open client connects"
        );
    }
}
