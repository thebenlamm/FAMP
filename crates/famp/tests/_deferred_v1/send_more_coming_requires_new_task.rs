//! Quick task 260425-pc7 — BL-01 regression.
//!
//! `--more-coming` paired with `--task` (instead of `--new-task`) used
//! to slip past clap's `requires = "new_task"` constraint and reach the
//! deliver-envelope path, where `args.more_coming` was never read. The
//! user's "more briefing follows" intent was silently dropped on the wire.
//!
//! `run_at_structured` now hard-rejects the combination with
//! `CliError::SendArgsInvalid` before any envelope work happens.
//!
//! ## Phase 02 Plan 02-04 transition (2026-04-28)
//!
//! Plan 02-04 swapped `cli::send` from the v0.8 HTTPS path to the v0.9
//! UDS bus path. The `SendArgs` shape changed (`to` became `Option<String>`,
//! new `channel` / `act_as` fields) AND `run_at_structured` no longer
//! accepts `home: &Path` (now `sock: &Path`). The BL-01 regression itself
//! still applies — the run-time guard moved over verbatim — but this
//! integration test was scaffolded around the v0.8 HTTPS path
//! (TOFU bootstrap, peer add, `init_home_in_process`) and cannot be
//! mechanically translated to the bus path inside Phase 02.
//!
//! The semantic check the test enforced is now covered at the unit level
//! by `crates/famp/src/cli/send/mod.rs::tests::
//! more_coming_without_new_task_errors_in_run_at_structured`. This file
//! is gated off until Phase 4 either deletes the v0.8 HTTPS path entirely
//! or migrates this file to the bus surface.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use famp::cli::error::CliError;
use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at_structured, SendArgs};

use common::init_home_in_process;

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test(flavor = "current_thread")]
            superseded by the unit test `more_coming_without_new_task_errors_in_run_at_structured` \
            in cli/send/mod.rs::tests. Phase 04 will delete this file with the v0.8 \
            send-via-listen surface."]
async fn more_coming_with_task_is_rejected_before_send() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // The guard fires after peer lookup and after the SendMode match,
    // so we need a registered peer for the call to reach it. The peer's
    // endpoint never gets dialed — the guard short-circuits first.
    run_add_at(
        &home,
        "self".to_string(),
        "https://127.0.0.1:1".to_string(),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    let args = SendArgs {
        to: Some("self".to_string()),
        channel: None,
        new_task: None,
        task: Some("019f0000-0000-7000-8000-000000000001".to_string()),
        terminal: false,
        body: None,
        more_coming: true,
        act_as: None,
    };
    let err = run_at_structured(&home, args)
        .await
        .expect_err("--more-coming + --task must be rejected");

    match err {
        CliError::SendArgsInvalid { reason } => {
            assert!(
                reason.contains("--more-coming is only valid with --new-task"),
                "expected the BL-01 guard reason, got: {reason}"
            );
        }
        other => panic!("expected CliError::SendArgsInvalid, got {other:?}"),
    }
}
