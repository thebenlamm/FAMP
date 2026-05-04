//! MCP-10 exhaustive coverage: every `famp_bus::BusErrorKind` variant
//! has a unique JSON-RPC error code in `-32100..=-32109` plus a
//! non-empty, unique kind string.
//!
//! This is the CI gate that complements the compile-time exhaustive
//! match in `cli::mcp::error_kind::bus_error_to_jsonrpc` (no `_ =>`
//! arm). The compile gate prevents *missing* variants; this runtime
//! gate prevents *colliding* codes/strings — two variants accidentally
//! sharing a discriminator would silently misroute tool errors at the
//! wire boundary.
//!
//! Plan 02-08 (MCP-10) introduced `bus_error_to_jsonrpc`; plan 02-01's
//! `every_variant_has_mcp_kind` / `mcp_kinds_are_unique` /
//! `mcp_kind_mapping_spot_checks` over `CliError::mcp_error_kind` stay
//! green and live alongside this `BusErrorKind` suite — both surfaces
//! coexist until plan 02-09 retires the `CliError` side.

// Integration test binaries inherit all of famp's transitive deps; silence
// "unused crate" warnings for crates we don't explicitly reference here.
#![allow(unused_crate_dependencies)]
// Test helpers legitimately need unwrap/expect for constructing error payloads.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use famp::cli::error::CliError;
use famp::cli::mcp::error_kind::bus_error_to_jsonrpc;
use famp_bus::BusErrorKind;

// ── Phase 2 (MCP-10): BusErrorKind exhaustive coverage ───────────────────────

/// Iterate `BusErrorKind::ALL` (the 10-element pinned constant) and
/// assert each variant maps to a unique JSON-RPC code in the documented
/// `-32100..=-32109` range plus a unique non-empty kind string.
#[test]
fn every_bus_error_kind_has_unique_jsonrpc_code() {
    let mut codes = HashSet::new();
    let mut kinds = HashSet::new();
    for kind in BusErrorKind::ALL {
        let (code, kind_str) = bus_error_to_jsonrpc(kind);
        assert!(
            (-32109..=-32100).contains(&code),
            "code {code} for {kind:?} must be in -32100..=-32109 (RESEARCH §2 Item 6)"
        );
        assert!(
            !kind_str.is_empty(),
            "kind_str for {kind:?} must be non-empty"
        );
        assert!(
            codes.insert(code),
            "duplicate JSON-RPC code {code} for kind {kind:?}"
        );
        assert!(
            kinds.insert(kind_str),
            "duplicate kind_str {kind_str:?} for kind {kind:?}"
        );
    }
    assert_eq!(
        codes.len(),
        10,
        "must cover all 10 BusErrorKind variants (saw {})",
        codes.len()
    );
    assert_eq!(
        kinds.len(),
        10,
        "must produce 10 unique kind strings (saw {})",
        kinds.len()
    );
}

#[test]
fn bus_error_kind_spot_checks() {
    // Pin the exact (code, kind_str) pairs that the wire boundary
    // commits to. A change here without a SUMMARY-documented major
    // version bump is a regression — these strings are stable API.
    let cases: &[(BusErrorKind, i64, &str)] = &[
        (BusErrorKind::NotRegistered, -32100, "not_registered"),
        (BusErrorKind::NameTaken, -32101, "name_taken"),
        (
            BusErrorKind::ChannelNameInvalid,
            -32102,
            "channel_name_invalid",
        ),
        (BusErrorKind::NotJoined, -32103, "not_joined"),
        (BusErrorKind::EnvelopeInvalid, -32104, "envelope_invalid"),
        (BusErrorKind::EnvelopeTooLarge, -32105, "envelope_too_large"),
        (BusErrorKind::TaskNotFound, -32106, "task_not_found"),
        (
            BusErrorKind::BrokerProtoMismatch,
            -32107,
            "broker_proto_mismatch",
        ),
        (
            BusErrorKind::BrokerUnreachable,
            -32108,
            "broker_unreachable",
        ),
        (BusErrorKind::Internal, -32109, "internal"),
    ];
    for (kind, expected_code, expected_str) in cases {
        let (got_code, got_str) = bus_error_to_jsonrpc(*kind);
        assert_eq!(got_code, *expected_code, "code mismatch for {kind:?}");
        assert_eq!(got_str, *expected_str, "kind_str mismatch for {kind:?}");
    }
}

// ── Phase 1 carry-forward: CliError::mcp_error_kind exhaustive coverage ──────
//
// The CliError side of error_kind.rs stays in place for plans 02-03..02-07
// (one-shot CLI subcommands) and the three pre-existing tests that call
// `err.mcp_error_kind()`. Plan 02-09 retires it once every tool body has
// migrated to BusErrorKind.

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
            "NameTaken",
            CliError::NameTaken {
                name: "alice".to_string(),
            },
        ),
        ("BrokerUnreachable", CliError::BrokerUnreachable),
        ("Disconnected", CliError::Disconnected),
        (
            "BusError",
            CliError::BusError {
                kind: BusErrorKind::Internal,
                message: "synthetic test error".to_string(),
            },
        ),
        (
            "NotRegisteredHint",
            CliError::NotRegisteredHint {
                name: "alice".to_string(),
            },
        ),
        (
            "BusClient",
            CliError::BusClient {
                detail: "io error".to_string(),
            },
        ),
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
