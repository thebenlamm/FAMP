# Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand — Pattern Map

**Mapped:** 2026-04-28
**Files analyzed:** 28 new/modified files
**Analogs found:** 23 / 28

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/famp/src/bus_client/mod.rs` | service | request-response | `crates/famp/src/cli/send/client.rs` | role-match |
| `crates/famp/src/bus_client/spawn.rs` | utility | event-driven | `crates/famp/src/cli/listen/mod.rs` (bind pattern) | partial |
| `crates/famp/src/bus_client/codec.rs` | utility | request-response | `crates/famp-bus/src/codec.rs` | exact |
| `crates/famp/src/cli/broker/mod.rs` | service | event-driven | `crates/famp/src/cli/listen/mod.rs` | role-match |
| `crates/famp/src/cli/broker/nfs_check.rs` | utility | request-response | none (greenfield) | none |
| `crates/famp/src/cli/broker/mailbox_env.rs` | service | file-I/O | `crates/famp-inbox/src/append.rs` + `crates/famp-bus/src/env.rs` | role-match |
| `crates/famp/src/cli/broker/cursor_exec.rs` | utility | file-I/O | `crates/famp-inbox/src/cursor.rs` | exact |
| `crates/famp/src/cli/identity.rs` | utility | request-response | `scripts/famp-local` `wires_lookup` + `cmd_identity_of` (bash analog) | role-match |
| `crates/famp/src/cli/register.rs` | controller | request-response | `crates/famp/src/cli/listen/mod.rs` | role-match |
| `crates/famp/src/cli/send/mod.rs` (rewire) | controller | request-response | `crates/famp/src/cli/send/mod.rs` (current) | exact |
| `crates/famp/src/cli/inbox/mod.rs` (rewire) | controller | request-response | `crates/famp/src/cli/inbox/mod.rs` (current) | exact |
| `crates/famp/src/cli/await_cmd/mod.rs` (rewire) | controller | request-response | `crates/famp/src/cli/await_cmd/mod.rs` (current) | exact |
| `crates/famp/src/cli/join.rs` | controller | request-response | `crates/famp/src/cli/inbox/mod.rs` | role-match |
| `crates/famp/src/cli/leave.rs` | controller | request-response | `crates/famp/src/cli/inbox/mod.rs` | role-match |
| `crates/famp/src/cli/sessions.rs` | controller | request-response | `crates/famp/src/cli/inbox/list.rs` | role-match |
| `crates/famp/src/cli/whoami.rs` | controller | request-response | `crates/famp/src/cli/info.rs` | role-match |
| `crates/famp/src/cli/mcp/session.rs` (reshape) | service | request-response | `crates/famp/src/cli/mcp/session.rs` (current) | exact |
| `crates/famp/src/cli/mcp/error_kind.rs` (retarget) | utility | request-response | `crates/famp/src/cli/mcp/error_kind.rs` (current) | exact |
| `crates/famp/src/cli/mcp/tools/register.rs` (rewrite) | controller | request-response | `crates/famp/src/cli/mcp/tools/register.rs` (current) | exact |
| `crates/famp/src/cli/mcp/tools/send.rs` (rewrite) | controller | request-response | `crates/famp/src/cli/mcp/tools/send.rs` (current) | exact |
| `crates/famp/src/cli/mcp/tools/{inbox,await_,peers,whoami}.rs` (rewrite) | controller | request-response | `crates/famp/src/cli/mcp/tools/send.rs` | role-match |
| `crates/famp/src/cli/mcp/tools/join.rs` (new) | controller | request-response | `crates/famp/src/cli/mcp/tools/send.rs` | role-match |
| `crates/famp/src/cli/mcp/tools/leave.rs` (new) | controller | request-response | `crates/famp/src/cli/mcp/tools/send.rs` | role-match |
| `crates/famp/src/bin/famp.rs` (extend) | config | — | `crates/famp/src/bin/famp.rs` (current) | exact |
| `scripts/famp-local` (hook additions) | utility | event-driven | `scripts/famp-local` `cmd_wire` (bash) | exact |
| `crates/famp/tests/broker_lifecycle.rs` | test | event-driven | `crates/famp/tests/listen_smoke.rs` | role-match |
| `crates/famp/tests/broker_spawn_race.rs` + `broker_crash_recovery.rs` | test | event-driven | `crates/famp/tests/mcp_stdio_tool_calls.rs` (subprocess pattern) | role-match |
| `crates/famp/tests/{cli_dm_roundtrip,cli_channel_fanout,cli_inbox,cli_sessions,mcp_bus_e2e,hook_subcommand}.rs` | test | request-response | `crates/famp/tests/mcp_stdio_tool_calls.rs` | role-match |

---

## Pattern Assignments

### `crates/famp/src/bus_client/codec.rs` (utility, request-response)

**Analog:** `crates/famp-bus/src/codec.rs`

The sync codec already exists in `famp-bus`. The `bus_client/codec.rs` module wraps it for async tokio I/O. Copy the frame constants and error type; add `AsyncReadExt`/`AsyncWriteExt` wrappers.

**Frame encode/decode pattern** (entire file, 53 lines):
```rust
// crates/famp-bus/src/codec.rs — full file is the pattern source.
// BUS-06: 4-byte big-endian unsigned length prefix; max 16 MiB; min 1 byte payload.
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const LEN_PREFIX_BYTES: usize = 4;

