#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::too_many_lines)]
//! DIST-03 falsification pair: proves the shipped curl installer
//! (`crates/famp/tests/fixtures/installers/famp-installer.sh`, committed by
//! 16-01, kept honest by 16-02's `just check-installer-drift`) fails CLOSED
//! on a checksum-corrupted artifact, and that the rejection is specifically
//! attributable to checksum verification rather than some other failure
//! mode (a 404, a tar failure, ...).
//!
//! **The pair, by function name** (repo standing rule: "a falsification run
//! needs a control" — green under both the working and the broken state
//! carries zero information):
//!   - `installer_accepts_a_matching_artifact` — THE CONTROL. MUST STILL
//!     PASS if checksum verification were removed from the installer: it
//!     proves the harness isn't vacuously green (e.g. every download
//!     404ing would still "work" if verification were the only thing
//!     gating success).
//!   - `installer_rejects_a_corrupted_artifact_without_installing` — THE
//!     FALSIFICATION. MUST FAIL if checksum verification were removed: it
//!     asserts a non-zero exit, an empty install root (fail-CLOSED
//!     ordering), a checksum-mismatch token in the installer's own error
//!     output, AND — via a stripped installer copy with only the
//!     checksum-verification call removed — that the *same* corrupted
//!     artifact IS accepted once that check is gone. That inversion is what
//!     makes the rejection provably load-bearing rather than incidental.
//!
//! **D-06 boundary (do not exceed in any assertion, comment, or test name
//! here):** these tests prove the installer rejects an artifact whose bytes
//! do not match its published checksum. They do NOT prove the release
//! workflow itself was uncompromised — an attacker who can substitute the
//! archive can substitute the checksum beside it. Signing is the named
//! follow-up, not this gate.
//!
//! ## Deviation from the plan's assumed mechanism (recorded per Rule 3)
//!
//! 16-03-PLAN.md's `<behavior>` describes "a fabricated `.tar.xz` and a
//! `.sha256` whose digest matches it" as two separate downloaded artifacts.
//! Reading `famp-installer.sh` in full (as `<read_first>` required) shows
//! that is not how `dist` 0.32's shell installer actually verifies
//! checksums: there is no second HTTP fetch of a `.sha256` file anywhere in
//! this script. `verify_checksum()` (line ~1436, called from line ~299)
//! compares the downloaded archive's freshly-computed digest against a
//! `_checksum_value` that is baked directly into the installer's own
//! per-target `case` statement AT GENERATION TIME — confirmed by running
//! `dist build --artifacts=all --target aarch64-apple-darwin` locally,
//! which regenerates the installer with real lines
//! `_checksum_style="sha256"` / `_checksum_value="<64-hex>"` inserted right
//! after `_zip_ext=".tar.xz"` for the target that was actually built.
//!
//! The committed fixture at
//! `crates/famp/tests/fixtures/installers/famp-installer.sh` was generated
//! by 16-01 via `dist build --artifacts=global`, which builds no real
//! per-target archives and therefore has no real checksums to embed — its
//! three `case` blocks declare `_checksum_style`/`_checksum_value` as
//! `local` (line 220-221) but never assign them, so `verify_checksum` is
//! currently dead code in the committed fixture (`[ -n "${_checksum_style:-}" ]`
//! is always false, taking the "no checksums to verify" branch at line 301).
//!
//! This test file therefore builds its own **working copy** of the
//! installer: the committed fixture text, PLUS the two checksum lines
//! `dist` itself would have inserted for each of the three targets,
//! computed from this test's own fabricated "good" archives. This is
//! filling in a data gap the incomplete `--artifacts=global`-only
//! generation left behind — not a change to the verification *code path*
//! (`verify_checksum`'s logic is untouched) — so it stays inside the
//! must-have prohibition that the CONTROL may not have its checksum
//! verification code modified. `fixtures::assert_only_checksum_lines_added`
//! proves mechanically that the working copy differs from the committed
//! fixture ONLY by those six inserted lines (two per target x three
//! targets), each containing `_checksum_style` or `_checksum_value`.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// The three release targets this installer ships (D-02/D-05), read from
/// `famp-installer.sh`'s own `case "$_artifact_name" in` block rather than
/// assumed.
const TARGETS: [&str; 3] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
];

fn committed_installer_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/installers/famp-installer.sh")
}

fn artifact_name(target: &str) -> String {
    format!("famp-{target}.tar.xz")
}

// ---------------------------------------------------------------------
// Archive fabrication
// ---------------------------------------------------------------------

