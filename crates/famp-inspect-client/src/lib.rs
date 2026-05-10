//! FAMP v0.10 Inspector RPC client.
//!
//! Two paths:
//! 1. Live broker: `connect_and_call(sock, kind)` performs Hello,
//!    sends `BusMessage::Inspect { kind }`, decodes
//!    `BusReply::InspectOk { payload }`.
//! 2. Dead broker: `raw_connect_probe(sock)` does not start a broker;
//!    it returns a diagnosis outcome from direct UDS connect + Hello.
//!
//! No clap dependency (INSP-CRATE-02): the CLI consumer does its own
//! argument parsing.

use std::path::Path;

use famp_bus::{BusMessage, BusReply};
use famp_inspect_proto::InspectKind;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;

/// Source of the holder PID in an ORPHAN_HOLDER evidence row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PidSource {
    /// SO_PEERCRED (Linux) or LOCAL_PEERPID (macOS) socket option.
    Peercred,
    /// `lsof -U <socket_path>` (macOS) or process-list fallback.
    Lsof,
    /// All discovery paths failed.
    Unknown,
}

/// Broker-down state classification for `famp inspect broker`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrokerDownState {
    DownClean,
    StaleSocket,
    OrphanHolder,
    PermissionDenied,
}

/// Outcome of `raw_connect_probe` against the socket path.
#[derive(Debug)]
pub enum ProbeOutcome {
    /// Socket opened and Hello succeeded; broker is alive.
    Healthy { stream: UnixStream },
    /// No socket file at the resolved path.
    DownClean,
    /// Socket file present but `connect()` returned ECONNREFUSED.
    StaleSocket,
    /// Connect succeeded but the listener rejected our `Hello`
    /// frame or replied with a non-`HelloOk` shape.
    OrphanHolder { hello_reject_summary: String },
    /// `connect()` returned EACCES.
    PermissionDenied,
}

#[derive(Debug, thiserror::Error)]
pub enum InspectClientError {
    #[error("io error talking to broker")]
    Io(#[from] std::io::Error),
    #[error("frame too large to encode")]
    FrameTooLarge,
    #[error("broker reply was not InspectOk: {0}")]
    UnexpectedReply(String),
    #[error("canonical-JSON error: {0}")]
    Canonical(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PeerPidError {
    #[error("io error during peer_pid discovery")]
    Io(#[from] std::io::Error),
}

/// Probe the socket without starting the broker. This uses raw
/// `UnixStream::connect` plus a manual Hello so dead-broker diagnosis
/// observes the socket exactly as it exists.
pub async fn raw_connect_probe(sock_path: &Path) -> ProbeOutcome {
    match tokio::fs::metadata(sock_path).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return ProbeOutcome::DownClean;
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return ProbeOutcome::PermissionDenied;
        }
        Err(_) => return ProbeOutcome::DownClean,
    }

    let mut stream = match UnixStream::connect(sock_path).await {
        Ok(s) => s,
        Err(e) => match e.kind() {
            std::io::ErrorKind::ConnectionRefused => return ProbeOutcome::StaleSocket,
            std::io::ErrorKind::PermissionDenied => return ProbeOutcome::PermissionDenied,
            std::io::ErrorKind::NotFound => return ProbeOutcome::DownClean,
            _ => return ProbeOutcome::StaleSocket,
        },
    };

    let hello = BusMessage::Hello {
        bus_proto: 1,
        client: "famp-inspect-client/0.10.0".into(),
        bind_as: None,
    };
    let bytes = match famp_canonical::canonicalize(&hello) {
        Ok(b) => b,
        Err(_) => {
            return ProbeOutcome::OrphanHolder {
                hello_reject_summary: "canonicalize failed".into(),
            };
        }
    };
    if write_frame(&mut stream, &bytes).await.is_err() {
        return ProbeOutcome::OrphanHolder {
            hello_reject_summary: "write_frame failed".into(),
        };
    }
    let reply_bytes = match read_frame(&mut stream).await {
        Ok(b) => b,
        Err(e) => {
            return ProbeOutcome::OrphanHolder {
                hello_reject_summary: format!("read_frame: {e}"),
            };
        }
    };
    let reply: Result<BusReply, _> = famp_canonical::from_slice_strict(&reply_bytes);
    match reply {
        Ok(BusReply::HelloOk { .. }) => ProbeOutcome::Healthy { stream },
        Ok(other) => ProbeOutcome::OrphanHolder {
            hello_reject_summary: format!("unexpected reply: {other:?}"),
        },
        Err(e) => ProbeOutcome::OrphanHolder {
            hello_reject_summary: format!("non-FAMP reply: {e}"),
        },
    }
}

/// Send `BusMessage::Inspect { kind }` over a Hello'd stream and
/// decode `BusReply::InspectOk { payload }`.
pub async fn call(
    stream: &mut UnixStream,
    kind: InspectKind,
) -> Result<serde_json::Value, InspectClientError> {
    let frame = BusMessage::Inspect { kind };
    let bytes = famp_canonical::canonicalize(&frame)
        .map_err(|e| InspectClientError::Canonical(e.to_string()))?;
    write_frame(stream, &bytes).await?;
    let reply_bytes = read_frame(stream).await?;
    let reply: BusReply = famp_canonical::from_slice_strict(&reply_bytes)
        .map_err(|e| InspectClientError::Canonical(e.to_string()))?;
    match reply {
        BusReply::InspectOk { payload } => Ok(payload),
        other => Err(InspectClientError::UnexpectedReply(format!("{other:?}"))),
    }
}

/// One-shot dead-broker probe + inspect call. Used by Wave 2 CLI
/// subcommands for fast-fail "broker not running" behavior.
pub async fn connect_and_call(
    sock_path: &Path,
    kind: InspectKind,
) -> Result<serde_json::Value, InspectClientError> {
    match raw_connect_probe(sock_path).await {
        ProbeOutcome::Healthy { mut stream } => call(&mut stream, kind).await,
        ProbeOutcome::DownClean | ProbeOutcome::StaleSocket | ProbeOutcome::OrphanHolder { .. } => {
            Err(InspectClientError::UnexpectedReply(
                "broker not running".into(),
            ))
        }
        ProbeOutcome::PermissionDenied => Err(InspectClientError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        ))),
    }
}

