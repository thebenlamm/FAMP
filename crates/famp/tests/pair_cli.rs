//! PAIR-04/05/07/08 CLI-level tests (18-03).
//!
//! Structured the way `peer_rotate_cli.rs` is: in-process `run_at` cases
//! mixed with process-level `assert_cmd` cases in one binary,
//! `tempfile::tempdir()` per home, literal timestamps, and a seeded
//! `StdRng` for `invite::run_at`'s code draw.
//!
//! `spawn_mock_inviter` stands in for `famp_gateway::pairing_ingress`'s
//! real redemption endpoint. The `famp` crate deliberately does not (and
//! must not) depend on `famp-gateway` -- that dependency edge runs the
//! other way (`famp-gateway` depends on `famp`) -- so this file cannot
//! reuse the real `ingest_redemption` the way
//! `famp-gateway/tests/pairing_e2e.rs` does. The mock skips code/attempt
//! validation (already exhaustively covered by `pairing_ingress.rs` and
//! `pairing_e2e.rs` in the gateway crate) and exists only to prove this
//! crate's OWN wiring: that `redeem::run_at` and `status::run_at` produce
//! the right CLI-level artifacts and pin the right keys when a redemption
//! succeeds or fails in each of the ways this crate is responsible for.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use assert_cmd::Command as AssertCommand;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use famp::cli::pair::{invite, redeem, status};
use famp::cli::peer::identity::{self, gateway_identity_path, gateway_peers_keyring_path};
use famp::pairing::consent::CONSENT_WARNING;
use famp::pairing::invite::{pairing_store_path, InviteState, InviteStore};
use famp::pairing::{PairingError, RedemptionRequest, RedemptionResponse, Signed};
use famp_core::Principal;
use famp_crypto::{key_id, FampSigningKey};
use famp_keyring::{KeyLookupOutcome, Keyring};
use rand::{rngs::StdRng, SeedableRng};
use tempfile::TempDir;

// ── Shared fixtures ─────────────────────────────────────────────────────

/// Write `$FAMP_HOME/own-domain` -- the file-based own-domain resolution
/// tier, same convention `pairing_e2e.rs` uses to give two homes distinct
/// domains without racing a process-global env var.
fn set_own_domain(home: &Path, domain: &str) {
    std::fs::write(home.join("own-domain"), format!("{domain}\n")).unwrap();
}

/// Build the TLS-preconfigured outbound `reqwest::Client` the SAME way
/// `cli::pair::redeem`'s own (private) `build_client` does.
fn stub_client() -> reqwest::Client {
    let tls = famp_transport_http::tls::build_client_config(None).unwrap();
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .unwrap()
}

/// Create a `Pending` invite via `invite::run_at` with a seeded RNG (so the
/// drawn code is deterministic) and `--confirm-installed` implied.
/// Returns the full printed artifact string and the invite record's `id`.
fn create_invite(
    home: &Path,
    as_principal: &str,
    url: Option<&str>,
    now: &str,
    seed: u64,
) -> (String, String) {
    let mut artifact_bytes = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);
    invite::run_at(
        home,
        &invite::PairInviteArgs {
            as_principal: as_principal.to_string(),
            url: url.map(str::to_string),
            confirm_installed: true,
        },
        &mut artifact_bytes,
        now,
        &mut rng,
    )
    .expect("invite::run_at must succeed");
    let artifact = String::from_utf8(artifact_bytes).unwrap();
    let store = InviteStore::load(&pairing_store_path(home)).unwrap();
    let id = store.invites[0].id.clone();
    (artifact, id)
}

/// The final non-empty line of an invite artifact -- the five-word code.
fn code_line(artifact: &str) -> String {
    artifact.trim_end().lines().last().unwrap().to_string()
}

/// In-process test double for the inviter's `/famp/v1/pair/redeem` route.
/// Accepts ANY well-formed signed request (no code/attempt validation --
/// see this file's module doc), marks the matching `Pending` record
/// `Redeemed` on disk exactly as the real endpoint would, and returns a
/// `Signed<RedemptionResponse>` signed with the inviter's OWN gateway
/// identity key so `redeem::run_at`'s proof-of-possession check succeeds.
#[derive(Clone)]
struct MockInviter {
    home: std::path::PathBuf,
    sk: Arc<FampSigningKey>,
    principal: String,
}

