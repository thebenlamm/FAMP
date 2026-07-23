//! Async UDS `BusClient` — Phase 02.
//!
//! Opens a Unix domain socket, performs the BUS-06 Hello handshake, and
//! exposes a request-reply `send_recv` shape every CLI subcommand and
//! the MCP session reuses. The constructor `connect(sock, bind_as)`
//! takes an optional D-10 proxy identity:
//!
//! - `bind_as = None` is the canonical-holder shape used by `famp register`
//!   and the long-lived MCP server. The connection becomes its own slot
//!   when (and only when) it later sends `BusMessage::Register`.
//! - `bind_as = Some(name)` is the proxy shape used by every one-shot
//!   CLI subcommand (`send`, `inbox list/ack`, `await`, `join`, `leave`,
//!   `sessions --me`, `whoami`). The broker validates that `name` maps
//!   to a live registered holder at Hello time and rejects otherwise
//!   with `HelloErr { kind: NotRegistered }`.
//!
//! NOTE on the wire shape: the `bind_as` field on `BusMessage::Hello`
//! is added in plan 02-02 (per D-10). When 02-01 lands first temporally
//! the parameter is accepted by `connect` but NOT yet serialized into
//! the Hello frame; `Some(name)` is held in the client and surfaced via
//! `BusClient::bind_as()` so 02-02 can back-fill the wire field without
//! changing the public CLI-side API.

use std::path::{Path, PathBuf};

use famp_bus::{BusErrorKind, BusMessage, BusReply, BUS_PROTO_VERSION};
use tokio::io::AsyncWriteExt as _;
use tokio::net::UnixStream;

pub mod codec;
pub mod spawn;

/// Active connection to the local broker. Holds the open `UnixStream`
/// and the optional D-10 proxy identity supplied at `connect` time.
pub struct BusClient {
    stream: UnixStream,
    bind_as: Option<String>,
}

