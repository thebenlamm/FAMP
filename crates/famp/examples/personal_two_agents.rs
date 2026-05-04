//! FAMP v0.7 Personal Runtime — same-process happy-path example.
//!
//! Two agents (`agent:local/alice`, `agent:local/bob`) exchange a full
//! `request -> commit -> deliver -> ack` cycle over an in-process
//! `MemoryTransport`, with every message signed and verified against a
//! pre-pinned `Keyring`. Exits 0 on success; prints an ordered typed
//! trace to stdout.
//!
//! Invoke: `cargo run --example personal_two_agents`

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::similar_names)] // example, not lib

// Silence workspace `unused_crate_dependencies` for deps pulled in via famp's
// Cargo.toml that the example does not reference directly.
use assert_cmd as _;
use axum as _;
use base64 as _;
use clap as _;
use dirs as _;
use famp as _;
use famp_bus as _;
use famp_inbox as _;
use famp_taskdir as _;
use famp_transport_http as _;
use hex as _;
use humantime as _;
use insta as _;
use nix as _;
use regex as _;
use reqwest as _;
use serde as _;
use sha2 as _;
use temp_env as _;
use tempfile as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
use which as _;

use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;
use famp_transport::MemoryTransport;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[path = "../tests/common/cycle_driver.rs"]
mod cycle_driver;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Principal::from_str("agent:local/alice")?;
    let bob = Principal::from_str("agent:local/bob")?;

    let mut rng = rand::rngs::OsRng;
    let alice_dalek = ed25519_dalek::SigningKey::generate(&mut rng);
    let bob_dalek = ed25519_dalek::SigningKey::generate(&mut rng);

    let alice_sk = FampSigningKey::from_bytes(alice_dalek.to_bytes());
    let bob_sk = FampSigningKey::from_bytes(bob_dalek.to_bytes());
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    let bob_vk: TrustedVerifyingKey = bob_sk.verifying_key();

    let alice_keyring = Keyring::new().with_peer(bob.clone(), bob_vk.clone())?;
    let bob_keyring = Keyring::new().with_peer(alice.clone(), alice_vk.clone())?;

    let transport = MemoryTransport::new();
    transport.register(alice.clone()).await;
    transport.register(bob.clone()).await;

    let trace: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));

    let bob_task = {
        let transport = transport.clone();
        let bob_keyring = bob_keyring.clone();
        let bob_p = bob.clone();
        let alice_p = alice.clone();
        let trace = trace.clone();
        let bob_sk = FampSigningKey::from_bytes(bob_dalek.to_bytes());
        tokio::spawn(async move {
            cycle_driver::drive_bob(&transport, &bob_keyring, &bob_p, &alice_p, &bob_sk, &trace)
                .await
                .expect("bob cycle succeeds");
        })
    };

    let alice_task = {
        let transport = transport.clone();
        let alice_keyring = alice_keyring.clone();
        let alice_p = alice.clone();
        let bob_p = bob.clone();
        let trace = trace.clone();
        let alice_sk = FampSigningKey::from_bytes(alice_dalek.to_bytes());
        tokio::spawn(async move {
            cycle_driver::drive_alice(
                &transport,
                &alice_keyring,
                &alice_p,
                &bob_p,
                &alice_sk,
                &trace,
            )
            .await
            .expect("alice cycle succeeds");
        })
    };

    alice_task.await?;
    bob_task.await?;

    let final_trace = trace.lock().unwrap();
    assert_eq!(
        final_trace.len(),
        4,
        "expected 4 trace lines, got {}",
        final_trace.len()
    );
    assert!(final_trace[0].contains("Request"));
    assert!(final_trace[1].contains("Commit"));
    assert!(final_trace[2].contains("Deliver"));
    assert!(final_trace[3].contains("Ack"));
    drop(final_trace);

    println!("OK: personal_two_agents complete");
    Ok(())
}