async fn mock_redeem_handler(
    State(state): State<MockInviter>,
    body: axum::body::Bytes,
) -> Json<Signed<RedemptionResponse>> {
    let signed: Signed<RedemptionRequest> =
        serde_json::from_slice(&body).expect("mock inviter received a malformed request body");
    let redeemer_vk =
        famp_crypto::TrustedVerifyingKey::from_b64url(&signed.statement.pubkey_b64url)
            .expect("mock inviter received a malformed redeemer pubkey");
    let redeemer_key_id = key_id(&redeemer_vk);

    let store_path = pairing_store_path(&state.home);
    let mut store = InviteStore::load(&store_path).unwrap();
    for record in &mut store.invites {
        if matches!(record.state, InviteState::Pending) {
            record.state = InviteState::Redeemed {
                by: signed.statement.principal.clone(),
                key_id: redeemer_key_id.clone(),
                pubkey_b64url: signed.statement.pubkey_b64url.clone(),
                at: "2030-08-03T00:05:00Z".to_string(),
            };
        }
    }
    store.save_atomic(&store_path).unwrap();

    let vk = state.sk.verifying_key();
    let response = RedemptionResponse {
        invite_id: "mock-invite".to_string(),
        inviter_principal: state.principal.clone(),
        inviter_pubkey_b64url: vk.to_b64url(),
        redeemer_key_id,
        at: "2030-08-03T00:05:00Z".to_string(),
    };
    Json(Signed::new(response, &state.sk).unwrap())
}