/// Errors produced by `BusClient` operations.
#[derive(Debug, thiserror::Error)]
pub enum BusClientError {
    #[error("io error talking to broker")]
    Io(#[source] std::io::Error),
    #[error("frame codec error")]
    Frame(#[source] famp_bus::codec::FrameError),
    #[error("canonical-JSON strict-parse failed")]
    Decode(#[source] famp_canonical::CanonicalError),
    #[error("Hello handshake refused: {kind:?}: {message}")]
    HelloFailed { kind: BusErrorKind, message: String },
    /// Bus protocol version mismatch. The daemon is running an old protocol
    /// version. D-02: error MUST name `famp daemon restart`.
    #[error(
        "bus protocol mismatch ({broker_message}); \
         run `famp daemon restart` to pick up the new binary"
    )]
    ProtocolMismatch { broker_message: String },
    #[error("unexpected broker reply: {0}")]
    UnexpectedReply(String),
    #[error("broker did not start")]
    BrokerDidNotStart(#[source] spawn::SpawnError),
}

/// Classify a Hello reply into a `BusClient` result.
///
/// Extracted as a pure free function so tests can exercise the match-arm
/// logic without a live broker socket. Called by `BusClient::connect`.
///
/// - `HelloOk` → logs the CLIENT build version (D-03 Decision B: daemon build
///   surfaced separately via `famp daemon status`/`InspectBrokerReply.build_version`)
///   and returns `Ok(())`.
/// - `HelloErr { BrokerProtoMismatch }` → returns `ProtocolMismatch` naming
///   `famp daemon restart` (D-01/D-02).
/// - Any other `HelloErr` or `Err` → returns `HelloFailed`.
/// - Anything else → returns `UnexpectedReply`.
fn classify_hello_reply(reply: BusReply) -> Result<(), BusClientError> {
    match reply {
        BusReply::HelloOk { .. } => {
            // D-03 Decision B: log the CLIENT build only. Daemon build is
            // surfaced via `famp daemon status` (InspectBrokerReply.build_version).
            // No connect-time Inspect round-trip; no HelloOk wire field added.
            eprintln!(
                "famp: connected to broker (client_build={}, bus_proto={BUS_PROTO_VERSION})",
                env!("CARGO_PKG_VERSION")
            );
            Ok(())
        }
        BusReply::HelloErr {
            kind: BusErrorKind::BrokerProtoMismatch,
            message,
        } => Err(BusClientError::ProtocolMismatch {
            broker_message: message,
        }),
        BusReply::HelloErr { kind, message } | BusReply::Err { kind, message } => {
            Err(BusClientError::HelloFailed { kind, message })
        }
        other => Err(BusClientError::UnexpectedReply(format!("{other:?}"))),
    }
}

impl BusClient {
    /// Connect to `sock_path`, performing the Hello handshake. If the
    /// broker is not running, spawn it via `spawn::spawn_broker_if_absent`
    /// (the locked Q1 portable pattern).
    ///
    /// `bind_as = None` → canonical-holder shape (register, MCP).
    /// `bind_as = Some(name)` → D-10 proxy shape (one-shot CLI commands).
    ///
    /// Retries connect up to 20×100ms (2s total) to ride out the broker
    /// spawn race; the spawn helper itself polls 10×200ms but a freshly
    /// spawned broker can take additional time to bind on slow CI hosts.
    pub async fn connect(
        sock_path: &Path,
        bind_as: Option<String>,
    ) -> Result<Self, BusClientError> {
        spawn::spawn_broker_if_absent(sock_path).map_err(BusClientError::BrokerDidNotStart)?;

        // Retry connect to ride out the spawn-then-bind race. Both
        // NotFound and ConnectionRefused are valid retry triggers.
        let stream = {
            let mut attempts: u8 = 0;
            loop {
                match UnixStream::connect(sock_path).await {
                    Ok(s) => break s,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        ) =>
                    {
                        attempts += 1;
                        if attempts >= 20 {
                            return Err(BusClientError::Io(e));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => return Err(BusClientError::Io(e)),
                }
            }
        };

        let mut client = Self { stream, bind_as };
        // D-10 (back-filled in plan 02-02): forward `bind_as` on the
        // Hello frame. The broker validates `bind_as = Some(holder)`
        // maps to a live registered holder and rejects with
        // `HelloErr { NotRegistered }` if not.
        let hello = BusMessage::Hello {
            bus_proto: BUS_PROTO_VERSION,
            client: format!("famp-cli/{}", env!("CARGO_PKG_VERSION")),
            bind_as: client.bind_as.clone(),
        };
        let reply = client.send_recv(hello).await?;
        classify_hello_reply(reply).map(|()| client)
    }

    /// Connect to `sock_path` performing the Hello handshake, WITHOUT
    /// spawning a broker if one is absent. Byte-for-byte the same
    /// handshake as [`connect`](Self::connect), minus the
    /// `spawn::spawn_broker_if_absent` call and its retry-to-ride-out-the-
    /// spawn-race loop.
    ///
    /// A long-running Layer-2 service (`famp-gateway`) must fail loud
    /// when the v0.11 daemon is down, not auto-spawn one — auto-spawn is
    /// a CLI/MCP convenience for interactive, session-scoped use, not
    /// appropriate for a service process that outlives any one caller
    /// (07-RESEARCH.md Anti-Patterns: "Gateway auto-spawning a broker").
    /// A single connect attempt that fails with `NotFound` or
    /// `ConnectionRefused` returns `BusClientError::Io` immediately —
    /// there is no spawn race to ride out here since nothing spawned
    /// anything.
    pub async fn connect_no_spawn(
        sock_path: &Path,
        bind_as: Option<String>,
    ) -> Result<Self, BusClientError> {
        let stream = UnixStream::connect(sock_path)
            .await
            .map_err(BusClientError::Io)?;

        let mut client = Self { stream, bind_as };
        let hello = BusMessage::Hello {
            bus_proto: BUS_PROTO_VERSION,
            client: format!("famp-gateway/{}", env!("CARGO_PKG_VERSION")),
            bind_as: client.bind_as.clone(),
        };
        let reply = client.send_recv(hello).await?;
        classify_hello_reply(reply).map(|()| client)
    }

    /// The optional D-10 proxy identity supplied at `connect` time.
    /// `None` for canonical-holder connections (`famp register`, MCP).
    /// `Some(name)` for one-shot CLI proxies (`send`, `inbox`, `await`, …).
    pub fn bind_as(&self) -> Option<&str> {
        self.bind_as.as_deref()
    }

    /// Send one `BusMessage`, return one `BusReply`. Strict 1:1 — every
    /// frame this layer writes is matched by exactly one frame read.
    pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, BusClientError> {
        let (mut reader, mut writer) = self.stream.split();
        codec::write_frame(&mut writer, &msg).await?;
        codec::read_frame::<_, BusReply>(&mut reader).await
    }

    /// Like [`send_recv`](Self::send_recv), but races the reply against
    /// readability on `abort` (issue #21). Returns `Ok(Some(reply))` when a
    /// bus reply arrives and `Ok(None)` when the abort wins.
    ///
    /// `abort` becomes readable on bytes OR EOF; the caller decides what
    /// that means (a queued host notification, a parent shutdown, …). This
    /// layer is host-neutral.
    ///
    /// ## Exit-code tie-break (why `biased;` + a grace re-poll)
    ///
    /// The read future is **pinned once and reused** so a partially-read
    /// frame is never dropped mid-decode. `biased;` polls the reply branch
    /// first, so if both are ready in the same tick the message wins. When
    /// only the abort is ready we still give the reply a short grace window
    /// (`ABORT_GRACE`) by re-polling the *same* pinned read future: an
    /// `AwaitOk` the broker already wrote to the socket must beat a
    /// simultaneous abort. Only if the grace elapses with no frame do we
    /// abort.
    pub async fn send_recv_abortable(
        &mut self,
        msg: BusMessage,
        abort: &tokio::io::unix::AsyncFd<std::os::fd::OwnedFd>,
    ) -> Result<Option<BusReply>, BusClientError> {
        /// Grace window during which an in-flight reply beats the abort.
        /// Generous enough to ride out a loaded-CI socket round-trip; the
        /// only cost of over-sizing it is delaying a *genuine* abort by at
        /// most this long, negligible against the multi-minute stall #21
        /// removes.
        const ABORT_GRACE: std::time::Duration = std::time::Duration::from_millis(500);

        let (mut reader, mut writer) = self.stream.split();
        codec::write_frame(&mut writer, &msg).await?;

        let read = codec::read_frame::<_, BusReply>(&mut reader);
        tokio::pin!(read);

        tokio::select! {
            biased;
            // Reply polled first: on a both-ready tie, the message wins.
            r = &mut read => Ok(Some(r?)),
            // Abort became readable. Re-poll the SAME pinned read future for
            // a grace window so an already-in-flight reply is not lost.
            _ = abort.readable() => {
                match tokio::time::timeout(ABORT_GRACE, &mut read).await {
                    Ok(r) => Ok(Some(r?)),
                    Err(_) => Ok(None),
                }
            }
        }
    }

    /// Cleanly close the connection (best-effort). Does not error if the
    /// broker has already gone away.
    pub async fn shutdown(&mut self) {
        let _ = self.stream.shutdown().await;
    }

    /// Wait until the underlying broker connection is closed (broker
    /// process death, OS-level reset, or graceful peer shutdown). The
    /// returned future never resolves while the broker is alive: under
    /// the Phase-1 request/reply contract the broker NEVER sends
    /// unsolicited frames, so any readable event indicates the peer has
    /// gone away.
    ///
    /// Used by `famp register`'s `block_until_disconnect` to drive the
    /// reconnect-loop arm. A 1-byte peek (`AsyncReadExt::read` on a
    /// 1-byte buffer) returns `Ok(0)` on EOF or a `BrokenPipe`/`ConnectionReset`
    /// error on abrupt close — both observed via the broker SIGKILL
    /// path (TEST-03). A nonzero read would mean the broker violated
    /// the request/reply invariant; we still surface it as "disconnect"
    /// because the stream is then desynchronized and unusable.
    pub async fn wait_for_disconnect(&mut self) {
        use tokio::io::AsyncReadExt;
        let mut probe = [0u8; 1];
        // Any read result — EOF (Ok(0)), broker-side close (Err), or
        // even an unsolicited byte (protocol violation) — means the
        // request/reply session is no longer usable. Return so the
        // outer loop tears down and reconnects.
        match self.stream.read(&mut probe).await {
            // IN-04: a non-zero read is a Phase-1 contract violation
            // (broker MUST NOT send unsolicited frames). Surface it so
            // a future broker bug doesn't manifest as a silent reconnect
            // storm.
            Ok(n) if n > 0 => eprintln!(
                "warning: broker sent {n} unsolicited byte(s) (0x{:02x}); disconnecting",
                probe[0]
            ),
            Ok(_) | Err(_) => {} // expected disconnect path
        }
    }
}

/// Resolve the broker socket path. `$FAMP_BUS_SOCKET` overrides;
/// otherwise `~/.famp/bus.sock`. Mirrors the v0.8 `FAMP_HOME` →
/// `FAMP_LOCAL_ROOT` precedence pattern but for the v0.9 bus socket.
///
/// # Behavior
/// Falls back to `/nonexistent-famp-home/.famp/bus.sock` when both
/// `$FAMP_BUS_SOCKET` and `$HOME` are unset, so the next syscall fails
/// visibly rather than silently writing into the cwd. (No panic — the
/// fallback path is intentionally non-existent on every supported
/// platform so connect/bind surface the misconfiguration.)
pub fn resolve_sock_path() -> PathBuf {
    if let Ok(p) = std::env::var("FAMP_BUS_SOCKET") {
        return PathBuf::from(p);
    }
    let home = dirs::home_dir().unwrap_or_else(|| {
        // Fall back to a clearly-bogus path so the next syscall fails
        // visibly rather than silently writing into the cwd. This path
        // must stay non-existent on every supported platform.
        PathBuf::from("/nonexistent-famp-home")
    });
    home.join(".famp").join("bus.sock")
}

/// Parent directory of the broker socket, used to anchor the broker's
/// log file, mailbox tree, and cursor files. Returns `sock_path.parent()`.
///
/// # Panics
/// Panics if `sock_path` has no parent (only possible if `sock_path`
/// is `/`); never reachable in practice for any caller of
/// `resolve_sock_path`.
pub fn bus_dir(sock_path: &Path) -> &Path {
    sock_path
        .parent()
        .unwrap_or_else(|| Path::new("/nonexistent-famp-home"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp_bus::BusReply;

    /// VER-01: A `HelloErr { BrokerProtoMismatch }` reply must produce
    /// `BusClientError::ProtocolMismatch` whose Display contains both
    /// "famp daemon restart" (D-02) and the broker's original message body.
    #[test]
    fn proto_mismatch_names_restart() {
        let broker_msg =
            "client bus_proto=2 is not supported by this broker; expected bus_proto=1".to_owned();
        let reply = BusReply::HelloErr {
            kind: famp_bus::BusErrorKind::BrokerProtoMismatch,
            message: broker_msg.clone(),
        };
        let err = classify_hello_reply(reply).unwrap_err();
        let display = err.to_string();
        assert!(
            display.contains("famp daemon restart"),
            "error must name `famp daemon restart`; got: {display}"
        );
        assert!(
            display.contains(&broker_msg),
            "error must relay broker message; got: {display}"
        );
        // Confirm it is the ProtocolMismatch variant, not HelloFailed.
        assert!(
            matches!(err, BusClientError::ProtocolMismatch { .. }),
            "must be ProtocolMismatch variant; got: {err:?}"
        );
    }

    /// VER-01: A `HelloOk` reply must produce `Ok(())` — matching proto
    /// connects normally without error.
    #[test]
    fn matching_proto_connects() {
        let reply = BusReply::HelloOk {
            bus_proto: famp_bus::BUS_PROTO_VERSION,
        };
        let result = classify_hello_reply(reply);
        assert!(result.is_ok(), "HelloOk must yield Ok(()); got: {result:?}");
    }

    #[test]
    fn resolve_sock_path_honours_env_override() {
        // WR-06: scoped via temp_env::with_var. Save+restore happens
        // automatically around the closure — survives test panics and
        // works under Rust 2024's `unsafe fn` `set_var`.
        temp_env::with_var(
            "FAMP_BUS_SOCKET",
            Some("/tmp/famp-test-resolve.sock"),
            || {
                let p = resolve_sock_path();
                assert_eq!(p, PathBuf::from("/tmp/famp-test-resolve.sock"));
            },
        );
    }

    #[test]
    fn bus_dir_returns_parent() {
        let sock = PathBuf::from("/tmp/famp/bus.sock");
        assert_eq!(bus_dir(&sock), Path::new("/tmp/famp"));
    }
}
