//! CONF-05/06/07 adversarial integration tests over `MemoryTransport`.
//!
//! Uses the `test-util` feature of `famp-transport` which is enabled ONLY
//! in `[dev-dependencies]` of `crates/famp/Cargo.toml`. Production builds
//! cannot link `send_raw_for_test`.
//!
//! Each CONF case asserts a DISTINCT [`RuntimeError`] variant — the
//! load-bearing guarantee for D-D8 (CONF-06 vs CONF-07 must never
//! collapse into the same error).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::similar_names,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

use ed25519_dalek as _;
use rand as _;
use thiserror as _;

use famp::runtime::{process_one_message, RuntimeError};
use famp_canonical::{canonicalize, from_slice_strict};
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{AckBody, AckDisposition, Bounds, Budget, RequestBody};
use famp_envelope::{EnvelopeDecodeError, SignedEnvelope, Timestamp, UnsignedEnvelope};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::{MemoryTransport, Transport, TransportMessage};
use std::path::PathBuf;
use std::str::FromStr;

// ---------- Test harness ----------

// Deterministic seeds — reused across tests for reproducibility.
const ALICE_SECRET: [u8; 32] = [1u8; 32];
const WRONG_SECRET: [u8; 32] = [99u8; 32];

struct Harness {
    transport: MemoryTransport,
    bob_keyring: Keyring,
    alice: Principal,
    bob: Principal,
    fsm: TaskFsm,
}

async fn setup_two_agents() -> Harness {
    let alice = Principal::from_str("agent:local/alice").unwrap();
    let bob = Principal::from_str("agent:local/bob").unwrap();

    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    let bob_keyring = Keyring::new().with_peer(alice.clone(), alice_vk).unwrap();

    let transport = MemoryTransport::new();
    transport.register(alice.clone()).await;
    transport.register(bob.clone()).await;

    Harness {
        transport,
        bob_keyring,
        alice,
        bob,
        fsm: TaskFsm::new(),
    }
}

fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "100".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: None,
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: None,
    }
}

fn fixed_msg_id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}

/// Re-parse + re-canonicalize `SignedEnvelope` wire bytes so they satisfy
/// the runtime's RFC-8785 canonical pre-check.
fn canonical_bytes<B: famp_envelope::BodySchema>(signed: &SignedEnvelope<B>) -> Vec<u8> {
    let encoded = signed.encode().unwrap();
    let value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    canonicalize(&value).unwrap()
}

// ---------- Compile-time gate: test-util feature must be linked ----------

/// If `famp-transport`'s `test-util` feature is accidentally removed from
/// `[dev-dependencies]` of the top `famp` crate, this function fails to
/// compile — the call to `send_raw_for_test` is behind `#[cfg(feature =
/// "test-util")]` in `famp-transport`.
#[allow(dead_code)]
async fn _require_test_util(t: &MemoryTransport, m: TransportMessage) {
    let _ = t.send_raw_for_test(m).await;
}

// ---------- CONF-05: unsigned envelope rejected ----------

#[tokio::test]
async fn conf_05_unsigned_message_rejected() {
    let mut h = setup_two_agents().await;

    // Build a valid ACK envelope Value, remove the signature field, then
    // canonicalize. The canonical pre-check will accept the canonical form,
    // decode runs, sees no `signature` key, fails with MissingSignature.
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let signed = UnsignedEnvelope::<AckBody>::new(
        fixed_msg_id(),
        h.alice.clone(),
        h.bob.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&alice_sk)
    .unwrap();
    let encoded = signed.encode().unwrap();
    let mut value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    value.as_object_mut().unwrap().remove("signature");
    let bytes = canonicalize(&value).unwrap();

    h.transport
        .send_raw_for_test(TransportMessage {
            sender: h.alice.clone(),
            recipient: h.bob.clone(),
            bytes,
        })
        .await
        .unwrap();
    let msg = h.transport.recv(&h.bob).await.unwrap();
    let result = process_one_message(&msg, &h.bob_keyring, &mut h.fsm);
    assert!(
        matches!(
            result,
            Err(RuntimeError::Decode(EnvelopeDecodeError::MissingSignature))
        ),
        "CONF-05 expected Decode(MissingSignature), got: {result:?}"
    );
}

