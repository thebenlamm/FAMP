//! Integration tests for the FAMP_HOME → register-first migration in
//! `scripts/famp-local`. Drives bash via `Command::new` for shell-portable
//! CI coverage. Spec: 01-CONTEXT.md "Migration in scripts/famp-local".

#![allow(
    unused_crate_dependencies,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown
)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .to_path_buf()
}

fn script_path() -> PathBuf {
    workspace_root().join("scripts/famp-local")
}

/// Place a stub `famp` binary on a synthetic PATH so `cmd_wire`'s
/// internal calls to `famp setup` / `famp listen` / `famp info` /
/// `famp peer import` all silently succeed without touching the real
/// workspace state.
///
/// The `listen` sub-command actually binds the port advertised in
/// `$FAMP_HOME/config.toml` via Python so `start_daemon`'s `lsof`
/// readiness check passes.  Python exits after 10 s (well after the
/// test finishes) so it doesn't leak indefinitely.
fn make_stub_famp(dir: &Path) -> PathBuf {
    let bin_dir = dir.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let stub = bin_dir.join("famp");
    // Note: the listen branch reads the port from config.toml using awk
    // (matching port_of() in famp-local) then binds it via Python3.
    // The Python process is backgrounded so the stub exits 0 immediately,
    // letting nohup return and the script record the PID.
    std::fs::write(
        &stub,
        "#!/usr/bin/env bash\n\
         case \"$1\" in\n\
           listen)\n\
             PORT=$(awk -F'\"' '/listen_addr/ {print $2}' \"$FAMP_HOME/config.toml\" | awk -F: '{print $NF}')\n\
             python3 -c \"import socket,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1); s.bind(('127.0.0.1',int('$PORT'))); s.listen(1); time.sleep(10)\" &\n\
             exit 0\n\
             ;;\n\
           *) exit 0 ;;\n\
         esac\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    stub
}

/// Pre-create `local_root/agents/<name>/` with the files that make
/// `ensure_agent` treat the agent as "already initialised":
///   - `config.toml` with a valid `listen_addr`
///   - `peers.toml` (empty — `pin_peer_fingerprint` can write into it)
///   - `tls.cert.pem` — a real self-signed cert so `openssl x509` succeeds
///     inside `pin_all_fingerprints` (the script uses `set -euo pipefail`;
///     a failing openssl pipe would exit the script)
///
/// No `daemon.pid` is written intentionally.  `cmd_stop` would try to
/// SIGTERM the PID in that file, which is unsafe.  Instead `start_daemon`
/// will spawn the stub `famp listen` which binds the port via Python so
/// `lsof` succeeds within the 1 s deadline.
fn seed_agent(local_root: &Path, name: &str, port: u16) {
    let dir = local_root.join("agents").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        format!("listen_addr = \"127.0.0.1:{port}\"\n"),
    )
    .unwrap();
    std::fs::write(dir.join("peers.toml"), "").unwrap();
    // Generate a minimal self-signed cert so `openssl x509` succeeds.
    // The script uses `set -euo pipefail` so a non-zero openssl exit code
    // would terminate it early during pin_all_fingerprints.
    let cert_pem = generate_self_signed_cert_pem(name);
    std::fs::write(dir.join("tls.cert.pem"), cert_pem).unwrap();
}

/// Generate a minimal self-signed Ed25519 cert PEM using the `openssl` CLI.
/// Both macOS and Linux CI runners ship openssl; this is not optional.
fn generate_self_signed_cert_pem(cn: &str) -> String {
    let tmp = tempfile::tempdir().unwrap();
    let cert_path = tmp.path().join("cert.pem");
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ed25519",
            "-keyout",
            "/dev/null",
            "-out",
            cert_path.to_str().unwrap(),
            "-days",
            "3650",
            "-nodes",
            "-subj",
            &format!("/CN={cn}"),
        ])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("openssl must be on PATH; macOS and Linux CI both ship it");
    assert!(status.success(), "openssl self-signed cert generation failed");
    std::fs::read_to_string(&cert_path).unwrap()
}

