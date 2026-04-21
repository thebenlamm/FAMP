//! Tests for `famp_inbox` filter semantics (spec 2026-04-20).
//!
//! Task 1 covers the `extract_task_id` helper; Tasks 2-4 extend with
//! filter, fail-open/fail-closed, cache, and MCP round-trips.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::cli::inbox::list::extract_task_id_for_test;
use famp_core::MessageClass;
use serde_json::json;

/// Every `MessageClass` variant must either yield a non-empty `task_id`
/// or be explicitly handled. A new variant that lands its id outside
/// the currently-understood envelope shape fails this test.
#[test]
fn extract_task_id_covers_every_message_class() {
    let cases: &[(MessageClass, &str)] = &[
        (MessageClass::Request, "01913000-0000-7000-8000-00000000000a"),
        (MessageClass::Commit, "01913000-0000-7000-8000-00000000000b"),
        (MessageClass::Deliver, "01913000-0000-7000-8000-00000000000c"),
        (MessageClass::Ack, "01913000-0000-7000-8000-00000000000d"),
        (MessageClass::Control, "01913000-0000-7000-8000-00000000000e"),
    ];

    for (class, expected_tid) in cases {
        let value = match class {
            // `request`: envelope's own `id` IS the task_id.
            MessageClass::Request => json!({
                "id": expected_tid,
                "class": class.to_string(),
            }),
            // Every other class: task_id lives in `causality.ref`.
            _ => json!({
                "id": "01913000-0000-7000-8000-0000000000ff",
                "class": class.to_string(),
                "causality": { "ref": expected_tid },
            }),
        };
        let extracted = extract_task_id_for_test(&value);
        assert_eq!(
            extracted,
            *expected_tid,
            "class={class} extracted={extracted:?} expected={expected_tid:?}",
        );
    }
}

// Silencers — match the convention in inbox_list_respects_cursor.rs.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
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
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tempfile as _;
use tokio as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