/// D-04 / D-05: discover the holder PID for the socket path.
pub async fn peer_pid(sock_path: &Path) -> Result<(Option<u32>, PidSource), PeerPidError> {
    if let Some(pid) = peer_pid_via_socket_option(sock_path).await {
        return Ok((Some(pid), PidSource::Peercred));
    }
    if let Some(pid) = peer_pid_via_subprocess(sock_path).await {
        return Ok((Some(pid), PidSource::Lsof));
    }
    Ok((None, PidSource::Unknown))
}

#[cfg(target_os = "linux")]
async fn peer_pid_via_socket_option(sock_path: &Path) -> Option<u32> {
    use std::os::fd::AsFd;

    let stream = UnixStream::connect(sock_path).await.ok()?;
    let creds =
        nix::sys::socket::getsockopt(&stream.as_fd(), nix::sys::socket::sockopt::PeerCredentials)
            .ok()?;
    let pid = creds.pid();
    if pid > 0 {
        u32::try_from(pid).ok()
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
async fn peer_pid_via_socket_option(sock_path: &Path) -> Option<u32> {
    use std::os::fd::AsFd;

    let stream = UnixStream::connect(sock_path).await.ok()?;
    let pid = nix::sys::socket::getsockopt(
        &stream.as_fd(),
        nix::sys::socket::sockopt::LocalPeerPid,
    )
    .ok()?;
    if pid > 0 {
        u32::try_from(pid).ok()
    } else {
        None
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
async fn peer_pid_via_socket_option(_sock_path: &Path) -> Option<u32> {
    None
}

#[cfg(target_os = "macos")]
async fn peer_pid_via_subprocess(sock_path: &Path) -> Option<u32> {
    use tokio::process::Command;

    let path = sock_path.to_string_lossy().into_owned();
    let fut = Command::new("lsof").args(["-U", "-Fp", &path]).output();
    let out = match tokio::time::timeout(std::time::Duration::from_secs(2), fut).await {
        Ok(Ok(o)) => o,
        _ => return None,
    };
    if !out.status.success() {
        return None;
    }
    for line in std::str::from_utf8(&out.stdout).ok()?.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            if let Ok(pid) = rest.trim().parse::<u32>() {
                return Some(pid);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
async fn peer_pid_via_subprocess(sock_path: &Path) -> Option<u32> {
    use tokio::process::Command;

    let path = sock_path.to_string_lossy().into_owned();
    let fut = Command::new("ss").args(["-lxep", &path]).output();
    let out = match tokio::time::timeout(std::time::Duration::from_secs(2), fut).await {
        Ok(Ok(o)) => o,
        _ => return None,
    };
    if !out.status.success() {
        return None;
    }
    for line in std::str::from_utf8(&out.stdout).ok()?.lines() {
        if let Some(idx) = line.find("pid=") {
            let rest = &line[idx + 4..];
            let end = rest.find([',', ')']).unwrap_or(rest.len());
            if let Ok(pid) = rest[..end].parse::<u32>() {
                return Some(pid);
            }
        }
    }
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
async fn peer_pid_via_subprocess(_sock_path: &Path) -> Option<u32> {
    None
}

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

async fn write_frame(stream: &mut UnixStream, payload: &[u8]) -> Result<(), InspectClientError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(InspectClientError::FrameTooLarge);
    }
    let len = u32::try_from(payload.len()).map_err(|_| InspectClientError::FrameTooLarge)?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, InspectClientError> {
    let mut len_buf = [0_u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(InspectClientError::FrameTooLarge);
    }
    let mut buf = vec![0_u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn raw_connect_probe_returns_down_clean_on_missing_socket() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("nonexistent.sock");
        match raw_connect_probe(&sock).await {
            ProbeOutcome::DownClean => {}
            other => panic!("expected DownClean, got {other:?}"),
        }
    }

    #[test]
    fn pid_source_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PidSource::Peercred).unwrap(),
            "\"peercred\""
        );
        assert_eq!(
            serde_json::to_string(&PidSource::Lsof).unwrap(),
            "\"lsof\""
        );
        assert_eq!(
            serde_json::to_string(&PidSource::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    #[tokio::test]
    async fn peer_pid_unknown_on_missing_socket() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = tmp.path().join("nonexistent.sock");
        let (pid, source) = peer_pid(&sock).await.unwrap();
        assert_eq!(pid, None);
        assert_eq!(source, PidSource::Unknown);
    }
}
