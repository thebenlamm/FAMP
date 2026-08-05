//! Shared two-host gateway test harness.
//!
//! Mechanically extracted (11-04 Task 2, no behavioral change) out of
//! `e2e_cross_host_delivery.rs` so a second e2e test file
//! (`e2e_shipping_surface.rs`) can stand up the same isolated
//! broker+gateway-pair-over-loopback-HTTPS topology without copy-pasting
//! it. Every function here is byte-identical in behavior to its prior
//! in-file version — only visibility (`pub`) and location changed.
//!
//! Include via `#[path = "common/gateway_harness.rs"] mod gateway_harness;`
//! (mirrors the `common/child_guard.rs` convention already used by this
//! test package). `#![allow(dead_code)]` because not every consumer uses
//! every helper.

#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::too_many_arguments
)]

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use famp_inspect_proto::IdentityListReply;

#[path = "child_guard.rs"]
mod child_guard;
pub use child_guard::ChildGuard;

/// Domain-qualified principal for the REAL identity registered on side A.
pub const ALICE: &str = "agent:hosta.test/alice";
/// Domain-qualified principal for the REAL identity registered on side B.
pub const BOB: &str = "agent:hostb.test/bob";
/// Bare domain segments (must match the `--peer` map + the envelope's own
/// `from_domain`/`to_domain` derivation via `Principal::authority()`).
pub const ALICE_DOMAIN: &str = "hosta.test";
pub const BOB_DOMAIN: &str = "hostb.test";

/// Bounded poll deadline shared by every convergence assertion in this
/// harness — generous enough to absorb subprocess startup + egress
/// `Await` polling (~1s cadence, `egress.rs::AWAIT_TIMEOUT_MS`) without
/// being unbounded.
pub const POLL_DEADLINE: Duration = Duration::from_secs(20);

/// Bounded deadline for *subprocess startup* waits (`wait_for_broker_socket`,
/// `wait_until_live`, `wait_for_https`).
///
/// Deliberately much larger than the ~1s a broker needs when it has the box to
/// itself: CI (and `cargo test -p famp-gateway`) runs test *binaries* in
/// parallel, so several two-host harnesses can be spawning brokers, registers
/// and gateways at the same moment. Under that contention a 5s budget is not
/// enough and the suite fails with "broker socket ... never came up" even
/// though nothing is actually broken — observed 2026-07-28 running this suite
/// alongside `gateway_setup_doc_accuracy`, green in isolation.
///
/// Polling is 50-100ms, so a fast startup still returns immediately; this only
/// raises the ceiling before a genuine hang is declared.
pub const STARTUP_DEADLINE: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------
// Subprocess + harness plumbing (Pattern A, liveness.rs)
// ---------------------------------------------------------------------

/// `Command::cargo_bin("famp")` resolves the sibling `famp` binary via
/// assert_cmd's `legacy_cargo_bin` fallback rather than the
/// `CARGO_BIN_EXE_famp` env var, which Cargo does not propagate across
/// package boundaries. Build famp's bin explicitly first so this test is
/// hermetic regardless of invocation order (identical rationale to
/// `liveness.rs::ensure_famp_bin_built`). `famp-gateway`'s own `[[bin]]`
/// is built automatically as a side effect of `cargo test -p famp-gateway`
/// building this same package's test target.
pub fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

/// `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` — the
/// shared TLS fixture cert pair every cross-host gateway test reuses
/// (09-PATTERNS.md). TLS here is channel encryption only (D-08); the
/// Ed25519 envelope signature is the real trust boundary.
pub fn cross_machine_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("famp")
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}

/// OS-assigned free loopback port. Bind-then-drop before the port is
/// handed to a spawned `famp-gateway` subprocess's own `--listen`
/// bind — a small, standard TOCTOU window this codebase already accepts
/// elsewhere for ephemeral-port test setups.
pub fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// One side's isolated broker + `FAMP_HOME`. A single `TempDir` serves
/// BOTH isolation axes (09-RESEARCH.md §7): `--socket` for bus/mailbox
/// isolation, `FAMP_HOME` for the gateway's identity/peers-keyring
/// isolation.
pub struct Side {
    tmp: tempfile::TempDir,
}

impl Side {
    pub fn new() -> Self {
        Self {
            tmp: tempfile::TempDir::new().expect("tempdir"),
        }
    }

    pub fn sock(&self) -> PathBuf {
        self.tmp.path().join("bus.sock")
    }

    pub fn home(&self) -> &Path {
        self.tmp.path()
    }
}

