#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 04 FED-01 invariant: the deleted federation verbs (init, setup,
//! listen, old TLS-form send) MUST NOT appear in `famp --help` output.
//! Migration doc carries the load (D-05 hard delete).
//!
//! `famp send` is preserved (bus-routed). `famp peer` was fully removed in
//! Phase 4 (its `add`/`import` TOML-`peers.toml` subcommands) and
//! **deliberately reintroduced in Phase 8** (CONTEXT.md D-05, locked) as
//! `famp peer export`/`import` — a different shape (Ed25519 TOFU trust
//! bootstrap against `famp-keyring`, not TOML `peers.toml`). This test
//! asserts the OLD `peer add`/`peer import` subcommand shape stays gone
//! while the NEW `peer export`/`import` shape is present.

use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

#[test]
fn famp_help_omits_deleted_federation_verbs() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "famp --help must exit 0; got {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for verb in ["init", "setup", "listen"] {
        // Match the bare deleted verb as a clap subcommand token, not longer
        // names that share a prefix (e.g. `listen-wake` must remain legal).
        assert!(
            !stdout.lines().any(|l| {
                let trimmed = l.trim_start();
                trimmed == verb
                    || trimmed.starts_with(&format!("{verb} "))
                    || trimmed.starts_with(&format!("{verb}\t"))
            }),
            "famp --help must not advertise deleted verb `{verb}`; got:\n{stdout}"
        );
    }
    // Positive: host-neutral wake command is advertised.
    assert!(
        stdout
            .lines()
            .any(|l| l.trim_start().starts_with("listen-wake")),
        "famp --help must advertise listen-wake; got:\n{stdout}"
    );

    // Phase 8 (TRUST-01): `famp peer` is BACK, but with the export/import
    // trust-bootstrap shape, not the deleted TOML add/import shape.
    assert!(
        stdout.lines().any(|l| l.trim_start().starts_with("peer")),
        "famp --help must advertise the Phase 8 `peer` verb; got:\n{stdout}"
    );
    let peer_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "--help"])
        .output()
        .unwrap();
    assert!(
        peer_help.status.success(),
        "famp peer --help must exit 0; got {:?}",
        peer_help.status
    );
    let peer_stdout = String::from_utf8_lossy(&peer_help.stdout);
    for verb in ["export", "import"] {
        assert!(
            peer_stdout
                .lines()
                .any(|l| l.trim_start().starts_with(verb)),
            "famp peer --help must advertise `{verb}`; got:\n{peer_stdout}"
        );
    }
    // The OLD deleted `peer add` subcommand must NOT have come back.
    assert!(
        !peer_stdout
            .lines()
            .any(|l| l.trim_start().starts_with("add")),
        "famp peer --help must NOT advertise the deleted `add` subcommand; got:\n{peer_stdout}"
    );
}