// ---------- CONF-06: wrong-key signature rejected ----------

#[tokio::test]
async fn conf_06_wrong_key_signature_rejected() {
    let mut h = setup_two_agents().await;

    // Build a valid request envelope but sign it with WRONG_SECRET.
    // Bob's keyring only holds alice's REAL key, so signature verification
    // inside decode fails with SignatureInvalid.
    let wrong_sk = FampSigningKey::from_bytes(WRONG_SECRET);
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<RequestBody>::new(
        fixed_msg_id(),
        h.alice.clone(),
        h.bob.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&wrong_sk)
    .unwrap();
    let bytes = canonical_bytes(&signed);

    h.transport
        .send_raw_for_test(TransportMessage {
            sender: h.alice.clone(),
            recipient: h.bob.clone(),
            bytes,
        })
        .await
        .unwrap();
    let msg = h.transport.recv(&h.bob).await.unwrap();
    let result = process_one_message(&msg, &h.bob_keyring, &mut h.fsm);
    assert!(
        matches!(
            result,
            Err(RuntimeError::Decode(EnvelopeDecodeError::SignatureInvalid))
        ),
        "CONF-06 expected Decode(SignatureInvalid), got: {result:?}"
    );
}

// ---------- CONF-07: canonical divergence rejected ----------

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("conf-07-canonical-divergence.json")
}

/// Generate CONF-07 fixture deterministically from ALICE_SECRET. The bytes
/// are a valid signed request envelope, but pretty-printed with whitespace
/// so `wire_bytes != canonicalize(parse(wire_bytes))`. The signature is
/// still valid for the canonical form (verify runs over the stripped
/// Value, which canonicalizes internally).
fn generate_conf_07_bytes() -> Vec<u8> {
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<RequestBody>::new(
        fixed_msg_id(),
        Principal::from_str("agent:local/alice").unwrap(),
        Principal::from_str("agent:local/bob").unwrap(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&alice_sk)
    .unwrap();
    let encoded = signed.encode().unwrap();
    let value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    // Pretty-print: adds whitespace/newlines that canonical form does not
    // have. Guaranteed byte-inequal to `canonicalize(value)`.
    let pretty = serde_json::to_vec_pretty(&value).unwrap();

    // Sanity: pretty MUST differ from canonical, else the test is bogus.
    let canonical = canonicalize(&value).unwrap();
    assert_ne!(
        pretty, canonical,
        "fixture generator failed to produce divergent bytes"
    );
    pretty
}

#[tokio::test]
async fn conf_07_canonical_divergence_rejected() {
    let mut h = setup_two_agents().await;

    // Ensure the fixture file exists on disk, regenerate if missing or stale.
    // Committing it is required by the plan acceptance criteria, but the
    // file can be re-derived from fixed seeds if a future refactor
    // invalidates the on-disk bytes.
    let path = fixture_path();
    let bytes = if path.exists() {
        std::fs::read(&path).unwrap()
    } else {
        let generated = generate_conf_07_bytes();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, &generated).unwrap();
        generated
    };

    // Belt-and-braces: if the fixture survived a seed change and is now
    // canonical by accident, fail loudly rather than silently passing.
    let parsed: serde_json::Value = from_slice_strict(&bytes).unwrap();
    let canonical = canonicalize(&parsed).unwrap();
    assert_ne!(
        bytes, canonical,
        "CONF-07 fixture is canonical — regenerate it"
    );

    h.transport
        .send_raw_for_test(TransportMessage {
            sender: h.alice.clone(),
            recipient: h.bob.clone(),
            bytes,
        })
        .await
        .unwrap();
    let msg = h.transport.recv(&h.bob).await.unwrap();
    let result = process_one_message(&msg, &h.bob_keyring, &mut h.fsm);
    assert!(
        matches!(result, Err(RuntimeError::CanonicalDivergence)),
        "CONF-07 expected CanonicalDivergence, got: {result:?}"
    );
}