/// Spawn [`MockInviter`] on an ephemeral loopback TCP port. Returns the
/// base URL; the join handle is dropped, letting the server task run for
/// the lifetime of the test's tokio runtime (same convention
/// `pairing_e2e.rs::spawn_pairing_server` uses).
async fn spawn_mock_inviter(home: &Path, principal: &str) -> String {
    let sk = identity::load_or_generate(&gateway_identity_path(home)).unwrap();
    let state = MockInviter {
        home: home.to_path_buf(),
        sk: Arc::new(sk),
        principal: principal.to_string(),
    };
    let router = Router::new()
        .route("/famp/v1/pair/redeem", post(mock_redeem_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral loopback port");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

// ── PAIR-04 / PAIR-08: the one artifact (Task 2 coverage, integration level) ──

#[test]
fn artifact_code_offset_greater_than_consent_and_install_lines() {
    let tmp = TempDir::new().unwrap();
    let (artifact, _id) = create_invite(
        tmp.path(),
        "agent:inviter.test/gateway",
        Some("https://gateway.inviter.test:8443"),
        "2026-08-03T00:00:00Z",
        1,
    );
    let code = code_line(&artifact);
    let code_offset = artifact.rfind(&code).unwrap();
    let consent_offset = artifact
        .find(CONSENT_WARNING)
        .expect("consent warning must appear in the artifact");
    let install_offset = artifact
        .find("famp --version")
        .expect("the install-check line must appear in the artifact");
    assert!(
        code_offset > consent_offset,
        "code offset {code_offset} must be strictly greater than consent offset {consent_offset}"
    );
    assert!(
        code_offset > install_offset,
        "code offset {code_offset} must be strictly greater than install offset {install_offset}"
    );
}

#[test]
fn artifact_code_line_is_final_non_empty_line() {
    let tmp = TempDir::new().unwrap();
    let (artifact, _id) = create_invite(
        tmp.path(),
        "agent:inviter.test/gateway",
        Some("https://gateway.inviter.test:8443"),
        "2026-08-03T00:00:00Z",
        1,
    );
    let code = code_line(&artifact);
    assert_eq!(code.split(' ').count(), 5, "code must be 5 words: {code}");
    assert!(
        artifact.trim_end().ends_with(&code),
        "the code must be the final non-empty line of the artifact"
    );
}

#[test]
fn artifact_no_long_base64url_token_excluding_https() {
    let tmp = TempDir::new().unwrap();
    let (artifact, _id) = create_invite(
        tmp.path(),
        "agent:inviter.test/gateway",
        Some("https://gateway.inviter.test:8443"),
        "2026-08-03T00:00:00Z",
        1,
    );
    for token in artifact.split_whitespace() {
        if token.starts_with("https://") {
            continue;
        }
        let is_base64url_alphabet = !token.is_empty()
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if is_base64url_alphabet {
            assert!(
                token.len() < 16,
                "token '{token}' is {} chars of pure base64url alphabet -- looks like key \
                 material",
                token.len()
            );
        }
    }
}

#[test]
fn artifact_does_not_contain_invite_id() {
    let tmp = TempDir::new().unwrap();
    let (artifact, id) = create_invite(
        tmp.path(),
        "agent:inviter.test/gateway",
        Some("https://gateway.inviter.test:8443"),
        "2026-08-03T00:00:00Z",
        1,
    );
    assert!(
        !artifact.contains(&id),
        "the artifact must never contain the invite record's id"
    );
}

#[test]
fn process_level_invite_without_confirm_installed_exits_2() {
    let famp_home = TempDir::new().unwrap();
    let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
    let assert = cmd
        .env("FAMP_HOME", famp_home.path())
        .arg("pair")
        .arg("invite")
        .arg("--as")
        .arg("agent:x.test/gateway")
        .assert();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit code 2, got: {output:?}"
    );
}

// ── PAIR-07: observe-before-pin, asymmetric done-signals ───────────────

/// Wraps a `Write` sink and, on the FIRST call to `write`, snapshots
/// `keyring_path`'s current bytes. `status::run_at` writes the
/// `REDEEMED BY:` identity line in exactly one `write_all` call before any
/// keyring mutation (see `status.rs::pin_redeemed_record`), so the first
/// write this wrapper observes is that line -- letting this test prove
/// the keyring is byte-identical AT THE MOMENT it is emitted, not merely
/// "before `run_at` returns".
struct SnapshotOnFirstWrite<'a> {
    inner: Vec<u8>,
    keyring_path: &'a Path,
    snapshot: Option<Vec<u8>>,
}

impl Write for SnapshotOnFirstWrite<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.snapshot.is_none() {
            self.snapshot = Some(std::fs::read(self.keyring_path).unwrap_or_default());
        }
        self.inner.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn seed_redeemed_record(home: &Path, invite_id: &str) -> famp_crypto::TrustedVerifyingKey {
    let sk = FampSigningKey::from_bytes([7u8; 32]);
    let vk = sk.verifying_key();
    InviteStore {
        invites: vec![famp::pairing::invite::InviteRecord {
            id: invite_id.to_string(),
            principal: "agent:inviter.test/gateway".to_string(),
            code_digest: "c".repeat(64),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            expires_at: "2026-08-04T00:00:00Z".to_string(),
            attempts: 0,
            state: InviteState::Redeemed {
                by: "agent:redeemer.test/gateway".to_string(),
                key_id: key_id(&vk),
                pubkey_b64url: vk.to_b64url(),
                at: "2026-08-03T01:00:00Z".to_string(),
            },
        }],
    }
    .save_atomic(&pairing_store_path(home))
    .unwrap();
    vk
}

#[test]
fn status_observe_before_pin_keyring_unchanged_at_write_time() {
    let tmp = TempDir::new().unwrap();

    // Seed the keyring with an UNRELATED principal first, so it is
    // non-empty and would visibly change (grow) if the pin ran before the
    // observe line -- a snapshot that happened to be taken on an
    // already-empty file would pass trivially even under the broken
    // ordering.
    let other_export_home = TempDir::new().unwrap();
    let other_args = famp::cli::peer::export::PeerExportArgs {
        as_principal: "agent:other.test/gateway".to_string(),
    };
    let mut other_blob = Vec::new();
    famp::cli::peer::export::run_at(other_export_home.path(), &other_args, &mut other_blob)
        .unwrap();
    famp::cli::peer::import::run_at(tmp.path(), &mut Cursor::new(other_blob)).unwrap();

    let keyring_path = gateway_peers_keyring_path(tmp.path());
    let pre_run_bytes = std::fs::read(&keyring_path).unwrap();
    assert!(
        !pre_run_bytes.is_empty(),
        "fixture must seed a non-empty keyring"
    );

    seed_redeemed_record(tmp.path(), "inv-observe-before-pin");

    let mut writer = SnapshotOnFirstWrite {
        inner: Vec::new(),
        keyring_path: &keyring_path,
        snapshot: None,
    };
    status::run_at(tmp.path(), &mut writer, "2026-08-03T02:00:00Z", false).unwrap();

    let printed = String::from_utf8(writer.inner).unwrap();
    assert!(
        printed.starts_with("REDEEMED BY: "),
        "the first write must be the identity line: {printed}"
    );

    assert_eq!(
        writer.snapshot.unwrap(),
        pre_run_bytes,
        "the keyring file must be byte-identical to its pre-run state at the moment the \
         REDEEMED BY: line is emitted"
    );

    // Positive control: the pin DID still happen by the time `run_at`
    // returns -- proving this isn't a no-op keyring path.
    let post_run_bytes = std::fs::read(&keyring_path).unwrap();
    assert_ne!(
        post_run_bytes, pre_run_bytes,
        "the pin must still land on disk by the time run_at returns"
    );
}

#[test]
fn status_redeemed_by_line_has_principal_and_key_id() {
    let tmp = TempDir::new().unwrap();
    let vk = seed_redeemed_record(tmp.path(), "inv-line-shape");
    let mut out = Vec::new();
    status::run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z", false).unwrap();
    let printed = String::from_utf8(out).unwrap();
    let line = printed
        .lines()
        .find(|l| l.starts_with("REDEEMED BY: "))
        .expect("expected a REDEEMED BY: line");
    assert!(line.contains("agent:redeemer.test/gateway"));
    assert!(line.contains(&format!("key_id={}", key_id(&vk))));
}

#[test]
fn status_no_redeemed_records_is_ok_and_keyring_untouched() {
    let tmp = TempDir::new().unwrap();
    InviteStore {
        invites: vec![famp::pairing::invite::InviteRecord {
            id: "inv-pending".to_string(),
            principal: "agent:inviter.test/gateway".to_string(),
            code_digest: "d".repeat(64),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            expires_at: "2026-08-04T00:00:00Z".to_string(),
            attempts: 0,
            state: InviteState::Pending,
        }],
    }
    .save_atomic(&pairing_store_path(tmp.path()))
    .unwrap();

    let mut out = Vec::new();
    status::run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z", false)
        .expect("zero Redeemed records is Ok, not an error");
    assert!(!gateway_peers_keyring_path(tmp.path()).exists());
}

// ── PAIR-05: jargon-free, doc-synced error taxonomy ─────────────────────

#[test]
fn pair_errors_avoid_jargon() {
    let jargon = ["public key", "fingerprint", "Ed25519", "keyring", "base64"];
    let messages = [
        PairingError::CodeMalformed {
            reason: "any".to_string(),
        }
        .to_string(),
        PairingError::WrongCode.to_string(),
        PairingError::Expired.to_string(),
        PairingError::AlreadyRedeemed.to_string(),
        PairingError::AttemptsExhausted.to_string(),
        PairingError::GatewayUnreachable {
            url: "https://gateway.example.test:8443".to_string(),
        }
        .to_string(),
        PairingError::SameMachineRefusal.to_string(),
    ];
    for message in &messages {
        for term in jargon {
            assert!(
                !message.contains(term),
                "message '{message}' contains jargon term '{term}'"
            );
        }
    }
}

/// A clean grep is not proof of absence (this repo has been bitten by
/// exactly that before). Sanity-check the jargon list and matcher against
/// a KNOWN-POSITIVE string before trusting the zero above.
#[test]
fn pair_errors_avoid_jargon_sanity_check_catches_known_positive() {
    let jargon = ["public key", "fingerprint", "Ed25519", "keyring", "base64"];
    let known_positive =
        "Compare the public key fingerprint using Ed25519 against your keyring, base64-encoded.";
    let hits: Vec<&str> = jargon
        .iter()
        .copied()
        .filter(|term| known_positive.contains(term))
        .collect();
    assert_eq!(
        hits.len(),
        jargon.len(),
        "the known-positive fixture must trip every jargon term, or the matcher/list is broken: \
         hit {hits:?} of {jargon:?}"
    );
}

#[test]
fn consent_warning_matches_quarantine_doc() {
    let quarantine_doc = include_str!("../../../docs/QUARANTINE.md");
    assert!(
        quarantine_doc.contains(CONSENT_WARNING),
        "docs/QUARANTINE.md must contain CONSENT_WARNING's exact bytes"
    );
}

// ── Invariant: pinned principal == send's `from` for the same identity ──

/// The real defect this fixes: `redeem::run_at` proposes a principal for
/// pinning that must equal the `from` principal `cli::send`'s (private)
/// `build_remote_envelope_value` (`send/mod.rs:679`) constructs for the
/// SAME identity -- `agent:{own_domain}/{identity}` -- or the follower's
/// later `famp send` never matches what the inviter pinned and every
/// follower-to-inviter envelope is rejected `UnpinnedKey`.
///
/// `build_remote_envelope_value` is private to `cli::send::mod`, so this
/// asserts against its exact construction (copied verbatim below) rather
/// than calling it -- a future divergence in that construction must be
/// caught here, not silently untested.
#[tokio::test]
async fn redeem_pins_principal_matching_send_from_for_same_identity() {
    let inviter_home = TempDir::new().unwrap();
    let redeemer_home = TempDir::new().unwrap();
    let own_domain = "alice-host.test";
    set_own_domain(redeemer_home.path(), own_domain);

    let (artifact, _id) = create_invite(
        inviter_home.path(),
        "agent:inviter-invariant.test/gateway",
        None,
        "2030-08-03T00:00:00Z",
        42,
    );
    let code = code_line(&artifact);
    let base_url =
        spawn_mock_inviter(inviter_home.path(), "agent:inviter-invariant.test/gateway").await;

    let client = stub_client();
    let mut reader = Cursor::new(format!("{code}\n").into_bytes());
    let identity = "alice";
    redeem::run_at(
        redeemer_home.path(),
        &redeem::PairRedeemArgs {
            confirm_key_change: false,
            from: base_url,
            as_identity: identity.to_string(),
            trust_cert: None,
        },
        &mut reader,
        &client,
        "2030-08-03T00:05:00Z",
    )
    .await
    .expect("redeem::run_at must succeed against the mock inviter");

    // What the redeemer actually submitted as its own principal --
    // captured by `MockInviter` into the invite record's `by` field, the
    // same value `redeem::run_at` will pin the inviter under once
    // `status::run_at` runs.
    let store = InviteStore::load(&pairing_store_path(inviter_home.path())).unwrap();
    let submitted_principal = match &store.invites[0].state {
        InviteState::Redeemed { by, .. } => by.clone(),
        other => panic!("expected Redeemed state, got {other:?}"),
    };

    // `send/mod.rs:679`'s exact construction, copied verbatim as the
    // source of truth this test pins against.
    let expected_send_from = format!("agent:{own_domain}/{identity}");

    assert_eq!(
        submitted_principal, expected_send_from,
        "pair redeem must submit the SAME principal `famp send`'s \
         build_remote_envelope_value (send/mod.rs:679) uses as `from` for identity \
         '{identity}': pinned='{submitted_principal}' expected='{expected_send_from}'"
    );
}

// ── PAIR-05: redeem-path failure mapping ────────────────────────────────

#[tokio::test]
async fn redeem_malformed_code_rejected_before_network_call() {
    let redeemer_home = TempDir::new().unwrap();
    let client = stub_client();
    // Four words, not five: fails parse_code's shape check client-side.
    let mut reader = Cursor::new(b"not a real code\n".to_vec());
    let err = redeem::run_at(
        redeemer_home.path(),
        &redeem::PairRedeemArgs {
            confirm_key_change: false,
            from: "http://127.0.0.1:1".to_string(),
            as_identity: "redeemer".to_string(),
            trust_cert: None,
        },
        &mut reader,
        &client,
        "2030-08-03T00:00:00Z",
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("does not look like a pairing code"),
        "expected the malformed-code message (proving the client-side check ran BEFORE any \
         network call to the deliberately-unreachable --from URL), got: {msg}"
    );
}

#[tokio::test]
async fn redeem_gateway_unreachable_message_interpolates_url() {
    let redeemer_home = TempDir::new().unwrap();
    set_own_domain(redeemer_home.path(), "redeemer3.test");
    let client = stub_client();
    let mut reader = Cursor::new(b"abandon ability able about above\n".to_vec());
    let from = "http://127.0.0.1:1".to_string();
    let err = redeem::run_at(
        redeemer_home.path(),
        &redeem::PairRedeemArgs {
            confirm_key_change: false,
            from: from.clone(),
            as_identity: "redeemer".to_string(),
            trust_cert: None,
        },
        &mut reader,
        &client,
        "2030-08-03T00:00:00Z",
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Could not reach"), "got: {msg}");
    assert!(
        msg.contains(&from),
        "expected the --from URL interpolated into the message, got: {msg}"
    );
}

// ── PAIR-01/PAIR-07 restated at the CLI level ───────────────────────────

#[tokio::test]
async fn two_home_mutual_pin_via_cli() {
    let inviter_home = TempDir::new().unwrap();
    let redeemer_home = TempDir::new().unwrap();
    set_own_domain(redeemer_home.path(), "redeemer.test");

    let (artifact, _id) = create_invite(
        inviter_home.path(),
        "agent:inviter.test/gateway",
        None,
        "2030-08-03T00:00:00Z",
        7,
    );
    let code = code_line(&artifact);

    let base_url = spawn_mock_inviter(inviter_home.path(), "agent:inviter.test/gateway").await;

    let client = stub_client();
    let mut reader = Cursor::new(format!("{code}\n").into_bytes());
    redeem::run_at(
        redeemer_home.path(),
        &redeem::PairRedeemArgs {
            confirm_key_change: false,
            from: base_url,
            as_identity: "gateway".to_string(),
            trust_cert: None,
        },
        &mut reader,
        &client,
        "2030-08-03T00:05:00Z",
    )
    .await
    .expect("redeem::run_at must succeed against the mock inviter");

    let mut status_out = Vec::new();
    status::run_at(
        inviter_home.path(),
        &mut status_out,
        "2030-08-03T00:10:00Z",
        false,
    )
    .expect("status::run_at must pin the redeemer's key");

    let inviter_principal: Principal = "agent:inviter.test/gateway".parse().unwrap();
    let redeemer_principal: Principal = "agent:redeemer.test/gateway".parse().unwrap();

    let redeemer_keyring =
        Keyring::load_from_file(&gateway_peers_keyring_path(redeemer_home.path())).unwrap();
    assert!(
        matches!(
            redeemer_keyring.active_key(&inviter_principal, "2030-08-03T00:10:00Z"),
            KeyLookupOutcome::Active(_)
        ),
        "redeemer's keyring must hold the inviter's key as Active"
    );

    let inviter_keyring =
        Keyring::load_from_file(&gateway_peers_keyring_path(inviter_home.path())).unwrap();
    assert!(
        matches!(
            inviter_keyring.active_key(&redeemer_principal, "2030-08-03T00:10:00Z"),
            KeyLookupOutcome::Active(_)
        ),
        "inviter's keyring must hold the redeemer's key as Active"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redeem_success_done_signal_is_single_sentence_no_brace() {
    let inviter_home = TempDir::new().unwrap();
    let redeemer_home = TempDir::new().unwrap();
    set_own_domain(redeemer_home.path(), "redeemer2.test");

    let (artifact, _id) = create_invite(
        inviter_home.path(),
        "agent:inviter2.test/gateway",
        None,
        "2030-08-03T00:00:00Z",
        9,
    );
    let code = code_line(&artifact);
    let base_url = spawn_mock_inviter(inviter_home.path(), "agent:inviter2.test/gateway").await;

    let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
    let assert = cmd
        .env("FAMP_HOME", redeemer_home.path())
        .arg("pair")
        .arg("redeem")
        .arg("--from")
        .arg(&base_url)
        .arg("--as")
        .arg("redeemer2")
        .write_stdin(format!("{code}\n"))
        .assert();
    let output = assert.get_output();
    assert!(output.status.success(), "expected success, got: {output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let done_line = stderr
        .lines()
        .find(|l| l.starts_with("Paired with"))
        .unwrap_or_else(|| panic!("expected a 'Paired with' line in stderr: {stderr}"));
    assert!(
        !done_line.contains('{'),
        "done signal must not contain '{{': {done_line}"
    );
    // "Single sentence" can't be a raw '.' count -- a principal like
    // `agent:inviter2.test/gateway` legitimately contains a period that
    // is not a sentence boundary. A real second sentence would show up as
    // ". " (period then whitespace) somewhere before the very end; the
    // one terminal period is exempted by trimming it off first.
    let body = done_line
        .trim_end_matches('.')
        .trim_end_matches(|c: char| c.is_whitespace());
    assert!(
        !body.contains(". "),
        "done signal must be a single sentence (found a second sentence boundary): {done_line}"
    );
    assert!(
        done_line.ends_with('.'),
        "done signal must end with a period: {done_line}"
    );
}

/// P3: a pin that would REPLACE an existing Active key under the same
/// principal must be refused unless the operator explicitly opts in.
///
/// `rotate_to`'s `confirmed` parameter silently retires the existing Active
/// entry and pins the incoming key when it is `true`
/// (`famp-keyring/src/lib.rs`'s rotation contract). Both pair call sites used
/// to hardcode `true`, so — now that `--as` is caller-controlled — anyone
/// holding a valid invite code could take over an already-pinned principal.
///
/// Seeds a DIFFERENT key under the exact principal `status` will try to pin,
/// so the rotation reaches the `confirmed` check rather than returning early
/// via `FirstPin` or `AlreadyPinned`.
fn seed_conflicting_pin(home: &Path) {
    let victim_home = TempDir::new().unwrap();
    let args = famp::cli::peer::export::PeerExportArgs {
        as_principal: "agent:redeemer.test/gateway".to_string(),
    };
    let mut blob = Vec::new();
    famp::cli::peer::export::run_at(victim_home.path(), &args, &mut blob).unwrap();
    famp::cli::peer::import::run_at(home, &mut Cursor::new(blob)).unwrap();
}

#[test]
fn status_refuses_to_replace_an_existing_pin_without_confirmation() {
    let tmp = TempDir::new().unwrap();
    seed_redeemed_record(tmp.path(), "inv-p3-refuse");
    seed_conflicting_pin(tmp.path());

    let keyring_path = gateway_peers_keyring_path(tmp.path());
    let before = std::fs::read(&keyring_path).unwrap();

    let mut out = Vec::new();
    let err = status::run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z", false)
        .expect_err("replacing an existing Active key without --confirm-key-change must fail");

    let msg = err.to_string();
    assert!(
        msg.contains("--confirm-key-change"),
        "the error must name the remedy, got: {msg}"
    );
    assert!(
        msg.contains("agent:redeemer.test/gateway"),
        "the error must name the affected principal, got: {msg}"
    );

    assert_eq!(
        std::fs::read(&keyring_path).unwrap(),
        before,
        "a refused pin must leave the keyring byte-identical"
    );
}

#[test]
fn status_replaces_an_existing_pin_when_confirmation_is_given() {
    let tmp = TempDir::new().unwrap();
    let incoming_vk = seed_redeemed_record(tmp.path(), "inv-p3-confirm");
    seed_conflicting_pin(tmp.path());

    let mut out = Vec::new();
    status::run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z", true)
        .expect("--confirm-key-change must permit an explicit key replacement");

    let keyring = Keyring::load_from_file(&gateway_peers_keyring_path(tmp.path())).unwrap();
    let principal: Principal = "agent:redeemer.test/gateway".parse().unwrap();
    match keyring.active_key(&principal, "2026-08-03T02:00:01Z") {
        KeyLookupOutcome::Active(vk) => assert_eq!(
            vk.to_b64url(),
            incoming_vk.to_b64url(),
            "the confirmed replacement must leave the INCOMING key Active"
        ),
        other => panic!("expected an Active key after a confirmed rotation, got: {other:?}"),
    }
}