pub fn encode_frame<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, FrameError> {
    let body = famp_canonical::canonicalize(value)?;
    let Ok(len) = u32::try_from(body.len()) else {
        return Err(FrameError::FrameTooLarge(u32::MAX));
    };
    let mut out = Vec::with_capacity(LEN_PREFIX_BYTES + body.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn try_decode_frame<T: DeserializeOwned>(buf: &[u8]) -> Result<Option<(T, usize)>, FrameError> {
    if buf.len() < LEN_PREFIX_BYTES { return Ok(None); }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    // ... validate length, decode payload
    let value: T = famp_canonical::from_slice_strict(payload).map_err(FrameError::Decode)?;
    Ok(Some((value, total)))
}
```

**Tokio async wrapper pattern to add** (new in this file):
```rust
// bus_client/codec.rs — wrap sync codec for AsyncRead/AsyncWrite
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), BusClientError>
where W: AsyncWriteExt + Unpin, T: Serialize + ?Sized
{
    let frame = famp_bus::codec::encode_frame(value)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, BusClientError>
where R: AsyncReadExt + Unpin, T: DeserializeOwned
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    let value: T = famp_canonical::from_slice_strict(&body)?;
    Ok(value)
}
```

---

### `crates/famp/src/bus_client/mod.rs` (service, request-response)

**Analog:** `crates/famp/src/cli/send/client.rs` (request-response client pattern)

**Struct + connect + send_recv pattern:**
```rust
// bus_client/mod.rs
pub struct BusClient {
    stream: tokio::net::UnixStream,
}

impl BusClient {
    /// Connect to sock_path; spawn broker if absent.
    pub async fn connect(sock_path: &Path) -> Result<Self, BusClientError> {
        spawn::spawn_broker_if_absent(sock_path)?;
        // poll up to 2s for socket to appear
        let stream = tokio::net::UnixStream::connect(sock_path).await?;
        Ok(Self { stream })
    }

    /// Send one BusMessage, receive one BusReply (Hello handshake embedded).
    pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, BusClientError> {
        codec::write_frame(&mut self.stream, &msg).await?;
        codec::read_frame(&mut self.stream).await
    }
}
```

**Hello handshake pattern** (called immediately after connect):
```rust
// In BusClient::connect, after UnixStream::connect succeeds:
let mut client = Self { stream };
let hello = BusMessage::Hello { bus_proto: 1, client: "famp-cli/0.9.0".to_string() };
match client.send_recv(hello).await? {
    BusReply::HelloOk { .. } => Ok(client),
    BusReply::Err { kind, message } => Err(BusClientError::HelloFailed { kind, message }),
    other => Err(BusClientError::UnexpectedReply(format!("{other:?}"))),
}
```

**Socket path resolution pattern** (mirrors `FAMP_HOME` → `FAMP_LOCAL_ROOT` pattern from v0.8):
```rust
// Single resolution point — both BusClient::connect and broker bind() call this.
pub fn resolve_sock_path() -> PathBuf {
    if let Ok(p) = std::env::var("FAMP_BUS_SOCKET") {
        PathBuf::from(p)
    } else {
        dirs::home_dir()
            .expect("home dir must exist")
            .join(".famp")
            .join("bus.sock")
    }
}

pub fn bus_dir(sock_path: &Path) -> &Path {
    sock_path.parent().expect("socket path must have a parent")
}
```

---

### `crates/famp/src/bus_client/spawn.rs` (utility, event-driven)

**Analog:** `crates/famp/src/cli/listen/mod.rs` (bind and listener pattern; no direct UDS spawn analog exists)

**Greenfield: `posix_spawn` + `setsid` via `nix 0.31.2`.** No existing analog in the codebase. Use RESEARCH.md §5 verbatim. Key pattern elements:

```rust
// spawn.rs
use nix::spawn::{PosixSpawnAttr, PosixSpawnFileActions, posix_spawnp};

pub fn spawn_broker_if_absent(sock_path: &Path) -> Result<(), SpawnError> {
    // Try connect first — if already running, return immediately.
    if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
        return Ok(());
    }
    // Build posix_spawn attrs with POSIX_SPAWN_SETSID for terminal detach.
    let mut attr = PosixSpawnAttr::new()?;
    attr.setflags(nix::spawn::PosixSpawnFlags::POSIX_SPAWN_SETSID)?;
    // Redirect stdin → /dev/null, stdout+stderr → bus_dir/broker.log
    let bus_dir = sock_path.parent().expect("socket has parent");
    let log_path = bus_dir.join("broker.log");
    let mut fa = PosixSpawnFileActions::new()?;
    fa.open(1, &log_path, O_WRONLY | O_CREAT | O_APPEND, S_IRUSR | S_IWUSR)?;
    fa.dup2(1, 2)?;
    fa.open(0, Path::new("/dev/null"), O_RDONLY, Mode::empty())?;
    // Spawn self as "famp broker --socket <path>"
    let exe = std::env::current_exe()?;
    posix_spawnp(exe.to_str().unwrap(), &fa, &attr,
        &[exe.to_str().unwrap(), "broker", "--socket", sock_path.to_str().unwrap()],
        &[])?;
    // Poll up to 2s (10 × 200ms) for the socket to appear.
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
            return Ok(());
        }
    }
    Err(SpawnError::BrokerDidNotStart)
}
```

---

### `crates/famp/src/cli/broker/mod.rs` (service, event-driven)

**Analog:** `crates/famp/src/cli/listen/mod.rs` (tokio async server loop)

**Listen mod structure to copy** (lines 1-85, `listen/mod.rs`):
- `run(args: BrokerArgs) -> Result<(), CliError>` — production entry, resolves socket path, binds, calls `run_on_listener`
- `run_on_listener(sock_path, listener, shutdown_signal) -> Result<(), CliError>` — test-facing, takes pre-bound listener
- `tokio::select!` with accept arm + signal arm

**Broker-specific loop shape** (from RESEARCH.md §5, not in listen/mod.rs):
```rust
// cli/broker/mod.rs — UDS accept loop with idle timer
pub async fn run_on_listener(
    sock_path: &Path,
    listener: tokio::net::UnixListener,
    shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<(), CliError> {
    let (broker_tx, mut broker_rx) = tokio::sync::mpsc::channel::<BrokerMsg>(1024);
    let mut reply_senders: std::collections::HashMap<ClientId, tokio::sync::mpsc::Sender<BusReply>> = Default::default();
    let mut broker = Broker::new(DiskMailboxEnv::new(bus_dir(sock_path)));
    let mut client_count: u32 = 0;
    let mut idle: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
    let mut next_id: u64 = 0;
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            Ok((stream, _)) = listener.accept() => {
                client_count += 1;
                idle = None; // cancel idle timer on new connection
                let id = ClientId(next_id); next_id += 1;
                let (reply_tx, reply_rx) = tokio::sync::mpsc::channel(64);
                reply_senders.insert(id, reply_tx);
                tokio::spawn(client_task(id, stream, broker_tx.clone(), reply_rx));
            }
            Some(msg) = broker_rx.recv() => {
                // ... drive broker.handle(input, Instant::now()); execute_outs(outs)
                if let BrokerMsg::Disconnect(id) = &msg {
                    reply_senders.remove(id);
                    client_count -= 1;
                    if client_count == 0 {
                        idle = Some(Box::pin(tokio::time::sleep(Duration::from_secs(300))));
                    }
                }
            }
            _ = tick_interval.tick() => {
                let outs = broker.handle(BrokerInput::Tick, Instant::now());
                execute_outs(outs, &reply_senders, &mut broker.env).await;
            }
            _ = wait_or_never(&mut idle) => {
                // clean shutdown: fsync mailboxes, remove socket, exit
                std::fs::remove_file(sock_path).ok();
                return Ok(());
            }
            () = shutdown_signal => {
                eprintln!("shutdown signal received, exiting");
                return Ok(());
            }
        }
    }
}

