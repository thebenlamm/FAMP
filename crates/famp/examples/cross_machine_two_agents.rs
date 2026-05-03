//! `cross_machine_two_agents` — EX-02 / CONF-04 example (Phase 4).
//!
//! ONE binary, TWO invocations (D-E1). No auto-orchestration.
//! Symmetric topology: both roles run server + client over real HTTPS.
//!
//! Usage (manual):
//! ```text
//! # Terminal 1 (bob):
//! cargo run --example cross_machine_two_agents -p famp -- \
//!     --role bob --listen 127.0.0.1:8443 \
//!     --out-pubkey /tmp/bob.pub --out-cert /tmp/bob.crt --out-key /tmp/bob.key \
//!     --peer 'agent:local/alice=<alice-pub-b64>' \
//!     --addr 'agent:local/alice=https://127.0.0.1:8444' \
//!     --trust-cert /tmp/alice.crt
//!
//! # Terminal 2 (alice):
//! cargo run --example cross_machine_two_agents -p famp -- \
//!     --role alice --listen 127.0.0.1:8444 \
//!     --out-pubkey /tmp/alice.pub --out-cert /tmp/alice.crt --out-key /tmp/alice.key \
//!     --peer 'agent:local/bob=<bob-pub-b64>' \
//!     --addr 'agent:local/bob=https://127.0.0.1:8443' \
//!     --trust-cert /tmp/bob.crt
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::missing_const_for_fn,
    clippy::single_match_else,
    clippy::future_not_send
)]

// Silencers — deps from the famp crate that this example does not reference.
use assert_cmd as _;
use axum as _;
use clap as _;
use dirs as _;
use famp_bus as _;
use famp_canonical as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_taskdir as _;
use famp_transport as _;
use hex as _;
use humantime as _;
use insta as _;
use nix as _;
use regex as _;
use reqwest as _;
use rustls as _;
use serde as _;
use sha2 as _;
use temp_env as _;
use tempfile as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use uuid as _;
use which as _;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;
use famp_transport_http::{build_router, tls, tls_server, HttpTransport};
use rcgen::generate_simple_self_signed;
use std::{
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};
use url::Url;

#[path = "../tests/common/cycle_driver.rs"]
mod cycle_driver;

#[derive(Debug, Clone, Copy)]
enum Role {
    Alice,
    Bob,
}

impl Role {
    fn principal_str(self) -> &'static str {
        match self {
            Self::Alice => "agent:local/alice",
            Self::Bob => "agent:local/bob",
        }
    }
    fn default_port(self) -> u16 {
        match self {
            Self::Alice => 8444,
            Self::Bob => 8443,
        }
    }
}

