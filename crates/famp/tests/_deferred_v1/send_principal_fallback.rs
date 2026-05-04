//! Regression test for the silent-principal-fallback finding.
//!
//! `famp send` must NOT silently sign as `agent:localhost/self` when the
//! configured principal is malformed or `config.toml` cannot be parsed.
//! The fallback applies only when the field is genuinely absent.
//!
//! Phase 02 Plan 02-04: gated off — v0.8 HTTPS shape incompatible with
//! v0.9 bus path. See `send_more_coming_requires_new_task.rs` header.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};

use common::init_home_in_process;

/// A peer entry is required so `send` reaches `load_self_principal` rather
/// than failing earlier on `PeerNotFound`. The endpoint never gets dialled
/// because the principal load fails first.
fn add_dummy_peer(home: &std::path::Path) {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    let pubkey = URL_SAFE_NO_PAD.encode(bytes);
    run_add_at(
        home,
        "self".to_string(),
        "https://127.0.0.1:1".to_string(),
        pubkey,
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");
}

fn send_args() -> SendArgs {
    SendArgs {
        to: Some("self".to_string()),
        channel: None,
        new_task: Some("hi".to_string()),
        task: None,
        terminal: false,
        body: None,
        more_coming: false,
        act_as: None,
    }
}
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "current_thread")]
async fn malformed_config_toml_is_a_hard_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);
    add_dummy_peer(&home);

    // Corrupt the config so it's no longer valid TOML.
    std::fs::write(home.join("config.toml"), b"this = is = not = valid =\n").unwrap();

    let res = send_run_at(&home, send_args()).await;
    let err = res.expect_err("malformed config.toml must fail send, not silently fall back");
    assert_eq!(
        err.mcp_error_kind(),
        "toml_parse",
        "expected typed TomlParse error, got {err}"
    );
}
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "current_thread")]
async fn malformed_principal_field_is_a_hard_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);
    add_dummy_peer(&home);

    // Valid TOML, but the principal value is not a valid Principal.
    std::fs::write(
        home.join("config.toml"),
        b"listen_addr = \"127.0.0.1:8443\"\nprincipal = \"not-a-valid-principal\"\n",
    )
    .unwrap();

    let res = send_run_at(&home, send_args()).await;
    let err = res.expect_err("malformed principal must be hard-error, not silent fallback");
    assert_eq!(
        err.mcp_error_kind(),
        "principal_invalid",
        "expected typed PrincipalInvalid, got {err}"
    );
}
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "current_thread")]
async fn absent_principal_field_uses_fallback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);
    add_dummy_peer(&home);

    // Default config: no principal field. Send must succeed past the
    // principal load and then fail on the network connect (port 1 has no
    // listener) or on the refused TOFU bootstrap (no pin, no opt-in).
    // Either is fine — what matters is that the failure is NOT a config
    // or principal error, which would mean the fallback path is broken.
    let res = send_run_at(&home, send_args()).await;
    let err = res.expect_err("connect/bootstrap should fail with port 1");
    let kind = err.mcp_error_kind();
    assert!(
        matches!(kind, "send_failed" | "tofu_bootstrap_refused"),
        "fallback path produced unexpected error kind {kind}: {err}"
    );
}

// Silencers: this binary only uses a small slice of the workspace deps via
// `common`, so quiet `unused_crate_dependencies` for the rest.
use axum as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_taskdir as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use rand as _;
use rcgen as _;
use rustls as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
