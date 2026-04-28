//! Sentinel-discriminator test for the send-side terminal-advance error
//! surfacing at `crates/famp/src/cli/send/mod.rs:~514`.
//!
//! ## History / Pattern lineage
//!
//! This test mirrors `await_commit_advance_error_surfaces.rs`
//! (quick-260425-kbx), which in turn built on the lost-update race fix
//! (quick-260425-ho8) that introduced `TaskDir::try_update`.
//!
//! **Bug B2 (quick-260425-gst / ho8 / kbx):** the `await_cmd` commit-receipt
//! branch used `let _ = advance_committed(...)` + unconditional
//! `tasks.update(...)`, swallowing errors AND producing spurious writes on
//! FSM `Err`. quick-260425-ho8 replaced that with a single
//! `tasks.try_update(task_id, |mut r| advance_committed(&mut r).map(|_| r))`
//! call — the FSM advance lives INSIDE the closure; on closure `Err`,
//! `try_update` performs NO disk write.
//!
//! **This plan (quick-260425-lny):** ports the identical B2 anti-pattern on
//! the send side — `send/mod.rs`'s `SendMode::DeliverTerminal` arm of
//! `persist_post_send` uses `let _ = advance_terminal(...)` inside
//! `tasks.update(...)`. Same two symptoms:
//!   1. Errors are swallowed.
//!   2. Spurious file rewrite occurs on `advance_terminal` `Err`.
//!
//! ## Why a TOML-comment sentinel discriminates the bug
//!
//! The test injects a trailing TOML comment (`# TEST_SENTINEL_DO_NOT_REWRITE`)
//! into the task file out-of-band. TOML comments are valid input —
//! `toml::from_str` parses `TaskRecord` normally — but `toml::to_string`
//! does NOT preserve them on round-trip. Therefore:
//!
//! - **No write occurred:** sentinel comment SURVIVES → test PASSES.
//! - **Any write occurred** (benign re-serialize OR buggy spurious write):
//!   sentinel comment is CLOBBERED → test FAILS.
//!
//! This is a strict discrimination test: it FAILS against pre-lny send code
//! even when bytes would otherwise be identical (as the pre-kbx mtime test
//! would have missed).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::io::Write as _;
use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};
use famp_taskdir::TaskDir;

use common::{init_home_in_process, wait_for_tls_listener_ready};

const SENTINEL: &str = "\n# TEST_SENTINEL_DO_NOT_REWRITE\n";

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn terminal_send_when_record_in_requested_does_not_rewrite_task_file() {
    famp::cli::send::client::allow_tofu_bootstrap_for_tests();

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // 1. Spin up an in-process listener (mirror send_terminal_blocks_resend.rs).
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    };
    let home_for_task = home.clone();
    let server_task = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_for_task, listener, shutdown_signal)
            .await
            .expect("run_on_listener");
    });

    wait_for_tls_listener_ready().await;

    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    // 2. Send --new-task to create a record on disk in REQUESTED state.
    //    Do NOT consume the auto-commit reply via await_cmd — leave the record
    //    in REQUESTED so that advance_terminal will return Err(IllegalTransition)
    //    when called from the DeliverTerminal persist path.
    send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: Some("sentinel test".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
        },
    )
    .await
    .expect("new task send");

    // 3. Pull task_id from tasks.list().
    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    let task_id = tasks.list().unwrap()[0].task_id.clone();

    // Verify that the record is STILL in REQUESTED state (we deliberately
    // skipped the `await_cmd::run_at` step that would advance it to COMMITTED).
    let pre_record = tasks.read(&task_id).unwrap();
    assert_eq!(
        pre_record.state, "REQUESTED",
        "record must be REQUESTED before the terminal send (pre-condition for the bug path)"
    );

    // 4. Inject a TOML-comment SENTINEL into the task file out-of-band.
    //    The file must already exist (no .create(true)).
    let task_file = home.join("tasks").join(format!("{task_id}.toml"));
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&task_file)
            .expect("open task file for sentinel append");
        f.write_all(SENTINEL.as_bytes())
            .expect("append sentinel to task file");
    }

    // Sanity: sentinel is present and record still parses correctly.
    let pre_bytes = std::fs::read_to_string(&task_file).expect("read pre-send");
    assert!(
        pre_bytes.contains("TEST_SENTINEL_DO_NOT_REWRITE"),
        "sentinel must be present BEFORE terminal send (test setup integrity check)"
    );
    let _parse_check: famp_taskdir::TaskRecord =
        toml::from_str(&pre_bytes).expect("sentinel must not break TOML parsing");

    // 5. Call send_run_at with terminal=true on the task that is still in REQUESTED.
    //    The send path:
    //      - Pre-check at send/mod.rs:130-144: reads existing record, sees
    //        terminal=false, proceeds.
    //      - Builds + POSTs the deliver envelope (200 OK from the listener).
    //      - Hits persist_post_send → SendMode::DeliverTerminal arm.
    //      - Calls advance_terminal(&mut r) on a record in REQUESTED state.
    //      - advance_terminal returns Err(CliError::Envelope(IllegalTransition))
    //        because the FSM rejects REQUESTED → COMPLETED via Deliver.
    //      - Pre-fix: `let _ =` swallows; tasks.update rewrites file (sentinel
    //        clobbered).
    //      - Post-fix: try_update skips write on closure Err (sentinel survives).
    //
    //    The wire-side succeeded (200 OK), so send_run_at may return Ok.
    //    We do NOT assert on Ok/Err here — the bug is about the *side effect*
    //    (spurious write), not the return value.
    let send_result = send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: None,
            task: Some(task_id.clone()),
            terminal: true,
            body: Some("done".to_string()),
            more_coming: false,
        },
    )
    .await;
    // Print result for diagnostic purposes but don't panic.
    if let Err(ref e) = send_result {
        eprintln!("send_run_at returned Err (non-fatal for this test): {e:?}");
    }

    // 6. Primary assertion: sentinel survival.
    //    If a write occurred (whether the buggy spurious write from tasks.update
    //    or any other cause), the TOML comment will be gone because
    //    toml::to_string does not preserve comments.
    let post_bytes = std::fs::read_to_string(&task_file).expect("read post-send");
    assert!(
        post_bytes.contains("TEST_SENTINEL_DO_NOT_REWRITE"),
        "sentinel was clobbered: a spurious write occurred during SendMode::DeliverTerminal \
         persist when advance_terminal returned Err (record was in REQUESTED state). \
         Bytes pre/post:\n\
         ---PRE---\n{pre_bytes}\n---POST---\n{post_bytes}\n--- \
         (quick-260425-lny — RED guard for try_update closure-Err contract on send-side)"
    );

    // 6b. State must be unchanged: still REQUESTED (no spurious FSM mutation).
    let post_record = tasks.read(&task_id).unwrap();
    assert_eq!(
        post_record.state, "REQUESTED",
        "state must be unchanged on FSM error (no spurious write, no state corruption)"
    );

    // 7. Tear down.
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}

// Silencers — required to keep `unused_crate_dependencies` quiet.
// Copied from send_terminal_blocks_resend.rs; extended with additional deps
// that the compiler requests.
use axum as _;
use base64 as _;
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
use humantime as _;
use rand as _;
use rcgen as _;
use reqwest as _;
use rustls as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use tempfile as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