/// Build the env block every test wants: stub PATH, `FAMP_LOCAL_ROOT`,
/// `HOME` redirected to the tempdir so `update_zprofile_init` touches a
/// sandboxed file, never the user's real `~/.zprofile`.
fn cmd_with_env(stub_dir: &Path, local_root: &Path) -> Command {
    let mut cmd = Command::new("bash");
    cmd.arg(script_path())
        .env("FAMP_LOCAL_ROOT", local_root)
        .env("HOME", stub_dir) // sandbox ~/.zprofile updates
        .env("FAMP_BIN", "famp")
        .env(
            "PATH",
            format!(
                "{}:{}",
                stub_dir.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    cmd
}

#[test]
fn wire_rewrites_legacy_mcp_json_in_place() {
    let tmp = tempfile::tempdir().unwrap();
    let stub = tmp.path();
    make_stub_famp(stub);
    let local_root = stub.join("famp-local");
    std::fs::create_dir_all(&local_root).unwrap();

    seed_agent(&local_root, "alice", 58443);
    seed_agent(&local_root, "bob", 58444);

    // Repo dir with a legacy .mcp.json containing FAMP_HOME.
    let repo = stub.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let legacy = std::fs::read_to_string(
        workspace_root()
            .join("crates/famp/tests/fixtures/famp_local_wire/legacy.mcp.json"),
    )
    .unwrap();
    std::fs::write(repo.join(".mcp.json"), legacy).unwrap();

    let output = cmd_with_env(stub, &local_root)
        .args(["wire", repo.to_str().unwrap(), "--as", "alice"])
        .output()
        .expect("run famp-local wire");

    assert!(
        output.status.success(),
        "wire failed (exit {:?})\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let after = std::fs::read_to_string(repo.join(".mcp.json")).unwrap();
    assert!(
        after.contains(r#""args": ["mcp"]"#),
        "new template missing args field:\n{after}"
    );
    assert!(
        !after.contains("FAMP_HOME"),
        "FAMP_HOME still present after rewrite:\n{after}"
    );
    assert!(
        !after.contains("\"env\""),
        "env block still present after rewrite:\n{after}"
    );
}

#[test]
fn wire_idempotent_on_already_migrated_file() {
    let tmp = tempfile::tempdir().unwrap();
    let stub = tmp.path();
    let stub_bin = make_stub_famp(stub);
    let local_root = stub.join("famp-local");
    std::fs::create_dir_all(&local_root).unwrap();

    seed_agent(&local_root, "alice", 58445);
    seed_agent(&local_root, "bob", 58446);

    let repo = stub.join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    // Build the already-migrated content using the stub's exact path
    // (cmd_wire runs `command -v famp` which resolves to our stub, so the
    // emitted "command" value matches stub_bin exactly).
    let migrated_template = format!(
        "{{\n  \"mcpServers\": {{\n    \"famp\": {{\n      \"command\": \"{}\",\n      \"args\": [\"mcp\"]\n    }}\n  }}\n}}\n",
        stub_bin.display()
    );
    std::fs::write(repo.join(".mcp.json"), &migrated_template).unwrap();

    let mtime_before = std::fs::metadata(repo.join(".mcp.json"))
        .unwrap()
        .modified()
        .unwrap();
    // Sleep enough for mtime resolution (macOS HFS+ is 1 s; APFS and Linux are
    // sub-second but 1.1 s keeps us safe everywhere).
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let output = cmd_with_env(stub, &local_root)
        .args(["wire", repo.to_str().unwrap(), "--as", "alice"])
        .output()
        .expect("run famp-local wire");

    assert!(
        output.status.success(),
        "wire (idempotent run) failed (exit {:?})\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mtime_after = std::fs::metadata(repo.join(".mcp.json"))
        .unwrap()
        .modified()
        .unwrap();
    let after_content = std::fs::read_to_string(repo.join(".mcp.json")).unwrap();

    assert_eq!(
        mtime_before, mtime_after,
        "idempotent re-wire must not touch the file (mtime changed: before={mtime_before:?} after={mtime_after:?})"
    );
    assert_eq!(
        after_content, migrated_template,
        "content drift on idempotent re-wire"
    );
}

#[test]
fn mcp_add_does_not_emit_famp_home() {
    let script = std::fs::read_to_string(script_path()).unwrap();
    // After 01-04, no `mcp add` invocation should pass FAMP_HOME via --env.
    // Static grep — duplicates the Task 1 acceptance gate inside CI.
    for (idx, line) in script.lines().enumerate() {
        let l = line.trim_start();
        if (l.starts_with("claude mcp add") || l.starts_with("codex mcp add"))
            && line.contains("FAMP_HOME")
        {
            panic!(
                "scripts/famp-local line {}: mcp-add invocation still emits FAMP_HOME:\n  {}",
                idx + 1,
                line
            );
        }
    }
}