// Helper: returns Pending when idle=None, delegates to Sleep when Some.
async fn wait_or_never(idle: &mut Option<Pin<Box<tokio::time::Sleep>>>) {
    if let Some(ref mut s) = idle { s.await } else { std::future::pending().await }
}
```

**Shutdown signal pattern** (copy from `listen/signal.rs`):
```rust
// listen/signal.rs — existing SIGINT/SIGTERM shutdown signal
pub async fn shutdown_signal() { /* tokio::signal::ctrl_c() etc. */ }
```

**Bind-exclusion algorithm** (no existing analog; greenfield per RESEARCH.md §5):
```rust
// broker/mod.rs startup before run_on_listener
fn bind_exclusive(sock_path: &Path) -> Result<tokio::net::UnixListener, CliError> {
    match tokio::net::UnixListener::bind(sock_path) {
        Ok(l) => Ok(l),
        Err(e) if e.raw_os_error() == Some(libc::EADDRINUSE) => {
            // Try connecting — if OK, live broker exists; exit 0 (caller exits).
            if std::os::unix::net::UnixStream::connect(sock_path).is_ok() {
                std::process::exit(0);
            }
            // ECONNREFUSED → stale socket; unlink and retry once.
            std::fs::remove_file(sock_path).map_err(|_| CliError::Io { .. })?;
            tokio::net::UnixListener::bind(sock_path).map_err(|e| CliError::Io { .. })
        }
        Err(e) => Err(CliError::Io { path: sock_path.to_path_buf(), source: e }),
    }
}
```

---

### `crates/famp/src/cli/broker/nfs_check.rs` (utility, request-response)

**No existing analog.** Greenfield via `nix::sys::statfs`. RESEARCH.md §2 §"Item 3" contains the full implementation. Copy it verbatim:

```rust
// cli/broker/nfs_check.rs — full function, platform-conditional
#[cfg(target_os = "linux")]
pub fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::{statfs, NFS_SUPER_MAGIC};
    statfs(path).map(|s| s.filesystem_type() == NFS_SUPER_MAGIC).unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::statfs;
    statfs(path).map(|s| s.filesystem_type_name().to_bytes().starts_with(b"nfs")).unwrap_or(false)
}
```

---

### `crates/famp/src/cli/broker/mailbox_env.rs` (service, file-I/O)

**Analogs:** `crates/famp-bus/src/env.rs` (trait definition) + `crates/famp-inbox/src/append.rs` (Inbox struct)

**BrokerEnv impl pattern** (env.rs lines 1-7):
```rust
// famp-bus/src/env.rs — BrokerEnv blanket impl pattern
pub trait BrokerEnv: MailboxRead + LivenessProbe {}
impl<T: MailboxRead + LivenessProbe> BrokerEnv for T {}
```

**DiskMailboxEnv struct pattern:**
```rust
// cli/broker/mailbox_env.rs
pub struct DiskMailboxEnv {
    bus_dir: PathBuf,
    inboxes: std::collections::HashMap<MailboxName, famp_inbox::Inbox>,
}

impl MailboxRead for DiskMailboxEnv {
    fn drain_from(&self, name: &MailboxName, since_bytes: u64)
        -> Result<DrainResult, MailboxErr>
    {
        let path = self.mailbox_path(name);
        let lines_with_offset = famp_inbox::read::read_from(&path, since_bytes)?;
        Ok(DrainResult { lines: ..., next_offset: ... })
    }
}

// Appending (from Out::AppendMailbox executor, NOT inside MailboxRead):
// famp_inbox::Inbox::append(&line) — already fsyncs.
```

**Cursor write pattern** — copy `famp-inbox/src/cursor.rs` `InboxCursor::advance` (lines 58-91):
```rust
// cursor.rs lines 58-91 — atomic temp+rename pattern
pub async fn advance(&self, offset: u64) -> Result<(), InboxError> {
    let path = self.path.clone();
    let body = format!("{offset}\n");
    let res = tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
        let parent = path.parent().ok_or_else(|| std::io::Error::new(...))?;
        std::fs::create_dir_all(parent)?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
        tmp.write_all(body.as_bytes())?;
        tmp.as_file_mut().sync_all()?;
        tmp.persist(&path).map_err(|e| e.error)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }).await;
    // ... handle Ok(Ok), Ok(Err), Err(join) per existing cursor.rs pattern
}
```

---

### `crates/famp/src/cli/identity.rs` (utility, request-response)

**Analog:** `scripts/famp-local` `wires_lookup` (lines 137-145) + `cmd_identity_of` (lines 1090-1148, bash)

**D-01 resolution chain to copy:**
```rust
// cli/identity.rs
pub fn resolve_identity(
    as_flag: Option<&str>,      // --as <name>
) -> Result<String, CliError> {
    // Tier 1: explicit --as flag
    if let Some(name) = as_flag {
        return Ok(name.to_string());
    }
    // Tier 2: env var
    if let Ok(name) = std::env::var("FAMP_LOCAL_IDENTITY") {
        if !name.is_empty() { return Ok(name); }
    }
    // Tier 3: cwd → wires.tsv exact match (mirrors wires_lookup in bash)
    let cwd = std::env::current_dir()
        .map_err(|e| CliError::Io { path: PathBuf::new(), source: e })?;
    let cwd = cwd.canonicalize()
        .map_err(|e| CliError::Io { path: cwd.clone(), source: e })?;
    let wires_path = dirs::home_dir()
        .expect("home must exist")
        .join(".famp-local")
        .join("wires.tsv");
    if let Ok(content) = std::fs::read_to_string(&wires_path) {
        for line in content.lines() {
            let mut parts = line.splitn(2, '\t');
            if let (Some(dir), Some(name)) = (parts.next(), parts.next()) {
                if std::path::Path::new(dir) == cwd {
                    return Ok(name.to_string());
                }
            }
        }
    }
    // Tier 4: hard error
    Err(CliError::NotRegistered) // or a new CliError::NoIdentityBound
    // message: "no identity bound — pass --as, set $FAMP_LOCAL_IDENTITY,
    //           or run `famp-local wire <dir>` first"
}
```

**Bash analog for this function** (`scripts/famp-local` lines 137-145, `wires_lookup`):
```bash
wires_lookup() {
  local cdir="$1"
  local wf; wf="$(wires_file)"
  [ -f "$wf" ] || return 1
  id="$(awk -F'\t' -v cdir="$cdir" '$1 == cdir { print $2; found=1; exit } END { exit !found }' "$wf")" || return 1
  [ -n "$id" ] || return 1
  printf '%s\n' "$id"
}
```

---

### `crates/famp/src/cli/register.rs` (controller, request-response)

**Analog:** `crates/famp/src/cli/listen/mod.rs` (long-lived blocking subcommand pattern)

**Args + run pattern** (copy from listen/mod.rs lines 35-85):
```rust
// cli/register.rs
#[derive(clap::Args, Debug)]
pub struct RegisterArgs {
    /// Identity name to register as. Broker holds the slot for the lifetime
    /// of this process.
    pub name: String,
    /// Opt into live event stream on stderr (< prefix per received envelope).
    #[arg(long)]
    pub tail: bool,
    /// Exit non-zero on first broker disconnect instead of reconnecting.
    /// Use in tests/CI for deterministic behavior.
    #[arg(long)]
    pub no_reconnect: bool,
}

