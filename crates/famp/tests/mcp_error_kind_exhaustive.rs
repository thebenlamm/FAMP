//! Exhaustive test: every `CliError` variant has a `mcp_error_kind()` string,
//! all strings are non-empty, and they are unique across variants.
//!
//! This test is the CI gate that proves the compile-time exhaustive match in
//! `cli::mcp::error_kind` does not use a `_ =>` fallback and that every
//! discriminator string is meaningful and distinct.

// Integration test binaries inherit all of famp's transitive deps; silence
// "unused crate" warnings for crates we don't explicitly reference here.
#![allow(unused_crate_dependencies)]
// Test helpers legitimately need unwrap/expect for constructing error payloads.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use famp::cli::error::CliError;

// ── helpers ──────────────────────────────────────────────────────────────────

fn io_err() -> std::io::Error {
    std::io::Error::other("test")
}

fn path(s: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(s)
}

fn make_toml_ser_error() -> toml::ser::Error {
    // `()` cannot be serialized by `toml` because it has no TOML representation.
    toml::to_string(&()).expect_err("() must fail toml serialization")
}

fn make_toml_de_error() -> toml::de::Error {
    toml::from_str::<toml::Value>("!!!invalid").expect_err("invalid TOML must fail")
}

/// Construct the first half of known `CliError` variants (split to stay ≤ 100 lines).
fn variants_a() -> Vec<(&'static str, CliError)> {
    vec![
        ("HomeNotSet", CliError::HomeNotSet),
        (
            "HomeNotAbsolute",
            CliError::HomeNotAbsolute {
                path: path("relative"),
            },
        ),
        (
            "HomeHasNoParent",
            CliError::HomeHasNoParent { path: path("/") },
        ),
        (
            "HomeCreateFailed",
            CliError::HomeCreateFailed {
                path: path("/tmp/x"),
                source: io_err(),
            },
        ),
        (
            "AlreadyInitialized",
            CliError::AlreadyInitialized {
                existing_files: vec![path("a")],
            },
        ),
        (
            "IdentityIncomplete",
            CliError::IdentityIncomplete {
                missing: path("key.ed25519"),
            },
        ),
        ("KeygenFailed", CliError::KeygenFailed(Box::new(io_err()))),
        (
            "CertgenFailed",
            CliError::CertgenFailed(rcgen::Error::CouldNotParseCertificate),
        ),
        (
            "Io",
            CliError::Io {
                path: path("/tmp"),
                source: io_err(),
            },
        ),
        (
            "TomlSerialize",
            CliError::TomlSerialize(make_toml_ser_error()),
        ),
        (
            "TomlParse",
            CliError::TomlParse {
                path: path("peers.toml"),
                source: make_toml_de_error(),
            },
        ),
        (
            "PortInUse",
            CliError::PortInUse {
                addr: "127.0.0.1:8443".parse().expect("valid addr"),
            },
        ),
        (
            "Inbox",
            CliError::Inbox(famp_inbox::InboxError::Io {
                path: path("/tmp/inbox.jsonl"),
                source: io_err(),
            }),
        ),
        (
            "Tls",
            CliError::Tls(famp_transport_http::TlsError::NoPrivateKey),
        ),
    ]
}

/// Construct the second half of known `CliError` variants.
fn variants_b() -> Vec<(&'static str, CliError)> {
    vec![
        (
            "PeerNotFound",
            CliError::PeerNotFound {
                alias: "alice".to_string(),
            },
        ),
        (
            "PeerDuplicate",
            CliError::PeerDuplicate {
                alias: "alice".to_string(),
            },
        ),
        (
            "PeerEndpointInvalid",
            CliError::PeerEndpointInvalid {
                value: "not-a-url".to_string(),
            },
        ),
        (
            "PeerPubkeyInvalid",
            CliError::PeerPubkeyInvalid {
                value: "bad-key".to_string(),
            },
        ),
        (
            "TaskNotFound",
            CliError::TaskNotFound {
                task_id: "abc".to_string(),
            },
        ),
        (
            "TaskTerminal",
            CliError::TaskTerminal {
                task_id: "abc".to_string(),
            },
        ),
        ("SendFailed", CliError::SendFailed(Box::new(io_err()))),
        (
            "TaskDir",
            CliError::TaskDir(famp_taskdir::TaskDirError::NotFound {
                task_id: "abc".to_string(),
            }),
        ),
        ("Envelope", CliError::Envelope(Box::new(io_err()))),
        (
            "TlsFingerprintMismatch",
            CliError::TlsFingerprintMismatch {
                alias: "peer".to_string(),
                pinned: "aa".to_string(),
                got: "bb".to_string(),
            },
        ),
        (
            "SendArgsInvalid",
            CliError::SendArgsInvalid {
                reason: "test".to_string(),
            },
        ),
        (
            "AwaitTimeout",
            CliError::AwaitTimeout {
                timeout: "30s".to_string(),
            },
        ),
        (
            "InvalidDuration",
            CliError::InvalidDuration {
                value: "bad".to_string(),
            },
        ),
        (
            "KeyringBuildFailed",
            CliError::KeyringBuildFailed {
                alias: "peer".to_string(),
                reason: "bad key".to_string(),
            },
        ),
        (
            "TofuBootstrapRefused",
            CliError::TofuBootstrapRefused {
                alias: "peer".to_string(),
                got: "ab".to_string(),
            },
        ),
        (
            "PrincipalInvalid",
            CliError::PrincipalInvalid {
                path: path("/tmp/config.toml"),
                value: "garbage".to_string(),
                reason: "missing scheme".to_string(),
            },
        ),
    ]
}

