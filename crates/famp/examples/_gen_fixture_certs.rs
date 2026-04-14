//! One-off fixture cert regenerator for Phase 4 cross-machine tests.
//! Run with: `cargo run --example _gen_fixture_certs -p famp`
//! Outputs: `crates/famp/tests/fixtures/cross_machine/<role>.<crt|key>`
//!
//! Not a public example — the `_` prefix indicates internal tooling.

#![allow(clippy::unwrap_used, clippy::expect_used)]

// Silence workspace `unused_crate_dependencies` for deps the other example /
// tests reference but this one-shot binary does not.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp as _;
use time as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_transport as _;
use tower as _;
use tower_http as _;
use famp_transport_http as _;
use rand as _;
use reqwest as _;
use serde as _;
use serde_json as _;
use tempfile as _;
use thiserror as _;
use tokio as _;
use toml as _;
use url as _;

use rcgen::generate_simple_self_signed;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cross_machine");
    std::fs::create_dir_all(&dir)?;
    for name in ["alice", "bob"] {
        let ck = generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])?;
        std::fs::write(dir.join(format!("{name}.crt")), ck.cert.pem())?;
        std::fs::write(
            dir.join(format!("{name}.key")),
            ck.signing_key.serialize_pem(),
        )?;
        println!("wrote {name}.crt + {name}.key");
    }
    Ok(())
}
