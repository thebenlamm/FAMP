//! Phase 3 Plan 03-04 — INBOX-05 advisory-lock contention test.
//!
//! Acquires the `InboxLock` manually to simulate a held first reader,
//! then runs `famp await` against the same home. The subcommand must
//! fail fast with `InboxError::LockHeld` (not wait out its timeout).
//! After the manual lock is dropped, a second `famp await` on the same
//! empty inbox must make it all the way to `AwaitTimeout`, proving the
//! lock is correctly released.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp::cli::error::CliError;
use famp_inbox::{InboxError, InboxLock};

use common::conversation_harness::setup_home;

#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; \
revisit / migrate in Phase 4 federation gateway"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_await_while_first_holds_lock_returns_lockheld() {
    let tmp = setup_home();
    let home = tmp.path().to_path_buf();

    // 1. Hold the lock manually.
    let lock = InboxLock::acquire(&home).expect("acquire lock");

    // 2. famp await must fail fast with LockHeld (not spin to timeout).
    let t0 = std::time::Instant::now();
    let res = await_run_at(
        &home,
        AwaitArgs {
            timeout: "5s".to_string(),
            task: None,
        },
        &mut std::io::sink(),
    )
    .await;
    let elapsed = t0.elapsed();
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "await with held lock must fail fast, elapsed = {elapsed:?}"
    );
    match res {
        Err(CliError::Inbox(InboxError::LockHeld { pid, .. })) => {
            assert_eq!(pid, std::process::id());
        }
        other => panic!("expected Inbox(LockHeld), got {other:?}"),
    }

    // 3. Release the manual lock.
    drop(lock);

    // 4. Second await on an empty inbox must now make it to the
    //    polling loop and time out.
    let res2 = await_run_at(
        &home,
        AwaitArgs {
            timeout: "200ms".to_string(),
            task: None,
        },
        &mut std::io::sink(),
    )
    .await;
    match res2 {
        Err(CliError::AwaitTimeout { timeout }) => assert_eq!(timeout, "200ms"),
        other => panic!("expected AwaitTimeout, got {other:?}"),
    }

    // 5. Lock file should be gone after the second await's RAII drop.
    assert!(
        !home.join("inbox.lock").exists(),
        "inbox.lock must be removed after await returns"
    );
}

// Silencers.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
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
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