/// Variants kept out of the original split so neither
/// `variants_a` nor `variants_b` exceeds the 100-line `clippy::pedantic`
/// threshold. Same precedent as the original `_a`/`_b` split (see comment
/// on `variants_a` above).
fn variants_c() -> Vec<(&'static str, CliError)> {
    vec![
        (
            "PeerCardInvalid",
            CliError::PeerCardInvalid {
                reason: "missing pubkey".to_string(),
            },
        ),
        (
            "InvalidAgentName",
            CliError::InvalidAgentName {
                name: "bad name".to_string(),
                reason: "contains whitespace".to_string(),
            },
        ),
        (
            "FsmTransition",
            CliError::FsmTransition(famp_fsm::TaskFsmError::IllegalTransition {
                from: famp_fsm::TaskState::Requested,
                class: famp_core::MessageClass::Deliver,
                terminal_status: Some(famp_core::TerminalStatus::Completed),
            }),
        ),
        (
            "InvalidTaskState",
            CliError::InvalidTaskState {
                value: "BOGUS".to_string(),
            },
        ),
        ("NotRegistered", CliError::NotRegistered),
        (
            "UnknownIdentity",
            CliError::UnknownIdentity {
                name: "alice".to_string(),
            },
        ),
        (
            "InvalidIdentityName",
            CliError::InvalidIdentityName {
                name: "bad name".to_string(),
                reason: "must match [A-Za-z0-9_-]+".to_string(),
            },
        ),
        (
            "NoIdentityBound",
            CliError::NoIdentityBound {
                reason: "no identity bound — pass --as, set $FAMP_LOCAL_IDENTITY, or run \
                         `famp-local wire <dir>` first"
                    .to_string(),
            },
        ),
        (
            "NotRegisteredHint",
            CliError::NotRegisteredHint {
                name: "alice".to_string(),
            },
        ),
        (
            "BusError",
            CliError::BusError {
                kind: famp_bus::BusErrorKind::Internal,
                message: "boom".to_string(),
            },
        ),
        (
            "BusClient",
            CliError::BusClient {
                detail: "io error".to_string(),
            },
        ),
        ("BrokerUnreachable", CliError::BrokerUnreachable),
    ]
}

fn all_variant_kinds() -> Vec<(&'static str, String)> {
    variants_a()
        .into_iter()
        .chain(variants_b())
        .chain(variants_c())
        .map(|(name, err)| (name, err.mcp_error_kind().to_string()))
        .collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

// NOTE on test name vs. actual scope: this test only verifies that every
// variant *present in the fixture lists* (variants_a / variants_b /
// variants_c) returns a non-empty mcp_error_kind() string. It does NOT
// statically verify that every CliError variant in the source is present
// in the fixture — Rust has no zero-cost reflection that would let us
// enumerate enum variants at runtime. Compile-time exhaustiveness for the
// match itself is enforced inside `mcp_error_kind()` (no `_ =>` arm), so
// adding a CliError variant without an arm is a build failure. Adding a
// variant without a fixture row, however, is silent — verified manually.
// (tey LOW-2 honesty fix; PeerCardInvalid + InvalidAgentName fixture rows
// added the same patch.)
#[test]
fn every_variant_has_mcp_kind() {
    for (variant, kind) in all_variant_kinds() {
        assert!(
            !kind.is_empty(),
            "CliError::{variant} has an empty mcp_error_kind()"
        );
    }
}

#[test]
fn mcp_kinds_are_unique() {
    let all = all_variant_kinds();
    let total = all.len();
    let kinds: HashSet<String> = all.into_iter().map(|(_, k)| k).collect();
    assert_eq!(
        kinds.len(),
        total,
        "mcp_error_kind() strings are not unique across all {total} variants",
    );
}

#[test]
fn mcp_kind_mapping_spot_checks() {
    let checks: &[(&str, CliError)] = &[
        (
            "peer_not_found",
            CliError::PeerNotFound {
                alias: "x".to_string(),
            },
        ),
        (
            "peer_duplicate",
            CliError::PeerDuplicate {
                alias: "x".to_string(),
            },
        ),
        (
            "task_not_found",
            CliError::TaskNotFound {
                task_id: "x".to_string(),
            },
        ),
        (
            "task_terminal",
            CliError::TaskTerminal {
                task_id: "x".to_string(),
            },
        ),
        (
            "await_timeout",
            CliError::AwaitTimeout {
                timeout: "30s".to_string(),
            },
        ),
        (
            "keyring_build_failed",
            CliError::KeyringBuildFailed {
                alias: "x".to_string(),
                reason: "y".to_string(),
            },
        ),
        (
            "tls_fingerprint_mismatch",
            CliError::TlsFingerprintMismatch {
                alias: "x".to_string(),
                pinned: "a".to_string(),
                got: "b".to_string(),
            },
        ),
    ];

    for (expected, err) in checks {
        let got = err.mcp_error_kind();
        assert_eq!(
            got, *expected,
            "mcp_error_kind() for {err:?}: expected {expected:?}, got {got:?}"
        );
    }
}