impl Default for Side {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a one-shot `famp <args>` subprocess against `sock`/`home`,
/// returning its captured output. Sets `FAMP_BUS_SOCKET`, `FAMP_HOME`,
/// and `HOME` so every identity-resolution / mailbox / peer-keyring path
/// stays inside this side's isolated tempdir.
pub fn famp_cmd(sock: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", sock)
        .env("FAMP_HOME", home)
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap()
}

/// Spawn a long-lived `famp <args>` subprocess (e.g. `broker`,
/// `register`), ChildGuard-wrapped so a panicking test still reaps it
/// (project ChildGuard convention — leaked tmp-socket brokers/holders
/// respawn).
pub fn famp_spawn(sock: &Path, home: &Path, args: &[&str]) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", sock)
            .env("FAMP_HOME", home)
            .env("HOME", home)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

pub fn spawn_broker(side: &Side) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .arg("broker")
            .arg("--socket")
            .arg(side.sock())
            .env("FAMP_HOME", side.home())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Poll (bounded) until the broker's UDS socket accepts a raw connection —
/// never a fixed `sleep()`-then-assert (07-RESEARCH.md Pitfall 4).
pub fn wait_for_broker_socket(sock: &Path, deadline: Duration) {
    let start = Instant::now();
    loop {
        if std::os::unix::net::UnixStream::connect(sock).is_ok() {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "broker socket at {} never came up within {deadline:?}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn spawn_register(side: &Side, name: &str) -> ChildGuard {
    famp_spawn(&side.sock(), side.home(), &["register", name])
}

/// `famp inspect identities --json` against `sock`, or `None` if the call
/// failed (broker not yet up / transient). Callers poll on `None`.
pub fn live_identity_names(sock: &Path, home: &Path) -> Option<Vec<String>> {
    let out = famp_cmd(sock, home, &["inspect", "identities", "--json"]);
    if !out.status.success() {
        return None;
    }
    let list: IdentityListReply = serde_json::from_slice(&out.stdout).ok()?;
    Some(list.rows.into_iter().map(|r| r.name).collect())
}

/// Poll (bounded) until `name` appears among the live identities on
/// `sock` — used for both real `famp register` holders and gateway-backed
/// proxy names, since both are plain live registrations from the
/// broker's point of view.
pub fn wait_until_live(sock: &Path, home: &Path, name: &str, deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(live) = live_identity_names(sock, home) {
            if live.iter().any(|l| l == name) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for '{name}' to appear live on {}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `famp peer export --as <principal>` against `home`, returning the
/// captured one-line export blob.
pub fn peer_export(home: &Path, principal: &str) -> String {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_HOME", home)
        .args(["peer", "export", "--as", principal])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "famp peer export --as {principal} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// `famp peer import <file>` against `home`, TOFU-pinning `blob`'s
/// principal + pubkey into this side's gateway peers keyring. Written to
/// a temp file rather than piped over stdin — simpler subprocess
/// plumbing, same on-disk effect.
pub fn peer_import(home: &Path, blob: &str) {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), blob).unwrap();
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_HOME", home)
        .args(["peer", "import"])
        .arg(file.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "famp peer import failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Spawn a `famp-gateway` subprocess on `side`, backing the bare local
/// name `backed_name` (the REMOTE principal this gateway fronts on its
/// own bus, D-01), listening on `127.0.0.1:<listen_port>` with the
/// `{own_cert_stub}.{crt,key}` fixture pair, relaying to
/// `{peer_domain}=https://127.0.0.1:<peer_port>`, and trusting the
/// `{trust_cert_stub}.crt` fixture cert for its outbound TLS client.
///
/// 17-03/D-30 deviation: `own_domain` is a REQUIRED parameter, set via
/// the `FAMP_OWN_DOMAIN` env var on the spawned process. `famp-gateway`
/// has no `--domain` CLI flag of its own — `resolve_own_domain_or_exit`
/// always resolves against `(None, home)`, so `FAMP_OWN_DOMAIN` (or a
/// `$FAMP_HOME/own-domain` file) is the only viable mechanism from a test
/// harness. Own-domain-unset is now UNCONDITIONALLY startup-fatal for
/// every gateway (not merely relay-reachable ones), so every caller of
/// this fn must supply a real value — there is no more "leave it unset"
/// option, even for the loopback/test topology.
pub fn spawn_gateway(
    side: &Side,
    backed_name: &str,
    listen_port: u16,
    own_cert_stub: &str,
    peer_domain: &str,
    peer_port: u16,
    trust_cert_stub: &str,
    own_domain: &str,
) -> ChildGuard {
    let fixtures = cross_machine_fixture_dir();
    ChildGuard::new(
        Command::cargo_bin("famp-gateway")
            .unwrap()
            .arg("--socket")
            .arg(side.sock())
            .arg("--listen")
            .arg(format!("127.0.0.1:{listen_port}"))
            .arg("--tls-cert")
            .arg(fixtures.join(format!("{own_cert_stub}.crt")))
            .arg("--tls-key")
            .arg(fixtures.join(format!("{own_cert_stub}.key")))
            .arg("--peer")
            .arg(format!("{peer_domain}=https://127.0.0.1:{peer_port}"))
            .arg("--trust-cert")
            .arg(fixtures.join(format!("{trust_cert_stub}.crt")))
            .env("FAMP_HOME", side.home())
            .env("FAMP_OWN_DOMAIN", own_domain)
            .arg(backed_name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Poll (bounded) until the HTTPS listener completes a real TLS request.
///
/// A plaintext TCP connect probe is too weak a readiness proof here: it
/// succeeds as soon as the socket reaches LISTEN state, which the kernel
/// grants before the process has loaded its rustls config, started the TLS
/// accept loop, or mounted the axum router. This probe instead requests an
/// intentionally-unmounted path and accepts ANY HTTP response (normally
/// 404), which proves all four.
///
/// Do NOT name the raw std connect API in this file, even inside a comment:
/// the D-05 readiness guard in `e2e_ci_gate_guard.rs` greps this harness's
/// source text for it and cannot distinguish a comment from a call.
///
/// Load-bearing for the reason the old `wait_for_tcp` already documented:
/// `run_egress`'s `Await` drain ADVANCES the mailbox read cursor even on a
/// failed relay POST (no re-queue on error), so a message drained before the
/// peer listener can actually serve HTTPS is silently lost, not retried.
///
/// NOTE (2026-08-04): this probe is a strictly stronger readiness proof, but
/// it was NOT shown to be the cause of the `e2e_shipping_surface` section-4
/// timeout — see `.planning/debug/gateway-shipping-e2e-timeout.md`. Do not
/// cite it as that fix.
pub fn wait_for_https(addr: SocketAddr, own_cert_stub: &str, deadline: Duration) {
    let cert = cross_machine_fixture_dir().join(format!("{own_cert_stub}.crt"));
    let tls = famp_transport_http::tls::build_client_config(Some(&cert))
        .expect("build trusted TLS readiness client config");
    let url = format!("https://{addr}/__famp_gateway_readiness");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build HTTPS readiness runtime");
    let client = runtime.block_on(async {
        reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .timeout(Duration::from_secs(2))
            .http1_only()
            .build()
            .expect("build HTTPS readiness client")
    });
    let start = Instant::now();
    loop {
        if runtime
            .block_on(async { client.get(&url).send().await })
            .is_ok()
        {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "gateway HTTPS listener at {addr} never completed a TLS request within {deadline:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

// ---------------------------------------------------------------------
// Assertions — poll-with-deadline, never a fixed sleep.
// ---------------------------------------------------------------------

/// Poll `famp inbox list --as <as_name>` (bounded) until a JSONL line
/// with `class == class_wanted` and (`id` OR `causality.ref`) ==
/// `task_id` appears. Proves GW-01/GW-02: the envelope actually reached
/// this side's real local mailbox, not just "the gateway said 202".
pub fn poll_inbox_contains(
    sock: &Path,
    home: &Path,
    as_name: &str,
    task_id: &str,
    class_wanted: &str,
    deadline: Duration,
) {
    let start = Instant::now();
    loop {
        let out = famp_cmd(sock, home, &["inbox", "list", "--as", as_name]);
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                // Phase 14 (D-04/D-05): `famp inbox list` JSONL lines are
                // now `{"origin": ..., "envelope": {...}}` rather than
                // the bare envelope — the CLI is one of the seven
                // mechanical rendering surfaces this phase tags. Read
                // envelope fields from the nested `"envelope"` object.
                let env = v.get("envelope").unwrap_or(&v);
                let matches_id = env.get("id").and_then(serde_json::Value::as_str) == Some(task_id)
                    || env
                        .get("causality")
                        .and_then(|c| c.get("ref"))
                        .and_then(serde_json::Value::as_str)
                        == Some(task_id);
                let matches_class =
                    env.get("class").and_then(serde_json::Value::as_str) == Some(class_wanted);
                if matches_id && matches_class {
                    return;
                }
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for class={class_wanted} task={task_id} \
             in {as_name}'s inbox on {}; last stdout: {}",
            sock.display(),
            String::from_utf8_lossy(&out.stdout)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Poll `famp inspect tasks --id <task_id> --json` (bounded) until the
/// LAST envelope in the returned chain shows a terminal `fsm_transition`
/// (`COMPLETED`/`FAILED`/`CANCELLED`), returning it. GW-03's proof point.
pub fn poll_terminal_state(sock: &Path, home: &Path, task_id: &str, deadline: Duration) -> String {
    let start = Instant::now();
    loop {
        let out = famp_cmd(sock, home, &["inspect", "tasks", "--id", task_id, "--json"]);
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(state) = v
                    .get("envelopes")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|envs| envs.last())
                    .and_then(|last| last.get("fsm_transition"))
                    .and_then(serde_json::Value::as_str)
                {
                    if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
                        return state.to_string();
                    }
                }
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for a terminal fsm_transition on task \
             {task_id} via {}; last stdout: {}",
            sock.display(),
            String::from_utf8_lossy(&out.stdout)
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}