pub async fn run(args: RegisterArgs) -> Result<(), CliError> {
    let sock = bus_client::resolve_sock_path();
    // Reconnect loop with bounded exponential backoff (D-09)
    let mut delay = Duration::from_secs(1);
    loop {
        match BusClient::connect(&sock).await {
            Ok(mut client) => {
                let pid = std::process::id();
                let reply = client.send_recv(BusMessage::Register { name: args.name.clone(), pid }).await?;
                match reply {
                    BusReply::RegisterOk { drained, joined } => {
                        eprintln!("registered as {} (pid {}, joined: {:?}) — Ctrl-C to release",
                            args.name, pid, joined);
                        if args.tail {
                            tail_loop(&mut client, &args.name).await?;
                        } else {
                            block_until_disconnect(&mut client).await?;
                        }
                    }
                    BusReply::Err { kind, .. } => return Err(kind.into()),
                    other => return Err(CliError::Io { .. }),
                }
                delay = Duration::from_secs(1); // reset on clean disconnect
            }
            Err(_) => {
                if args.no_reconnect { return Err(CliError::Io { .. }); }
                eprintln!("broker disconnected — reconnecting in {}s", delay.as_secs());
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                let _ = bus_client::spawn::spawn_broker_if_absent(&sock);
            }
        }
    }
}
```

---

### `crates/famp/src/cli/send/mod.rs` (rewire, controller, request-response)

**Analog:** Current `crates/famp/src/cli/send/mod.rs` (exact — preserve arg shapes, swap transport)

**Args to preserve** (lines 42-64, send/mod.rs):
```rust
// Keep entire SendArgs struct unchanged — #[arg(conflicts_with)] + #[arg(requires)] matrix preserved.
// Add --as flag:
#[arg(long)]
pub send_as: Option<String>,
```

**Transport swap** (lines 200-210, from `client::post_envelope(...)` to `bus.send_recv(...)`):
```rust
// BEFORE (v0.8):
let outcome = client::post_envelope(&peer.endpoint, ...).await?;

// AFTER (v0.9):
let identity = resolve_identity(args.send_as.as_deref())?;
let mut bus = BusClient::connect(&resolve_sock_path()).await
    .map_err(|_| CliError::NotRegistered)?;
let reply = bus.send_recv(BusMessage::Send {
    to: Target::Agent { name: args.to.clone() },
    envelope: serde_json::to_value(&envelope_bytes)?,
    send_as: args.send_as.clone(),
}).await?;
let outcome = match reply {
    BusReply::SendOk { task_id, delivered } => SendOutcome { task_id: task_id.to_string(), state: "requested".to_string() },
    BusReply::Err { kind, message } => return Err(bus_error_to_cli(kind, &message)),
    other => return Err(CliError::Io { .. }),
};
```

**`run` / `run_at` / `run_at_structured` pattern** — preserve the three-layer pattern (lines 88-101) so MCP tools can continue calling `run_at_structured` unchanged.

---

### `crates/famp/src/cli/join.rs` + `crates/famp/src/cli/leave.rs` (controller, request-response)

**Analog:** `crates/famp/src/cli/inbox/mod.rs` (subcommand-with-identity-resolution pattern)

**Pattern to copy** (inbox/mod.rs lines 1-64):
```rust
// cli/join.rs — modeled on inbox/mod.rs minimal subcommand shape
#[derive(clap::Args, Debug)]
pub struct JoinArgs {
    pub channel: String,
    #[arg(long)]
    pub send_as: Option<String>,  // --as flag for D-01 resolution
}