/// Builds a `.tar.xz` archive matching the internal layout
/// `famp-installer.sh` expects: a single top-level directory named after
/// the artifact (stripped by the installer's `tar xf --strip-components 1`)
/// containing one file, `famp`, holding `content`. Verified against a real
/// `dist build --artifacts=all` output (`famp-<target>/famp` + LICENSE/README
/// files the installer does not care about) — only the `famp` entry is
/// actually consulted, since `_bins="famp"` for every target in this repo.
fn fabricate_archive(work_dir: &Path, target: &str, content: &[u8]) -> Vec<u8> {
    let dir_name = format!("famp-{target}");
    let payload_root = work_dir.join(format!("payload-{target}-{}", content.len()));
    let inner_dir = payload_root.join(&dir_name);
    fs::create_dir_all(&inner_dir).expect("create archive payload dir");
    let bin_path = inner_dir.join("famp");
    fs::write(&bin_path, content).expect("write fixture binary stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))
            .expect("chmod fixture binary stub");
    }

    let archive_path = payload_root.join("out.tar.xz");
    let status = Command::new("tar")
        .arg("-cJf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&payload_root)
        .arg(&dir_name)
        .status()
        .expect("spawn tar to build fixture archive");
    assert!(
        status.success(),
        "tar -cJf failed building fixture archive for {target}"
    );

    fs::read(&archive_path).expect("read fabricated archive bytes")
}

/// One byte of `content` flipped (XOR 0xFF, which always changes the byte).
/// Applied to the PRE-compression payload, not the compressed bytes: a
/// single flipped byte inside an xz stream frequently corrupts the LZMA2
/// container itself and makes `tar xf` fail outright, which would make the
/// stripped-installer inversion assertion (phase 3) fail for the wrong
/// reason (extraction failure, not "checksum verification would have
/// caught this but didn't"). Flipping pre-compression and re-compressing
/// fresh yields a fully valid, independently extractable archive that
/// differs from the good one by exactly one content byte — so checksum
/// verification, not tar, is what has to catch it.
fn flip_one_byte(content: &[u8]) -> Vec<u8> {
    let mut corrupted = content.to_vec();
    let idx = corrupted.len() / 2;
    corrupted[idx] ^= 0xFF;
    assert_ne!(
        corrupted, content,
        "byte flip must actually change the content"
    );
    corrupted
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------
// Installer text patching
// ---------------------------------------------------------------------

/// Inserts `_checksum_style="sha256"` / `_checksum_value="<hex>"` right
/// after the `_zip_ext=".tar.xz"` line inside `target`'s case block —
/// exactly where a real `dist build --artifacts=all` run puts them
/// (confirmed against a locally-built `target/distrib/famp-installer.sh`
/// during plan execution). Panics loudly if the installer's shape has
/// drifted (the marker not found) rather than silently no-op-ing.
fn insert_checksum(installer_src: &str, target: &str, checksum_hex: &str) -> (String, String) {
    let marker = format!(
        "\"{}\")\n            _arch=\"{target}\"\n            _zip_ext=\".tar.xz\"\n",
        artifact_name(target)
    );
    let idx = installer_src.find(&marker).unwrap_or_else(|| {
        panic!(
            "could not find the {target} case block in famp-installer.sh — the \
             installer's shape has drifted from what this test's insertion point assumes"
        )
    });
    let insert_at = idx + marker.len();
    let insertion = format!(
        "            _checksum_style=\"sha256\"\n            _checksum_value=\"{checksum_hex}\"\n"
    );
    let mut patched = String::with_capacity(installer_src.len() + insertion.len());
    patched.push_str(&installer_src[..insert_at]);
    patched.push_str(&insertion);
    patched.push_str(&installer_src[insert_at..]);
    (patched, insertion)
}

/// Proves the working copy differs from the committed fixture ONLY by the
/// exact checksum-line snippets this test inserted — no other line was
/// touched, and in particular `verify_checksum`'s own logic is
/// byte-identical. Removing each recorded insertion string exactly once
/// from the working copy must reproduce the committed installer exactly
/// (a substring-removal reconstruction, not a generic line filter — a
/// generic "drop any line mentioning `_checksum_style`/`_checksum_value`"
/// filter would also strip the pre-existing `local _checksum_style` /
/// `verify_checksum()` lines that were already in the committed fixture,
/// which is not what this is checking).
fn assert_only_checksum_lines_added(committed: &str, working: &str, insertions: &[String]) {
    let mut reconstructed = working.to_string();
    for insertion in insertions {
        assert!(
            reconstructed.contains(insertion.as_str()),
            "expected inserted checksum snippet not found in working installer copy: {insertion:?}"
        );
        reconstructed = reconstructed.replacen(insertion.as_str(), "", 1);
    }
    assert_eq!(
        reconstructed, committed,
        "working installer copy must equal the committed fixture once every recorded \
         checksum-line insertion is removed — any remaining difference means this test \
         modified something beyond the checksum data gap"
    );
}

/// The falsification's phase-3 CONTROL-on-removal: neutralizes the single
/// `verify_checksum "$_file" ...` call site so the installer proceeds
/// straight to `tar xf` regardless of checksum outcome. This is the ONLY
/// line this function may change — asserted by `assert_only_verify_call_changed`.
fn strip_checksum_verification(working_installer_src: &str) -> String {
    let call_line =
        "            verify_checksum \"$_file\" \"$_checksum_style\" \"$_checksum_value\"";
    assert!(
        working_installer_src.contains(call_line),
        "verify_checksum call site not found at the expected indentation — installer shape drifted"
    );
    let stripped_line =
        "            true # DIST-03 falsification control: verify_checksum call stripped";
    working_installer_src.replacen(call_line, stripped_line, 1)
}

/// Proves the stripped copy differs from the working copy in EXACTLY one
/// line: the `verify_checksum` call site. Line-for-line zip comparison is
/// valid here (unlike the checksum-insertion diff) because `replacen`
/// preserves line count — no lines were added or removed.
fn assert_only_verify_call_changed(working: &str, stripped: &str) {
    let working_lines: Vec<&str> = working.lines().collect();
    let stripped_lines: Vec<&str> = stripped.lines().collect();
    assert_eq!(
        working_lines.len(),
        stripped_lines.len(),
        "stripping the checksum-verification call must not add or remove lines"
    );
    let diffs: Vec<usize> = working_lines
        .iter()
        .zip(stripped_lines.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs,
        vec![working_lines
            .iter()
            .position(|l| l.contains("verify_checksum \"$_file\""))
            .expect("working copy must still contain the verify_checksum call site")],
        "stripping checksum verification must change exactly the verify_checksum call line, \
         nothing else — working diffs at {diffs:?}"
    );
    let changed = diffs[0];
    assert!(working_lines[changed].contains("verify_checksum"));
    assert!(stripped_lines[changed].contains("verify_checksum"));
}

// ---------------------------------------------------------------------
// Hermetic local artifact server: 127.0.0.1:0, never a hardcoded port.
// ---------------------------------------------------------------------

/// Minimal hand-rolled HTTP/1.1 static-file responder over
/// `std::net::TcpListener`. No new crate dependency (`git diff -- crates/famp/Cargo.toml`
/// is empty for this plan) — this repo already depends on nothing heavier
/// for a one-request-per-test fixture. Binds port 0 and reads back the
/// OS-assigned port via `local_addr()` (T-16-09: a hardcoded port in the
/// ephemeral range is a recorded ubuntu-only CI flake in this repo).
struct FixtureServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl FixtureServer {
    fn start(artifacts: HashMap<String, Vec<u8>>) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind fixture server to 127.0.0.1:0");
        listener
            .set_nonblocking(true)
            .expect("set fixture listener nonblocking");
        let addr = listener
            .local_addr()
            .expect("read fixture server local_addr");
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_thread = Arc::clone(&shutdown);
        let handle = thread::spawn(move || loop {
            if shutdown_for_thread.load(Ordering::SeqCst) {
                return;
            }
            match listener.accept() {
                Ok((stream, _)) => serve_one(stream, &artifacts),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        });
        Self {
            addr,
            shutdown,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

/// RAII guard: signals the accept-loop thread to stop and joins it on drop
/// (including panic-driven unwind), mirroring this repo's `ChildGuard`
/// convention (crates/famp/tests/common/child_guard.rs) of never leaving
/// spawned state behind after a failure.
impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn serve_one(mut stream: TcpStream, artifacts: &HashMap<String, Vec<u8>>) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }
    // Drain headers up to the blank line; we don't need them.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
        }
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .trim_start_matches('/')
        .to_string();

    if let Some(bytes) = artifacts.get(&path) {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(bytes);
    } else {
        let body = format!("fixture server: no artifact registered for {path}");
        let header = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body.as_bytes());
    }
}

// ---------------------------------------------------------------------
// Installer runner
// ---------------------------------------------------------------------

/// Runs `installer_src` under `sh`, with `CARGO_HOME` and `HOME` both
/// redirected to fresh temp dirs (this is not optional — an unredirected
/// run overwrites the developer's real `~/.cargo/bin/famp`, T-16-10), and
/// the download-base-URL override (`FAMP_DOWNLOAD_URL`, the
/// highest-priority override per famp-installer.sh line 28, confirmed by
/// 16-01's summary) pointed at the local fixture server.
fn run_installer(installer_path: &Path, cargo_home: &Path, home: &Path, base_url: &str) -> Output {
    Command::new("sh")
        .arg(installer_path)
        .env("CARGO_HOME", cargo_home)
        .env("HOME", home)
        .env("FAMP_DOWNLOAD_URL", base_url)
        .env("FAMP_NO_MODIFY_PATH", "1")
        .output()
        .expect("failed to spawn famp-installer.sh under sh")
}

// ---------------------------------------------------------------------
// Fixture assembly shared by both tests
// ---------------------------------------------------------------------

struct Fixtures {
    working_installer_src: String,
    stripped_installer_src: String,
    good_archives: HashMap<String, Vec<u8>>,
    corrupted_archives: HashMap<String, Vec<u8>>,
    // Kept alive for the duration of the test: archive fabrication writes
    // scratch payload files under here before tar-ing them into memory.
    _scratch: TempDir,
}

fn build_fixtures() -> Fixtures {
    let committed = fs::read_to_string(committed_installer_path())
        .expect("read committed famp-installer.sh fixture");
    let scratch = TempDir::new().expect("scratch tempdir for archive fabrication");

    let mut good_archives = HashMap::new();
    let mut corrupted_archives = HashMap::new();
    let mut working = committed.clone();
    let mut insertions: Vec<String> = Vec::new();

    for target in TARGETS {
        let good_content = format!("#!/bin/sh\necho famp-fixture-stub-{target}\n").into_bytes();
        let corrupted_content = flip_one_byte(&good_content);

        let good_bytes = fabricate_archive(scratch.path(), target, &good_content);
        let corrupted_bytes = fabricate_archive(
            &scratch.path().join("corrupted"),
            target,
            &corrupted_content,
        );
        assert_ne!(
            good_bytes, corrupted_bytes,
            "corrupted archive must differ from the good archive for {target}"
        );

        let checksum = sha256_hex(&good_bytes);
        let (patched, insertion) = insert_checksum(&working, target, &checksum);
        working = patched;
        insertions.push(insertion);

        good_archives.insert(artifact_name(target), good_bytes);
        corrupted_archives.insert(artifact_name(target), corrupted_bytes);
    }

    assert_eq!(
        insertions.len(),
        TARGETS.len(),
        "one insertion recorded per target"
    );
    assert_only_checksum_lines_added(&committed, &working, &insertions);

    let stripped = strip_checksum_verification(&working);
    assert_only_verify_call_changed(&working, &stripped);

    Fixtures {
        working_installer_src: working,
        stripped_installer_src: stripped,
        good_archives,
        corrupted_archives,
        _scratch: scratch,
    }
}

fn write_installer(dir: &Path, name: &str, src: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, src).expect("write installer script to tempdir");
    path
}

