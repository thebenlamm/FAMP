#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 04 FED-01 invariant: the 6 deleted federation verbs (init, setup,
//! listen, peer add, peer import, old TLS-form send) MUST NOT appear in
//! `famp --help` output. Migration doc carries the load (D-05 hard delete).
//!
//! `famp send` is preserved (bus-routed); `famp peer` is fully removed
//! (peer add and peer import were both subcommands of the deleted `peer`
//! verb).

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
    for verb in ["init", "setup", "listen", "peer"] {
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
        stdout.lines().any(|l| l.trim_start().starts_with("listen-wake")),
        "famp --help must advertise listen-wake; got:\n{stdout}"
    );
}