pub async fn run(args: JoinArgs) -> Result<(), CliError> {
    let identity = resolve_identity(args.send_as.as_deref())?;
    let channel = normalize_channel(&args.channel)?;
    let sock = bus_client::resolve_sock_path();
    let mut bus = BusClient::connect(&sock).await.map_err(|_| CliError::NotRegistered)?;
    let reply = bus.send_recv(BusMessage::Join { channel: channel.clone() }).await?;
    match reply {
        BusReply::JoinOk { members, drained } => {
            let out = serde_json::json!({ "channel": channel, "members": members, "drained": drained });
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
        BusReply::Err { kind, message } => Err(bus_error_to_cli(kind, &message)),
        other => Err(CliError::Io { .. }),
    }
}
```

**Channel normalization** (RESEARCH.md §2 §"Item 11"):
```rust
fn normalize_channel(input: &str) -> Result<String, CliError> {
    let normalized = if input.starts_with('#') { input.to_string() } else { format!("#{input}") };
    if normalized.starts_with("##") {
        return Err(CliError::SendArgsInvalid { reason: "channel name cannot start with ##".into() });
    }
    // Validate against BUS-04 regex (same CHANNEL_RE used in proto.rs)
    if !CHANNEL_RE.is_match(&normalized) {
        return Err(CliError::SendArgsInvalid { reason: format!("invalid channel name: {normalized}") });
    }
    Ok(normalized)
}
```

---

### `crates/famp/src/cli/sessions.rs` + `crates/famp/src/cli/whoami.rs` (controller, request-response)

**Analog:** `crates/famp/src/cli/inbox/list.rs` (JSONL stdout output pattern)

**JSONL output pattern** (copy from inbox/list.rs stdout loop):
```rust
// cli/sessions.rs — JSONL-per-row output, same shape as inbox list
pub async fn run(args: SessionsArgs) -> Result<(), CliError> {
    let sock = bus_client::resolve_sock_path();
    let mut bus = BusClient::connect(&sock).await.map_err(|_| CliError::NotRegistered)?;
    let reply = bus.send_recv(BusMessage::Sessions {}).await?;
    match reply {
        BusReply::SessionsOk { rows } => {
            // --me filter: resolve identity, emit only matching row
            let filter = if args.me { Some(resolve_identity(None)?) } else { None };
            for row in &rows {
                if filter.as_deref().map_or(true, |name| row.name == name) {
                    println!("{}", serde_json::to_string(row)?);
                }
            }
            Ok(())
        }
        BusReply::Err { kind, message } => Err(bus_error_to_cli(kind, &message)),
        other => Err(CliError::Io { .. }),
    }
}
```

---

### `crates/famp/src/cli/mcp/session.rs` (reshape, service, request-response)

**Analog:** Current `crates/famp/src/cli/mcp/session.rs` (exact — preserve OnceLock+Mutex pattern, replace inner type)

**Full pattern to preserve** (lines 65-93):
```rust
// session.rs — keep this exact OnceLock<Mutex<...>> pattern; replace IdentityBinding with SessionState
fn state() -> &'static Mutex<Option<IdentityBinding>> {
    static STATE: OnceLock<Mutex<Option<IdentityBinding>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub async fn current() -> Option<IdentityBinding> { state().lock().await.clone() }
pub async fn set(binding: IdentityBinding) -> Option<IdentityBinding> {
    let mut guard = state().lock().await;
    guard.replace(binding)
}
pub async fn clear() -> Option<IdentityBinding> {
    let mut guard = state().lock().await;
    guard.take()
}
```

**New inner type for v0.9** (from RESEARCH.md §7):
```rust
// session.rs v0.9 — drop IdentityBinding/home_path; add bus + active_identity
struct SessionState {
    bus: Option<BusClient>,           // None until first famp_register
    active_identity: Option<String>,  // set by famp_register tool
}

fn state() -> &'static Mutex<SessionState> {
    static S: OnceLock<Mutex<SessionState>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(SessionState { bus: None, active_identity: None }))
}

pub async fn ensure_bus() -> Result<(), BusErrorKind> {
    let mut guard = state().lock().await;
    if guard.bus.is_none() {
        let sock = bus_client::resolve_sock_path();
        guard.bus = Some(BusClient::connect(&sock).await.map_err(|_| BusErrorKind::BrokerUnreachable)?);
    }
    Ok(())
}
```

---

### `crates/famp/src/cli/mcp/error_kind.rs` (retarget, utility, request-response)

**Analog:** Current `crates/famp/src/cli/mcp/error_kind.rs` (exact pattern; retarget `BusErrorKind` instead of `CliError`)

**Exhaustive-match pattern to copy exactly** (lines 29-77 of current file):
```rust
// error_kind.rs (v0.9) — exhaustive match, no wildcard arm (MCP-10)
use famp_bus::BusErrorKind;

