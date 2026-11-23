//! Personal-network onboarding, end to end through the REAL binaries.
//!
//! `famp network setup` -> `famp network export/import` -> `famp network serve`
//! (which execs `famp-gateway` from the generated config) on two isolated
//! homes, then a real signed cross-host delivery via `famp send`, with
//! `famp doctor` as the readiness probe. Also pins that the gateway identity
//! file is byte-identical across setup rerun, import and delivery.
//!
//! Isolated homes only: `--no-services --host none`, loopback addresses via
//! `--allow-non-tailnet`. Nothing here touches launchd, systemd, ssh or the
//! user's real `~/.famp`.

#![allow(unused_crate_dependencies)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::path::Path;

#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    ensure_famp_bin_built, famp_cmd, famp_spawn, pick_free_port, poll_inbox_contains, spawn_broker,
    spawn_register, wait_for_broker_socket, wait_until_live, Side, POLL_DEADLINE, STARTUP_DEADLINE,
};

fn ok(sock: &Path, home: &Path, args: &[&str]) -> Vec<u8> {
    let out = famp_cmd(sock, home, args);
    assert!(
        out.status.success(),
        "famp {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn sha256(path: &Path) -> String {
    let out = std::process::Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success(), "shasum failed for {}", path.display());
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

fn setup(side: &Side, name: &str, port: u16) {
    ok(
        &side.sock(),
        side.home(),
        &[
            "network",
            "setup",
            "--no-services",
            "--host",
            "none",
            "--allow-non-tailnet",
            "--address",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--name",
            name,
        ],
    );
}

#[test]
fn setup_import_serve_delivers_a_signed_message_and_keeps_identity_bytes() {
    ensure_famp_bin_built();
    let side_a = Side::new();
    let side_b = Side::new();
    let (port_a, port_b) = (pick_free_port(), pick_free_port());

    let _broker_a = spawn_broker(&side_a);
    let _broker_b = spawn_broker(&side_b);
    wait_for_broker_socket(&side_a.sock(), STARTUP_DEADLINE);
    wait_for_broker_socket(&side_b.sock(), STARTUP_DEADLINE);

    setup(&side_a, "alice", port_a);
    setup(&side_b, "bob", port_b);
    let id_a = side_a.home().join("gateway/identity.ed25519");
    let id_b = side_b.home().join("gateway/identity.ed25519");
    let (hash_a, hash_b) = (sha256(&id_a), sha256(&id_b));

    // CONTROL for the rerun check below: a rerun must succeed byte-identically,
    // while a conflicting rerun must be refused.
    setup(&side_a, "alice", port_a);
    assert_eq!(sha256(&id_a), hash_a, "setup rerun rewrote the identity");
    let conflicting = famp_cmd(
        &side_a.sock(),
        side_a.home(),
        &[
            "network",
            "setup",
            "--no-services",
            "--host",
            "none",
            "--allow-non-tailnet",
            "--address",
            "127.0.0.1",
            "--name",
            "not-alice",
        ],
    );
    assert!(
        !conflicting.status.success(),
        "a conflicting rerun must be refused, not rotate the identity"
    );
    assert_eq!(sha256(&id_a), hash_a);

    // Exchange public cards (what `connect` does over ssh) and pin them.
    let card_a = ok(&side_a.sock(), side_a.home(), &["network", "export"]);
    let card_b = ok(&side_b.sock(), side_b.home(), &["network", "export"]);
    for card in [&card_a, &card_b] {
        let text = String::from_utf8_lossy(card);
        assert!(!text.contains("PRIVATE KEY"), "a card leaked a private key");
    }
    let bundle = format!(
        "[{},{}]",
        String::from_utf8_lossy(&card_a).trim(),
        String::from_utf8_lossy(&card_b).trim()
    );
    for side in [&side_a, &side_b] {
        let file = side.home().join("bundle.json");
        std::fs::write(&file, &bundle).unwrap();
        let file = file.to_str().unwrap();
        ok(
            &side.sock(),
            side.home(),
            &["network", "import", file, "--check"],
        );
        ok(&side.sock(), side.home(), &["network", "import", file]);
        // Importing the same bundle again is idempotent.
        ok(&side.sock(), side.home(), &["network", "import", file]);
    }
    assert_eq!(sha256(&id_a), hash_a, "import rewrote A's identity");
    assert_eq!(sha256(&id_b), hash_b, "import rewrote B's identity");

    // Serve from the GENERATED config: this is the service entry point.
    let _gateway_a = famp_spawn(&side_a.sock(), side_a.home(), &["network", "serve"]);
    let _gateway_b = famp_spawn(&side_b.sock(), side_b.home(), &["network", "serve"]);
    let _alice = spawn_register(&side_a, "alice");
    let _bob = spawn_register(&side_b, "bob");
    wait_until_live(&side_a.sock(), side_a.home(), "bob", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "alice", STARTUP_DEADLINE);

    // `famp doctor` is the readiness probe and must go green on both sides.
    for side in [&side_a, &side_b] {
        let start = std::time::Instant::now();
        loop {
            let out = famp_cmd(&side.sock(), side.home(), &["doctor", "--json"]);
            if out.status.success() {
                let report = String::from_utf8_lossy(&out.stdout);
                assert!(report.contains("\"permissions\""), "{report}");
                assert!(report.contains("key_id="), "doctor must print fingerprints");
                break;
            }
            assert!(
                start.elapsed() < STARTUP_DEADLINE,
                "doctor never went green: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }

    // Real signed cross-host delivery through the generated routes.
    let sent = ok(
        &side_a.sock(),
        side_a.home(),
        &[
            "send",
            "--to",
            "agent:bob.famp/bob",
            "--as",
            "alice",
            "--domain",
            "alice.famp",
            "--new-task",
            "onboarding ping",
        ],
    );
    let task: serde_json::Value = serde_json::from_slice(&sent).unwrap();
    let task_id = task["task_id"].as_str().unwrap();
    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        task_id,
        "request",
        POLL_DEADLINE,
    );
    assert_eq!(sha256(&id_a), hash_a, "delivery rewrote A's identity");
    assert_eq!(sha256(&id_b), hash_b, "delivery rewrote B's identity");

    // W0 through the real gateways. `setup` seeded B's own name, which is why the
    // first delivery worked; revoking it stops the NEXT message with no restart.
    let send = |text: &str| {
        let out = ok(
            &side_a.sock(),
            side_a.home(),
            &[
                "send",
                "--to",
                "agent:bob.famp/bob",
                "--as",
                "alice",
                "--domain",
                "alice.famp",
                "--new-task",
                text,
            ],
        );
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        v["task_id"].as_str().unwrap().to_owned()
    };
    ok(&side_b.sock(), side_b.home(), &["network", "deny", "bob"]);
    let denied_task = send("after deny");
    let window = std::time::Instant::now();
    while window.elapsed() < std::time::Duration::from_secs(6) {
        let inbox = ok(
            &side_b.sock(),
            side_b.home(),
            &["inbox", "list", "--as", "bob"],
        );
        assert!(
            !String::from_utf8_lossy(&inbox).contains(&denied_task),
            "a message for a name removed from the allowlist was delivered"
        );
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    ok(&side_b.sock(), side_b.home(), &["network", "allow", "bob"]);
    let allowed_task = send("after allow");
    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &allowed_task,
        "request",
        POLL_DEADLINE,
    );
}
