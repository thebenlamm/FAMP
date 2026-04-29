//! `DiskMailboxEnv` — `BrokerEnv` blanket impl backed by `famp-inbox`.
//!
//! The pure broker (`famp_bus::Broker`) is generic over `BrokerEnv`,
//! which is `MailboxRead + LivenessProbe`. This module supplies the
//! production implementation: mailbox reads come from JSONL files under
//! `<bus_dir>/mailboxes/<name>.jsonl`, and liveness probes use
//! `kill(pid, 0)` via `nix`.
//!
//! Two `Arc` clones of the env pattern (locked in plan 02-02): the
//! broker owns one clone for `MailboxRead::drain_from`, the executor
//! owns the other for `Out::AppendMailbox` (via `append`). Both clones
//! share the same internal `Mutex<HashMap<MailboxName, Inbox>>`, so
//! concurrent appends from the executor serialize correctly while the
//! broker can read the disk independently.
//!
//! On-disk file layout (Phase-1 D-09):
//!   - agent: `mailboxes/alice.jsonl`
//!   - channel: `mailboxes/#planning.jsonl`
//!
//! Each line is a raw canonical-JSON-encoded `BusEnvelope` (NOT a typed
//! `Vec<Value>` — the broker decodes via `AnyBusEnvelope::decode` before
//! serving `BusReply::{InboxOk,RegisterOk,JoinOk}.drained`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use famp_bus::LivenessProbe;
use famp_bus::{DrainResult, MailboxErr, MailboxName, MailboxRead};
use famp_inbox::Inbox;
use tokio::sync::Mutex;

/// Production `BrokerEnv`. Wrap in `Arc` and clone once for the broker
/// and once for the executor.
pub struct DiskMailboxEnv {
    bus_dir: PathBuf,
    /// Per-mailbox open `Inbox` handles, lazily opened on first append.
    /// `tokio::sync::Mutex` because `Inbox::append` is `async` and may
    /// suspend during fsync.
    inboxes: Mutex<HashMap<MailboxName, Arc<Inbox>>>,
}

impl DiskMailboxEnv {
    /// Create a new env rooted at `bus_dir`. Creates `bus_dir/mailboxes/`
    /// if missing. Errors propagate as `std::io::Error` (the caller —
    /// `cli/broker/mod.rs::run` — turns it into `CliError::Io`).
    pub fn new(bus_dir: &Path) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(bus_dir.join("mailboxes"))?;
        Ok(Self {
            bus_dir: bus_dir.to_path_buf(),
            inboxes: Mutex::new(HashMap::new()),
        })
    }

    /// Path to the on-disk JSONL for `name`. Channel names retain the
    /// `#` prefix (Phase-1 D-09 display form).
    fn mailbox_path(&self, name: &MailboxName) -> PathBuf {
        let display = name.to_string();
        self.bus_dir
            .join("mailboxes")
            .join(format!("{display}.jsonl"))
    }

    /// Append `line` to the mailbox file for `target`. Lazily opens the
    /// `Inbox` on first call. The `Inbox::append` call fsyncs before
    /// returning, so on `Ok` the line is durably persisted (Phase-1 D-04
    /// invariant: `AppendMailbox` BEFORE `Reply(SendOk)`).
    pub async fn append(&self, target: &MailboxName, line: Vec<u8>) -> Result<(), MailboxErr> {
        let inbox = {
            let mut guard = self.inboxes.lock().await;
            if let Some(existing) = guard.get(target) {
                Arc::clone(existing)
            } else {
                let path = self.mailbox_path(target);
                let opened = Inbox::open(&path).await.map_err(|e| {
                    MailboxErr::Internal(format!("Inbox::open {}: {e}", path.display()))
                })?;
                let arc = Arc::new(opened);
                guard.insert(target.clone(), Arc::clone(&arc));
                arc
            }
        };
        // Inbox::append takes `&self` (lock-internal), so dropping the
        // outer guard above is safe — concurrent appends to different
        // mailboxes can proceed without contending on `self.inboxes`.
        inbox
            .append(&line)
            .await
            .map_err(|e| MailboxErr::Internal(format!("Inbox::append: {e}")))?;
        Ok(())
    }
}

impl MailboxRead for DiskMailboxEnv {
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr> {
        let path = self.mailbox_path(name);
        read_raw_from(&path, since_bytes)
    }
}

impl LivenessProbe for DiskMailboxEnv {
    fn is_alive(&self, pid: u32) -> bool {
        // BL-05: POSIX defines `kill(pid=0, sig)` as targeting every
        // process in the calling pgrp; with `sig=None` it returns
        // Ok(()) whenever the calling process has any pgrp (i.e.
        // always). That would let a misbehaving client claim PID 0 and
        // ride forever as "alive", defeating the D-10 per-op proxy
        // liveness gate. Reject PID 0 (and any non-positive raw) up
        // front. `i32::try_from` already rejects values ≥ 2^31.
        if pid == 0 {
            return false;
        }
        let Ok(raw) = i32::try_from(pid) else {
            return false;
        };
        if raw <= 0 {
            return false;
        }
        nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), None).is_ok()
    }
}

/// Newtype `Arc<DiskMailboxEnv>` wrapper for the broker.
///
/// Satisfies the orphan rule for `impl ForeignTrait for
/// ForeignType<LocalType>`. The executor and the broker each hold a
/// `BrokerEnvHandle` clone (cheap `Arc` clone — same inner state); the
/// blanket `impl<T: MailboxRead + LivenessProbe> BrokerEnv for T`
/// (env.rs) then auto-applies.
#[derive(Clone)]
pub struct BrokerEnvHandle(Arc<DiskMailboxEnv>);