pub fn bus_error_to_jsonrpc(kind: BusErrorKind) -> (i64, &'static str) {
    // No `_ =>` arm. Adding a BusErrorKind variant fails compile here (MCP-10).
    let (code, kind_str) = match kind {
        BusErrorKind::NotRegistered       => (-32100, "not_registered"),
        BusErrorKind::NameTaken           => (-32101, "name_taken"),
        BusErrorKind::ChannelNameInvalid  => (-32102, "channel_name_invalid"),
        BusErrorKind::NotJoined           => (-32103, "not_joined"),
        BusErrorKind::EnvelopeInvalid     => (-32104, "envelope_invalid"),
        BusErrorKind::EnvelopeTooLarge    => (-32105, "envelope_too_large"),
        BusErrorKind::TaskNotFound        => (-32106, "task_not_found"),
        BusErrorKind::BrokerProtoMismatch => (-32107, "broker_proto_mismatch"),
        BusErrorKind::BrokerUnreachable   => (-32108, "broker_unreachable"),
        BusErrorKind::Internal            => (-32109, "internal"),
        // If a new BusErrorKind variant is added, THIS line fails to compile.
        // That is the intended behavior (MCP-10).
    };
    (code, kind_str)
}
```

**Companion exhaustive test** (copy `mcp_error_kind_exhaustive.rs` pattern — iterate `BusErrorKind::ALL`, assert unique codes):
```rust
// tests/bus_error_kind_exhaustive.rs — mirrors mcp_error_kind_exhaustive.rs
#[test]
fn every_bus_error_kind_has_jsonrpc_code() {
    use std::collections::HashSet;
    let mut codes = HashSet::new();
    for kind in famp_bus::BusErrorKind::ALL {
        let (code, kind_str) = bus_error_to_jsonrpc(kind);
        assert!(code < -32099, "code {code} must be in application range");
        assert!(!kind_str.is_empty());
        assert!(codes.insert(code), "duplicate code {code}");
    }
}
```

---

### `crates/famp/src/cli/mcp/tools/register.rs` (rewrite, controller, request-response)

**Analog:** Current `crates/famp/src/cli/mcp/tools/register.rs` (exact pattern; swap `resolve_identity_dir` for bus `Register` message)

**Tool call pattern to preserve** (lines 65-101):
```rust
// tools/register.rs (v0.9) — same input parsing, new backend call
pub async fn call(input: &Value) -> Result<Value, BusErrorKind> {
    let name = input["name"].as_str()
        .ok_or(BusErrorKind::EnvelopeInvalid)?.to_string();
    validate_identity_name(&name)?;

    // Connect bus + send Register (MCP process IS the identity; PID = self)
    session::ensure_bus().await?;
    let mut guard = session::state().lock().await;
    let bus = guard.bus.as_mut().expect("ensure_bus guarantees Some");
    let pid = std::process::id();
    let reply = bus.send_recv(BusMessage::Register { name: name.clone(), pid }).await
        .map_err(|_| BusErrorKind::BrokerUnreachable)?;
    match reply {
        BusReply::RegisterOk { drained, joined } => {
            guard.active_identity = Some(name.clone());
            Ok(serde_json::json!({ "active": name, "drained": drained, "peers": joined }))
        }
        BusReply::Err { kind, .. } => Err(kind),
        _ => Err(BusErrorKind::Internal),
    }
}
```

---

### `crates/famp/src/cli/mcp/tools/send.rs` (rewrite, controller, request-response)

**Analog:** Current `crates/famp/src/cli/mcp/tools/send.rs` (exact pattern — preserve input parsing shape; swap `run_at_structured(home, args)` for `bus.send_recv(BusMessage::Send{..})`)

**Input parsing pattern to preserve** (lines 30-103, send.rs):
```rust
// tools/send.rs (v0.9) — keep same input validation, new backend
pub async fn call(input: &Value) -> Result<Value, BusErrorKind> {
    // Input parsing identical to v0.8 (peer → to, mode, title, body, more_coming)
    let to_str = input["to"].as_str().ok_or(BusErrorKind::EnvelopeInvalid)?;
    let new_task = input["new_task"].as_str().map(str::to_string);
    // ...build BusEnvelope from new_task/task/terminal flags...
    let guard = session::state().lock().await;
    let bus = guard.bus.as_ref().ok_or(BusErrorKind::NotRegistered)?;
    let reply = bus.send_recv(BusMessage::Send {
        to: Target::Agent { name: to_str.to_string() },
        envelope: built_envelope_value,
        send_as: None,
    }).await.map_err(|_| BusErrorKind::BrokerUnreachable)?;
    match reply {
        BusReply::SendOk { task_id, delivered } =>
            Ok(serde_json::json!({ "task_id": task_id, "delivered": delivered })),
        BusReply::Err { kind, .. } => Err(kind),
        _ => Err(BusErrorKind::Internal),
    }
}
```

---

### `crates/famp/src/cli/mcp/tools/join.rs` + `tools/leave.rs` (new, controller, request-response)

**Analog:** Current `crates/famp/src/cli/mcp/tools/send.rs` (same call pattern; different message variant)

```rust
// tools/join.rs (new)
pub async fn call(input: &Value) -> Result<Value, BusErrorKind> {
    let channel = input["channel"].as_str().ok_or(BusErrorKind::EnvelopeInvalid)?.to_string();
    let channel = normalize_channel(&channel).map_err(|_| BusErrorKind::ChannelNameInvalid)?;
    // get bus from session::state()
    let reply = bus.send_recv(BusMessage::Join { channel: channel.clone() }).await?;
    match reply {
        BusReply::JoinOk { members, drained } =>
            Ok(serde_json::json!({ "channel": channel, "members": members, "drained": drained })),
        BusReply::Err { kind, .. } => Err(kind),
        _ => Err(BusErrorKind::Internal),
    }
}
```

---

### `crates/famp/src/cli/mcp/server.rs` (UNCHANGED)

**Analog:** Itself. The JSON-RPC loop in `server.rs` (lines 255-381) is preserved verbatim per D-04. The only changes are:
- `dispatch_tool` signature: remove `local_root: &Path` parameter
- Add `"famp_join"` and `"famp_leave"` arms to `dispatch_tool` match
- `tool_descriptors()`: add `famp_join` and `famp_leave` tool descriptors
- `cli_error_response` → `bus_error_response` (using `bus_error_to_jsonrpc`)

**Dispatch pattern to copy** (lines 355-381):
```rust
// server.rs dispatch_tool — same structure; add two arms
async fn dispatch_tool(name: &str, input: &serde_json::Value) -> Result<Value, BusErrorKind> {
    match name {
        "famp_register" => return tools::register::call(input).await,
        "famp_whoami"   => return tools::whoami::call(input).await,
        _ => {}
    }
    // Pre-registration gate (D-05): preserved from v0.8
    let identity = { session::state().lock().await.active_identity.clone() };
    if identity.is_none() { return Err(BusErrorKind::NotRegistered); }
    match name {
        "famp_send"  => tools::send::call(input).await,
        "famp_await" => tools::await_::call(input).await,
        "famp_inbox" => tools::inbox::call(input).await,
        "famp_peers" => tools::peers::call(input).await,
        "famp_join"  => tools::join::call(input).await,   // NEW
        "famp_leave" => tools::leave::call(input).await,  // NEW
        other => Err(BusErrorKind::Internal),
    }
}
```

---

### `crates/famp/src/bin/famp.rs` (extend, config)

**Analog:** Current `crates/famp/src/bin/famp.rs` + `crates/famp/src/cli/mod.rs`

**Commands enum extension pattern** (cli/mod.rs lines 33-56):
```rust
// cli/mod.rs — add new variants to Commands enum
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ... existing variants (Init, Setup, Info, Listen, Peer, Send, Await, Inbox, Mcp) ...
    /// Start (or connect to) the UDS broker daemon.
    Broker(broker::BrokerArgs),
    /// Register this terminal session as a named identity on the local bus.
    Register(register::RegisterArgs),
    /// Join a channel (#name).
    Join(join::JoinArgs),
    /// Leave a channel (#name).
    Leave(leave::LeaveArgs),
    /// List currently registered sessions.
    Sessions(sessions::SessionsArgs),
    /// Print the current registered identity.
    Whoami(whoami::WhoamiArgs),
}
```

**Tokio runtime dispatch pattern** (cli/mod.rs lines 66-124) — copy the `let rt = tokio::runtime::Builder::new_multi_thread()...` block for each new async command:
```rust
Commands::Broker(args) => {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()
        .map_err(|e| CliError::Io { path: std::path::PathBuf::new(), source: e })?;
    rt.block_on(broker::run(args))
}
Commands::Register(args) => {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()
        .map_err(|e| CliError::Io { path: std::path::PathBuf::new(), source: e })?;
    rt.block_on(register::run(args))
}
// ... same pattern for Join, Leave, Sessions, Whoami
```

**Unused-dep silencer pattern** (bin/famp.rs lines 7-37) — add `famp_bus as _;` and `nix as _;` to the silencer list.

---

### `scripts/famp-local` (hook additions, utility, event-driven)

**Analog:** `scripts/famp-local` `cmd_wire` (lines 734-812) + `wires_write_row`/`wires_remove_row` helpers (lines 109-131)

**`cmd_hook_add` pattern** (from RESEARCH.md §8 — copy verbatim):
```bash
cmd_hook_add() {
  local on="" to=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --on) on="$2"; shift 2 ;;
      --to) to="$2"; shift 2 ;;
      *) die "hook add: unknown argument '$1'" ;;
    esac
  done
  [ -n "$on" ] || die "hook add: --on is required"
  [ -n "$to" ] || die "hook add: --to is required"
  case "$on" in Edit:*) ;; *) die "hook add: --on must be 'Edit:<glob>'" ;; esac
  local id; id="h$(printf '%x' "$(date +%s)")$(head -c3 /dev/urandom | xxd -p)"
  local ts; ts="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u)"
  local hooks_file; hooks_file="$STATE_ROOT/hooks.tsv"
  mkdir -p "$(dirname "$hooks_file")"
  printf '%s\t%s\t%s\t%s\n' "$id" "$on" "$to" "$ts" >> "$hooks_file"
  printf 'hook added: id=%s on=%s to=%s\n' "$id" "$on" "$to"
}
```

**`cmd_hook_remove` atomic rewrite pattern** (mirrors `wires_remove_row` lines 122-131):
```bash
cmd_hook_remove() {
  [ $# -eq 1 ] || die "hook remove <id>"
  local id="$1"
  local f; f="$STATE_ROOT/hooks.tsv"
  [ -f "$f" ] || die "no hooks file found"
  local tmp; tmp="$(mktemp)"
  awk -F'\t' -v id="$id" '$1 != id' "$f" > "$tmp"
  if diff -q "$f" "$tmp" >/dev/null 2>&1; then
    rm "$tmp"; die "hook id '$id' not found"
  fi
  mv "$tmp" "$f"
  printf 'hook removed: %s\n' "$id"
}
```

**Dispatch routing addition** (mirrors `wire)` dispatch line 1216):
```bash
# In main case "$cmd" block:
hook)  cmd_hook "$@" ;;

