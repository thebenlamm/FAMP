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
        assert!(
            !stdout.lines().any(|l| l.trim_start().starts_with(verb)),
            "famp --help must not advertise deleted verb `{verb}`; got:\n{stdout}"
        );
    }
}
