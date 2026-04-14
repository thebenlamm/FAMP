//! Phase 3 Plan 03-03 Task 2 — `famp await` returns typed timeout error.
//!
//! Inits a fresh `FAMP_HOME`, runs `await --timeout 200ms` with no daemon
//! and no inbox.jsonl, asserts the call returns `CliError::AwaitTimeout`
//! within ~1s, and that the cursor file was not created.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::time::{Duration, Instant};

use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp::cli::error::CliError;

use common::init_home_in_process;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn await_times_out_on_empty_inbox() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    let args = AwaitArgs {
        timeout: "200ms".to_string(),
        task: None,
    };

    let start = Instant::now();
    let mut out = Vec::<u8>::new();
    let err = await_run_at(&home, args, &mut out).await.unwrap_err();
    let elapsed = start.elapsed();

    match err {
        CliError::AwaitTimeout { timeout } => assert_eq!(timeout, "200ms"),
        other => panic!("expected AwaitTimeout, got {other:?}"),
    }

    // Allow generous upper bound: 200ms timeout + 250ms poll cadence + slack.
    assert!(
        elapsed < Duration::from_millis(1500),
        "timeout took too long: {elapsed:?}"
    );
    // No stdout output on timeout.
    assert!(out.is_empty(), "expected no output on timeout");

    // Cursor file should not have been created — await never advances
    // on timeout.
    assert!(
        !home.join("inbox.cursor").exists(),
        "cursor file should not exist after timeout"
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
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