cmd_hook() {
  local sub="${1:-help}"; shift || true
  case "$sub" in
    add)    cmd_hook_add "$@" ;;
    list)   cmd_hook_list ;;
    remove) cmd_hook_remove "$@" ;;
    *)      die "hook: unknown subcommand '$sub' (add|list|remove)" ;;
  esac
}
```

---

### `crates/famp/tests/broker_lifecycle.rs` (test, event-driven)

**Analog:** `crates/famp/tests/listen_smoke.rs` (in-process async server + tokio test pattern)

**In-process tokio spawn pattern** (listen_smoke.rs lines 29-86):
```rust
// broker_lifecycle.rs — in-process broker for idle timer tests
#[tokio::test(start_paused = true)]  // enables tokio::time::advance()
async fn broker_exits_after_5min_idle() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let broker_handle = tokio::spawn(run_broker(sock.clone()));

    // Connect one client and disconnect (triggers idle timer arm)
    {
        let _stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        // drop closes connection
    }

    // Fast-forward 5 min + 1s
    tokio::time::advance(Duration::from_secs(301)).await;
    tokio::task::yield_now().await; // allow broker task to process

    assert!(!sock.exists(), "broker must unlink socket on idle exit");
    assert!(broker_handle.is_finished() || matches!(
        tokio::time::timeout(Duration::from_millis(100), broker_handle).await,
        Ok(_)
    ));
}
```

**`start_paused = true` is NOT yet used anywhere in the repo** — this is the first precedent. Add `tokio = { workspace = true, features = ["test-util"] }` to `crates/famp/Cargo.toml [dev-dependencies]`.

---

### `crates/famp/tests/broker_spawn_race.rs` + `broker_crash_recovery.rs` (test, event-driven)

**Analog:** `crates/famp/tests/mcp_stdio_tool_calls.rs` (subprocess spawn + `Command::cargo_bin("famp")` pattern)

**Subprocess spawn pattern** (mcp_stdio_tool_calls.rs lines 72-88):
```rust
// broker_spawn_race.rs — two simultaneous famp register invocations
#[test]
fn two_simultaneous_register_invocations_produce_one_broker() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let env = [("FAMP_BUS_SOCKET", sock.to_str().unwrap())];

    let mut c1 = Command::cargo_bin("famp").unwrap()
        .envs(env).args(["register", "alice", "--no-reconnect"]).spawn().unwrap();
    let mut c2 = Command::cargo_bin("famp").unwrap()
        .envs(env).args(["register", "bob", "--no-reconnect"]).spawn().unwrap();

    std::thread::sleep(Duration::from_secs(2));

    let connect = std::os::unix::net::UnixStream::connect(&sock);
    assert!(connect.is_ok(), "one broker must be running");

    c1.kill().ok(); c2.kill().ok();
}
```

**`assert_cmd` pattern** (not yet in repo — RESEARCH.md §9 confirms it must be added to `[dev-dependencies]`):
```toml
# crates/famp/Cargo.toml [dev-dependencies]
assert_cmd = "2.0"
tokio = { workspace = true, features = ["...", "test-util"] }
```

---

### `crates/famp/tests/mcp_bus_e2e.rs` (test, request-response)

**Analog:** `crates/famp/tests/mcp_stdio_tool_calls.rs` (MCP harness pattern — McpHarness struct, send_msg/recv_msg helpers)

**Harness to adapt** (mcp_stdio_tool_calls.rs lines 54-238):
- Copy `send_msg` / `recv_msg` helpers verbatim (lines 24-50)
- Adapt `McpHarness::new()` to set `FAMP_BUS_SOCKET=$tmp/test-bus.sock` instead of `FAMP_LOCAL_ROOT`
- Two harness instances (alice_mcp, bob_mcp) share the same socket env var for bus isolation

```rust
// mcp_bus_e2e.rs harness shape
fn spawn_mcp_process(sock_path: &Path, name_for_log: &str) -> (Child, ChildStdin, ChildStdout) {
    let mut child = Command::cargo_bin("famp").unwrap()
        .args(["mcp"])
        .env("FAMP_BUS_SOCKET", sock_path)
        .env_remove("FAMP_HOME")
        .env_remove("FAMP_LOCAL_ROOT")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    (child, stdin, stdout)
}
```

---

## Shared Patterns

### Identity Resolution (D-01 chain)
**Source:** `crates/famp/src/cli/identity.rs` (new module — pattern in §"identity.rs" above)
**Apply to:** All non-register CLI subcommands: `send`, `inbox list`, `inbox ack`, `await`, `join`, `leave`, `sessions` (--me), `whoami`

```rust
// Every non-register CLI command opens with:
let identity = resolve_identity(args.send_as.as_deref())?;
```

### Hard-Error on NotRegistered
**Source:** `crates/famp/src/cli/error.rs` (existing pattern) + bus client reply handling
**Apply to:** All CLI subcommands that call `BusClient::send_recv`

```rust
// When BusReply::Err { kind: BusErrorKind::NotRegistered, .. }:
eprintln!("{} is not registered — start `famp register {}` in another terminal first",
    identity, identity);
std::process::exit(1);
```

### BusClient Dispatch Pattern
**Source:** `crates/famp/src/bus_client/mod.rs` (new)
**Apply to:** All CLI subcommands and MCP tools that talk to the broker

```rust
// Consistent one-shot connect+send_recv for short-lived commands:
let mut bus = BusClient::connect(&resolve_sock_path()).await
    .map_err(|_| CliError::NotRegistered)?;