impl BrokerEnvHandle {
    #[must_use]
    pub const fn new(env: Arc<DiskMailboxEnv>) -> Self {
        Self(env)
    }

    pub async fn append(&self, target: &MailboxName, line: Vec<u8>) -> Result<(), MailboxErr> {
        self.0.append(target, line).await
    }
}

impl MailboxRead for BrokerEnvHandle {
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr> {
        self.0.drain_from(name, since_bytes)
    }
}

impl LivenessProbe for BrokerEnvHandle {
    fn is_alive(&self, pid: u32) -> bool {
        self.0.is_alive(pid)
    }
}

/// Read complete JSONL lines from `path` past `since_bytes`, returning
/// each line as raw bytes (without the trailing `\n`) plus the byte
/// offset of the first byte after the last consumed line. Mirrors
/// `famp_inbox::read::read_from` but keeps raw bytes — the broker
/// re-decodes lines via `AnyBusEnvelope::decode` (see
/// `famp-bus/src/broker/handle.rs::decode_lines`) so we MUST hand back
/// the on-disk bytes verbatim, not parsed JSON Values.
fn read_raw_from(path: &Path, since_bytes: u64) -> Result<DrainResult, MailboxErr> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DrainResult {
                lines: Vec::new(),
                next_offset: since_bytes,
            });
        }
        Err(source) => {
            return Err(MailboxErr::Internal(format!(
                "read {}: {source}",
                path.display()
            )));
        }
    };
    let file_len = bytes.len() as u64;
    if since_bytes >= file_len {
        return Ok(DrainResult {
            lines: Vec::new(),
            next_offset: file_len,
        });
    }
    let Ok(start) = usize::try_from(since_bytes) else {
        return Ok(DrainResult {
            lines: Vec::new(),
            next_offset: since_bytes,
        });
    };

    // Snap forward to the next `\n + 1` boundary if start is mid-line.
    let snapped = if start == 0 || bytes.get(start - 1) == Some(&b'\n') {
        start
    } else {
        match bytes[start..].iter().position(|&b| b == b'\n') {
            Some(off) => start + off + 1,
            None => {
                return Ok(DrainResult {
                    lines: Vec::new(),
                    next_offset: file_len,
                })
            }
        }
    };
    if snapped >= bytes.len() {
        return Ok(DrainResult {
            lines: Vec::new(),
            next_offset: file_len,
        });
    }

    let mut lines: Vec<Vec<u8>> = Vec::new();
    let mut running = snapped as u64;
    let mut cursor = snapped;
    let total = bytes.len();
    while cursor < total {
        // Find next newline.
        let Some(rel) = bytes[cursor..].iter().position(|&b| b == b'\n') else {
            // Tail tolerance: silently drop a partial trailing line; do
            // NOT advance `running` past it.
            break;
        };
        let line_end = cursor + rel;
        lines.push(bytes[cursor..line_end].to_vec());
        running += (rel as u64) + 1;
        cursor = line_end + 1;
    }

    Ok(DrainResult {
        lines,
        next_offset: running,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_then_drain_round_trips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        let name = MailboxName::Agent("alice".into());

        env.append(&name, b"line1".to_vec()).await.unwrap();
        env.append(&name, b"line2".to_vec()).await.unwrap();

        let drained = env.drain_from(&name, 0).unwrap();
        assert_eq!(drained.lines, vec![b"line1".to_vec(), b"line2".to_vec()]);
        assert_eq!(drained.next_offset, "line1\nline2\n".len() as u64);
    }

    #[tokio::test]
    async fn drain_from_nonzero_offset_skips_consumed_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        let name = MailboxName::Agent("bob".into());
        env.append(&name, b"a".to_vec()).await.unwrap();
        env.append(&name, b"b".to_vec()).await.unwrap();
        let first = env.drain_from(&name, 0).unwrap();
        assert_eq!(first.lines.len(), 2);
        let second = env.drain_from(&name, first.next_offset).unwrap();
        assert!(second.lines.is_empty());
        assert_eq!(second.next_offset, first.next_offset);
    }

    #[test]
    fn drain_from_missing_file_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        let name = MailboxName::Agent("nobody".into());
        let drained = env.drain_from(&name, 0).unwrap();
        assert!(drained.lines.is_empty());
        assert_eq!(drained.next_offset, 0);
    }

    #[test]
    fn channel_mailbox_path_uses_hash_prefix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        let path = env.mailbox_path(&MailboxName::Channel("#planning".into()));
        assert!(path
            .to_str()
            .unwrap()
            .ends_with("mailboxes/#planning.jsonl"));
    }

    #[test]
    fn is_alive_returns_true_for_self() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        let pid = std::process::id();
        assert!(env.is_alive(pid));
    }

    #[test]
    fn is_alive_returns_false_for_likely_dead_pid() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = DiskMailboxEnv::new(tmp.path()).unwrap();
        // PID 1 always exists, so we can't use it. Use a very large PID
        // that is overwhelmingly unlikely to exist; pid_max on Linux is
        // typically 4_194_304 and on macOS 99_998.
        assert!(!env.is_alive(0xFFFF_FFFE));
    }
}
