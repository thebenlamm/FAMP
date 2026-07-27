//! DOC-04 / D-07: accuracy gate — `docs/GATEWAY-SETUP.md`'s documented
//! `famp peer export`/`famp peer import` commands must match the shipping
//! clap CLI surface.
//!
//! Unlike `famp-gateway`, `famp peer` IS clap-based, so `--help` works
//! normally (mirrors `crates/famp/tests/cli_help_invariant.rs`'s pattern).
//! This asserts `famp peer export --help` / `famp peer import --help`
//! succeed and advertise the flags/subcommands the guide documents, and
//! that the guide's invocations are present in the clap help text.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn gateway_setup_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/GATEWAY-SETUP.md")
}

#[test]
fn gateway_setup_doc_accuracy() {
    // `famp peer export --help`
    let export_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "export", "--help"])
        .output()
        .expect("famp peer export --help must be runnable");
    assert!(
        export_help.status.success(),
        "famp peer export --help must exit 0; got {:?}",
        export_help.status
    );
    let export_stdout = String::from_utf8_lossy(&export_help.stdout);
    assert!(
        export_stdout.contains("--as"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: expected `famp peer export --help` to advertise `--as`; got:\n{export_stdout}"
    );

    // `famp peer import --help`
    let import_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "import", "--help"])
        .output()
        .expect("famp peer import --help must be runnable");
    assert!(
        import_help.status.success(),
        "famp peer import --help must exit 0; got {:?}",
        import_help.status
    );

    // `famp peer --help` — confirms both subcommand names still exist.
    let peer_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "--help"])
        .output()
        .expect("famp peer --help must be runnable");
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
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: `famp peer --help` must advertise `{verb}`; got:\n{peer_stdout}"
        );
    }

    // Guide-content half: the guide must actually document these exact
    // invocations, not just the binary supporting them.
    let doc_path = gateway_setup_doc_path();
    let doc = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| {
        panic!(
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: could not read {}: {e}",
            doc_path.display()
        )
    });
    assert!(
        doc.contains("famp peer export --as"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: guide is missing `famp peer export --as` usage"
    );
    assert!(
        doc.contains("famp peer import"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: guide is missing `famp peer import` usage"
    );
}