let reply = bus.send_recv(msg).await?;
// map reply variants to Result<_, CliError>
```

### `run` / `run_at` / `run_at_structured` Three-Layer Pattern
**Source:** `crates/famp/src/cli/send/mod.rs` (lines 88-101) + `crates/famp/src/cli/await_cmd/mod.rs` (lines 85-132)
**Apply to:** All rewired CLI subcommands that have MCP tool wrappers

Every rewired command must expose:
1. `run(args) -> Result<(), CliError>` — production entry (resolves socket, prints to stdout)
2. `run_at(sock_path, args, out) -> Result<(), CliError>` — test-facing (injectable output)
3. `run_at_structured(sock_path, args) -> Result<XxxOutcome, CliError>` — for MCP tools

### `#[arg(long)]` Clap Flag Matrix
**Source:** `crates/famp/src/cli/send/mod.rs` (lines 42-64)
**Apply to:** All new CLI Args structs

```rust
// Copy --as flag on every non-register subcommand:
#[arg(long, name = "as")]
pub send_as: Option<String>,

// Copy conflicts_with / requires discipline:
#[arg(long, conflicts_with = "task")]
pub new_task: Option<String>,
#[arg(long, requires = "task")]
pub terminal: bool,
```

### JSONL stdout Output
**Source:** `crates/famp/src/cli/inbox/list.rs`
**Apply to:** `sessions list`, `inbox list`, `await` (single-line), `send` (result JSON)

```rust
// One serde_json::to_string(&row)? per println! — same pattern as inbox list.
for row in &rows {
    println!("{}", serde_json::to_string(row)?);
}
```

### Atomic Cursor Write
**Source:** `crates/famp-inbox/src/cursor.rs` (lines 58-91)
**Apply to:** `cli/broker/cursor_exec.rs` (executes `Out::AdvanceCursor`), `cli/inbox/ack.rs` (rewired)

Copy the `tempfile::NamedTempFile` + `sync_all` + `persist` + `0o600 chmod` pattern verbatim.

### Tokio Runtime Builder
**Source:** `crates/famp/src/cli/mod.rs` (lines 74-83, every async command dispatch)
**Apply to:** All new async subcommands in `cli/mod.rs`

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| CliError::Io { path: std::path::PathBuf::new(), source: e })?;
rt.block_on(new_cmd::run(args))
```

### MCP Tool Call Pattern
**Source:** `crates/famp/src/cli/mcp/tools/send.rs` (lines 30-110)
**Apply to:** All 8 MCP tool files (rewrite + new)

```rust
// Every tool:
// 1. Parse input fields with `.as_str().ok_or(BusErrorKind::EnvelopeInvalid)?`
// 2. Check session::state().active_identity (except register + whoami)
// 3. send_recv via session::state().bus
// 4. Match BusReply exhaustively → Ok(serde_json::json!({...})) or Err(BusErrorKind)
```

### Exhaustive Match Compile Gate
**Source:** `crates/famp/src/cli/mcp/error_kind.rs` (lines 29-77) + `crates/famp/tests/mcp_error_kind_exhaustive.rs`
**Apply to:** `crates/famp/src/cli/mcp/error_kind.rs` (retargeted), any CLI code matching `BusErrorKind`

```rust
// No `_ =>` arm anywhere BusErrorKind is matched.
// Companion test uses BusErrorKind::ALL constant to verify all 10 variants covered.
```

### Test File Header Pattern
**Source:** `crates/famp/tests/mcp_stdio_tool_calls.rs` (lines 1-20) + `tests/listen_smoke.rs` (lines 1-20)
**Apply to:** All new test files

```rust
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
use std::time::Duration;
// ... assert_cmd imports for shelled tests
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/famp/src/cli/broker/nfs_check.rs` | utility | request-response | No `statfs`/filesystem-type detection in repo; use RESEARCH.md §2 §"Item 3" verbatim |
| `crates/famp/src/bus_client/spawn.rs` | utility | event-driven | No `posix_spawn` + `setsid` in repo; use RESEARCH.md §5 verbatim |
| `crates/famp/tests/broker_lifecycle.rs` (`start_paused = true` tests) | test | event-driven | First use of `tokio::time::pause()`/`advance()` in repo; requires `test-util` feature added to `[dev-dependencies]` |

---

## Metadata

**Analog search scope:** `crates/famp/src/`, `crates/famp/tests/`, `crates/famp-bus/src/`, `crates/famp-inbox/src/`, `scripts/famp-local`
**Files scanned:** 45 Rust source files, 1 bash script (1230 LoC)
**Pattern extraction date:** 2026-04-28

---

## PATTERN MAPPING COMPLETE

**Phase:** 02 — UDS wire + CLI + MV-MCP rewire + hook subcommand
**Files classified:** 28
**Analogs found:** 25 / 28

### Coverage
- Files with exact analog: 12 (bus codec, send/inbox/await rewires, mcp session, error_kind, tools/register, tools/send, famp.rs, scripts/famp-local hook pattern)
- Files with role-match analog: 13 (bus_client/mod, broker/mod, mailbox_env, identity, register, join, leave, sessions, whoami, mcp tools join/leave, test files)
- Files with no analog: 3 (nfs_check.rs, spawn.rs, start_paused tests)

### Key Patterns Identified
- All CLI subcommands use the three-layer `run`/`run_at`/`run_at_structured` pattern — MCP tools call the structured form
- Every `BusErrorKind` consumer must use exhaustive match with no `_ =>` arm; `BusErrorKind::ALL` constant enables companion test
- BusClient codec wraps the existing sync `encode_frame`/`try_decode_frame` with `AsyncReadExt`/`AsyncWriteExt` — identical 4-byte BE length prefix
- Session state follows the `OnceLock<Mutex<SessionState>>` module-scope singleton pattern; v0.9 replaces `IdentityBinding` inner type with `{ bus: Option<BusClient>, active_identity: Option<String> }`
- Atomic cursor writes use `tempfile::NamedTempFile` + `sync_all` + `persist` + `0o600` — copy `famp-inbox/src/cursor.rs` lines 58-91 verbatim
- Hook bash additions follow `wires_write_row`/`wires_remove_row` TSV-with-awk pattern from same file; ~110 LoC addition

### File Created
`/Users/benlamm/Workspace/FAMP/.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-PATTERNS.md`

### Ready for Planning
Pattern mapping complete. Planner can now reference analog patterns in PLAN.md files.