#[derive(Debug, Default)]
struct Args {
    role: Option<Role>,
    listen: Option<SocketAddr>,
    peers: Vec<(Principal, [u8; 32])>,
    addrs: Vec<(Principal, Url)>,
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
    trust_cert: Option<PathBuf>,
    out_pubkey: Option<PathBuf>,
    out_cert: Option<PathBuf>,
    out_key: Option<PathBuf>,
}

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: cross_machine_two_agents --role alice|bob \\
    [--listen <addr:port>] \\
    [--peer <principal>=<base64url-pubkey>] \\
    [--addr <principal>=<https-url>] \\
    [--cert <path> --key <path>] \\
    [--trust-cert <path>] \\
    [--out-pubkey <path>] [--out-cert <path>] [--out-key <path>]"
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    macro_rules! val {
        () => {
            it.next().unwrap_or_else(|| usage_and_exit())
        };
    }
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--role" => {
                args.role = Some(match val!().as_str() {
                    "alice" => Role::Alice,
                    "bob" => Role::Bob,
                    _ => usage_and_exit(),
                });
            }
            "--listen" => {
                args.listen = Some(val!().parse().unwrap_or_else(|_| usage_and_exit()));
            }
            "--peer" => {
                let v = val!();
                let (p, b64) = v.split_once('=').unwrap_or_else(|| usage_and_exit());
                let principal = Principal::from_str(p).unwrap_or_else(|_| usage_and_exit());
                let bytes = URL_SAFE_NO_PAD
                    .decode(b64.as_bytes())
                    .unwrap_or_else(|_| usage_and_exit());
                let arr: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .unwrap_or_else(|_| usage_and_exit());
                args.peers.push((principal, arr));
            }
            "--addr" => {
                let v = val!();
                let (p, u) = v.split_once('=').unwrap_or_else(|| usage_and_exit());
                let principal = Principal::from_str(p).unwrap_or_else(|_| usage_and_exit());
                let url = Url::parse(u).unwrap_or_else(|_| usage_and_exit());
                args.addrs.push((principal, url));
            }
            "--cert" => args.cert = Some(PathBuf::from(val!())),
            "--key" => args.key = Some(PathBuf::from(val!())),
            "--trust-cert" => args.trust_cert = Some(PathBuf::from(val!())),
            "--out-pubkey" => args.out_pubkey = Some(PathBuf::from(val!())),
            "--out-cert" => args.out_cert = Some(PathBuf::from(val!())),
            "--out-key" => args.out_key = Some(PathBuf::from(val!())),
            _ => usage_and_exit(),
        }
    }
    if args.role.is_none() {
        usage_and_exit();
    }
    args
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let role = args.role.expect("--role checked in parse_args");
    let me = Principal::from_str(role.principal_str())?;

    // Ed25519 keypair.
    let mut rng = rand::rngs::OsRng;
    let dalek = ed25519_dalek::SigningKey::generate(&mut rng);
    let my_sk = FampSigningKey::from_bytes(dalek.to_bytes());
    let my_vk: TrustedVerifyingKey = my_sk.verifying_key();

    // Dump self pubkey (base64url no-pad).
    if let Some(path) = &args.out_pubkey {
        std::fs::write(path, URL_SAFE_NO_PAD.encode(my_vk.as_bytes()))?;
    }

    // TLS cert (load-or-generate).
    let (cert_path, key_path) = match (args.cert.as_deref(), args.key.as_deref()) {
        (Some(c), Some(k)) => (c.to_path_buf(), k.to_path_buf()),
        _ => {
            let ck = generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])?;
            let slug = role.principal_str().replace([':', '/'], "_");
            let cp = args
                .out_cert
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join(format!("{slug}.crt")));
            let kp = args
                .out_key
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join(format!("{slug}.key")));
            std::fs::write(&cp, ck.cert.pem())?;
            std::fs::write(&kp, ck.signing_key.serialize_pem())?;
            eprintln!("generated cert: {}", cp.display());
            eprintln!("generated key:  {}", kp.display());
            (cp, kp)
        }
    };

    // Keyring (pin self + peers).
    let mut keyring = Keyring::new().with_peer(me.clone(), my_vk.clone())?;
    for (p, key_bytes) in &args.peers {
        let vk = TrustedVerifyingKey::from_bytes(key_bytes)?;
        keyring = keyring.with_peer(p.clone(), vk)?;
    }
    let keyring = Arc::new(keyring);

    // HttpTransport with D-B5 rustls client (trusts OS roots + peer cert).
    let transport = HttpTransport::new_client_only(args.trust_cert.as_deref())?;
    transport.register(me.clone()).await;
    for (p, url) in &args.addrs {
        transport.add_peer(p.clone(), url.clone()).await;
    }

    // Server config + router.
    let server_cert = tls::load_pem_cert(&cert_path)?;
    let server_key = tls::load_pem_key(&key_path)?;
    let server_config = tls::build_server_config(server_cert, server_key)?;
    let router = build_router(keyring.clone(), transport.inboxes());

    // Bind a std::net::TcpListener first so we can read local_addr BEFORE
    // spawning (for ephemeral ports / subprocess sync beacon, D-E6).
    let listen_addr = args
        .listen
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], role.default_port())));
    let std_listener = std::net::TcpListener::bind(listen_addr)?;
    std_listener.set_nonblocking(true)?;
    let bound = std_listener.local_addr()?;
    eprintln!("LISTENING https://{bound}");

    let handle = tls_server::serve_std_listener(std_listener, router, Arc::new(server_config));
    transport.attach_server(handle).await;

    // Drive the cycle via the shared helper.
    let alice_p = Principal::from_str("agent:local/alice")?;
    let bob_p = Principal::from_str("agent:local/bob")?;
    let trace: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));
    match role {
        Role::Alice => {
            cycle_driver::drive_alice(&transport, &keyring, &alice_p, &bob_p, &my_sk, &trace)
                .await?;
        }
        Role::Bob => {
            cycle_driver::drive_bob(&transport, &keyring, &bob_p, &alice_p, &my_sk, &trace).await?;
        }
    }

    println!("[done] {me} exiting 0");
    Ok(())
}