// ---------------------------------------------------------------------
// The pair
// ---------------------------------------------------------------------

/// THE CONTROL — MUST STILL PASS if checksum verification were removed
/// from the installer. Serves an uncorrupted archive whose bytes match the
/// checksum baked into the working installer copy; the installer must
/// accept it and the binary must land in `$CARGO_HOME/bin`. Without this
/// test, a "rejects a bad checksum" test alone could pass for the wrong
/// reason (e.g. every download 404ing) and would stay green even if
/// verification were deleted — this is what makes that scenario visible.
#[test]
fn installer_accepts_a_matching_artifact() {
    let fixtures = build_fixtures();
    let install_scripts_dir = TempDir::new().expect("installer scripts tempdir");
    let installer_path = write_installer(
        install_scripts_dir.path(),
        "famp-installer-working.sh",
        &fixtures.working_installer_src,
    );

    let server = FixtureServer::start(fixtures.good_archives);
    let cargo_home = TempDir::new().expect("CARGO_HOME tempdir");
    let home = TempDir::new().expect("HOME tempdir");

    let output = run_installer(
        &installer_path,
        cargo_home.path(),
        home.path(),
        &server.base_url(),
    );

    assert!(
        output.status.success(),
        "installer must exit 0 for a matching artifact: status={:?} stdout={} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let installed_bin = cargo_home.path().join("bin").join("famp");
    assert!(
        installed_bin.exists(),
        "matching artifact must be installed at $CARGO_HOME/bin/famp: {}",
        installed_bin.display()
    );
}

/// THE FALSIFICATION — MUST FAIL if checksum verification were removed
/// from the installer. Three phases:
///   1. Serve a corrupted archive (one byte flipped pre-compression, so it
///      remains independently valid/extractable) under the checksum the
///      working installer expects the GOOD archive to have. Assert
///      non-zero exit.
///   2. Assert `$CARGO_HOME/bin/famp` does NOT exist (fail-CLOSED: rejected
///      before install, not installed-then-complained-about) and that the
///      installer's own stderr names the mismatch.
///   3. Run the SAME corrupted archive against the stripped installer copy
///      (checksum-verification call removed, and only that line — see
///      `assert_only_verify_call_changed`) and assert it now exits 0 WITH
///      the corrupted binary installed. That inversion is what proves
///      phase 1's rejection was actually caused by checksum verification,
///      not some other failure mode.
///
/// D-06 boundary: this proves the installer rejects bytes that don't match
/// their published checksum. It does not, and this test must not claim to,
/// prove the release pipeline that produced the checksum was uncompromised.
#[test]
fn installer_rejects_a_corrupted_artifact_without_installing() {
    let fixtures = build_fixtures();
    let install_scripts_dir = TempDir::new().expect("installer scripts tempdir");
    let working_installer_path = write_installer(
        install_scripts_dir.path(),
        "famp-installer-working.sh",
        &fixtures.working_installer_src,
    );
    let stripped_installer_path = write_installer(
        install_scripts_dir.path(),
        "famp-installer-stripped.sh",
        &fixtures.stripped_installer_src,
    );

    // --- Phase 1 + 2: the working installer must reject the corrupted
    // artifact and fail CLOSED. ---
    let server = FixtureServer::start(fixtures.corrupted_archives.clone());
    let cargo_home = TempDir::new().expect("CARGO_HOME tempdir");
    let home = TempDir::new().expect("HOME tempdir");

    let rejecting_output = run_installer(
        &working_installer_path,
        cargo_home.path(),
        home.path(),
        &server.base_url(),
    );

    assert!(
        !rejecting_output.status.success(),
        "installer must exit non-zero for a checksum-corrupted artifact: stdout={} stderr={}",
        String::from_utf8_lossy(&rejecting_output.stdout),
        String::from_utf8_lossy(&rejecting_output.stderr),
    );
    let installed_bin = cargo_home.path().join("bin").join("famp");
    assert!(
        !installed_bin.exists(),
        "fail-CLOSED violation: a checksum-rejected artifact must not be installed at {}",
        installed_bin.display()
    );
    let rejecting_stderr = String::from_utf8_lossy(&rejecting_output.stderr);
    let rejecting_stdout = String::from_utf8_lossy(&rejecting_output.stdout);
    assert!(
        rejecting_stderr.contains("checksum mismatch")
            || rejecting_stdout.contains("checksum mismatch"),
        "rejection must name the checksum mismatch (installer's own verify_checksum error \
         string), not just exit non-zero for an unrelated reason: stdout={rejecting_stdout} \
         stderr={rejecting_stderr}"
    );
    drop(server);

    // --- Phase 3: the SAME corrupted artifact against the stripped
    // installer copy must be ACCEPTED — proving phase 1's rejection was
    // caused by checksum verification specifically. ---
    let server = FixtureServer::start(fixtures.corrupted_archives);
    let cargo_home = TempDir::new().expect("CARGO_HOME tempdir (stripped run)");
    let home = TempDir::new().expect("HOME tempdir (stripped run)");

    let accepting_output = run_installer(
        &stripped_installer_path,
        cargo_home.path(),
        home.path(),
        &server.base_url(),
    );

    assert!(
        accepting_output.status.success(),
        "stripped installer (checksum verification removed) must accept the corrupted \
         artifact — if it does not, phase 1's rejection is not attributable to checksum \
         verification: stdout={} stderr={}",
        String::from_utf8_lossy(&accepting_output.stdout),
        String::from_utf8_lossy(&accepting_output.stderr),
    );
    let stripped_installed_bin = cargo_home.path().join("bin").join("famp");
    assert!(
        stripped_installed_bin.exists(),
        "stripped installer must have installed the corrupted binary at {}",
        stripped_installed_bin.display()
    );
}
