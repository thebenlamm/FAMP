#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! CLI surface of the personal-network workflow.
//!
//! FED-01 (`cli_help_invariant.rs`, unchanged) keeps the top-level `setup` verb deleted.
//! The personal-network command is therefore `famp network setup`, never `famp setup`.
//! `connect` and `doctor` collide with no recorded invariant and stay top-level.

use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn famp(args: &[&str]) -> std::process::Output {
    Command::cargo_bin("famp")
        .unwrap()
        .args(args)
        .output()
        .unwrap()
}

fn lists_verb(help: &str, verb: &str) -> bool {
    help.lines().any(|l| {
        let t = l.trim_start();
        t == verb || t.starts_with(&format!("{verb} ")) || t.starts_with(&format!("{verb}\t"))
    })
}

#[test]
fn setup_lives_under_network_and_is_not_a_top_level_verb() {
    let top = famp(&["--help"]);
    assert!(top.status.success());
    let top = String::from_utf8_lossy(&top.stdout);
    assert!(
        !lists_verb(&top, "setup"),
        "`setup` must not be a top-level verb (FED-01); got:\n{top}"
    );
    assert!(lists_verb(&top, "connect") && lists_verb(&top, "doctor"));

    // The old spelling is not accepted at all.
    assert!(
        !famp(&["setup", "--help"]).status.success(),
        "`famp setup` must stay an unknown subcommand"
    );

    let net = famp(&["network", "--help"]);
    assert!(net.status.success());
    assert!(lists_verb(&String::from_utf8_lossy(&net.stdout), "setup"));

    let help = famp(&["network", "setup", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("personal FAMP network"), "{help}");
    for deleted_flag in ["--listen", "--tls-cert", "--tls-key", "--peers-toml"] {
        assert!(
            !help.contains(deleted_flag),
            "must not bring back the deleted TLS-form flag `{deleted_flag}`; got:\n{help}"
        );
    }
}
