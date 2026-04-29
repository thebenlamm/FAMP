//! Typed discriminator strings for MCP tool errors.
//!
//! Two parallel exhaustive-match tables live in this module:
//!
//! 1. [`bus_error_to_jsonrpc`] — Phase 2 (MCP-10): maps every
//!    `famp_bus::BusErrorKind` variant to a unique JSON-RPC error code
//!    in `-32100..=-32109` plus a `snake_case` kind string. This is
//!    what `cli::mcp::server::bus_error_response` invokes when a tool
//!    body returns `BusErrorKind` from a broker round-trip.
//!
//! 2. `CliError::mcp_error_kind()` — pre-Phase-2 carry-forward (still
//!    used by `tests/{clierror_fsm_transition_display, send_principal_fallback,
//!    send_tofu_bootstrap_refused, mcp_error_kind_exhaustive}.rs`).
//!    Plan 02-09 retires it once every tool body has been rewired
//!    onto the broker; until then it stays under the same
//!    "no wildcard arm" exhaustiveness rule.
//!
//! Both matches are intentionally exhaustive with NO wildcard arm. If a
//! new variant is added on either side without an arm here, the
//! compiler fails the build (T-04-13 mitigation, MCP-10 gate).

use famp_bus::BusErrorKind;
use famp_fsm::TaskFsmError;

use crate::cli::error::CliError::{
    AlreadyInitialized, AwaitTimeout, CertgenFailed, Envelope, FsmTransition, HomeCreateFailed,
    HomeHasNoParent, HomeNotAbsolute, HomeNotSet, IdentityIncomplete, Inbox, InvalidAgentName,
    InvalidDuration, InvalidIdentityName, InvalidTaskState, Io, KeygenFailed, KeyringBuildFailed,
    NoIdentityBound, NotRegistered, PeerCardInvalid, PeerDuplicate, PeerEndpointInvalid,
    PeerNotFound, PeerPubkeyInvalid, PortInUse, PrincipalInvalid, SendArgsInvalid, SendFailed,
    TaskDir, TaskNotFound, TaskTerminal, Tls, TlsFingerprintMismatch, TofuBootstrapRefused,
    TomlParse, TomlSerialize, UnknownIdentity,
};

impl crate::cli::error::CliError {
    /// Return a `snake_case` discriminator string that names this error in the
    /// MCP JSON-RPC error `data.famp_error_kind` field.
    ///
    /// Rules:
    /// - No wildcard arm — must remain exhaustive.
    /// - Every string must be unique (enforced by `mcp_error_kind_exhaustive` test).
    /// - Strings are stable API — once shipped they cannot change without a
    ///   major version bump.
    #[must_use]
    pub const fn mcp_error_kind(&self) -> &'static str {
        match self {
            HomeNotSet => "home_not_set",
            HomeNotAbsolute { .. } => "home_not_absolute",
            HomeHasNoParent { .. } => "home_has_no_parent",
            HomeCreateFailed { .. } => "home_create_failed",
            AlreadyInitialized { .. } => "already_initialized",
            IdentityIncomplete { .. } => "identity_incomplete",
            KeygenFailed(_) => "keygen_failed",
            CertgenFailed(_) => "certgen_failed",
            Io { .. } => "io_error",
            TomlSerialize(_) => "toml_serialize",
            TomlParse { .. } => "toml_parse",
            PortInUse { .. } => "port_in_use",
            Inbox(_) => "inbox_error",
            Tls(_) => "tls_error",
            PeerNotFound { .. } => "peer_not_found",
            PeerDuplicate { .. } => "peer_duplicate",
            PeerEndpointInvalid { .. } => "peer_endpoint_invalid",
            PeerPubkeyInvalid { .. } => "peer_pubkey_invalid",
            TaskNotFound { .. } => "task_not_found",
            TaskTerminal { .. } => "task_terminal",
            SendFailed(_) => "send_failed",
            TaskDir(_) => "taskdir_error",
            Envelope(_) => "envelope_error",
            // Nested exhaustive match: the kind string is committed to a
            // SPECIFIC `TaskFsmError` variant, not the FSM-error category as
            // a whole. Without this, adding a new `TaskFsmError` variant
            // would silently get misclassified as `"fsm_transition_illegal"`.
            // Compiler now forces a deliberate kind-string decision per FSM
            // variant. (tey LOW-1.)
            FsmTransition(inner) => match inner {
                TaskFsmError::IllegalTransition { .. } => "fsm_transition_illegal",
            },
            InvalidTaskState { .. } => "invalid_task_state",
            TlsFingerprintMismatch { .. } => "tls_fingerprint_mismatch",
            TofuBootstrapRefused { .. } => "tofu_bootstrap_refused",
            PrincipalInvalid { .. } => "principal_invalid",
            SendArgsInvalid { .. } => "send_args_invalid",
            AwaitTimeout { .. } => "await_timeout",
            InvalidDuration { .. } => "invalid_duration",
            KeyringBuildFailed { .. } => "keyring_build_failed",
            PeerCardInvalid { .. } => "peer_card_invalid",
            InvalidAgentName { .. } => "invalid_agent_name",
            NotRegistered => "not_registered",
            UnknownIdentity { .. } => "unknown_identity",
            InvalidIdentityName { .. } => "invalid_identity_name",
            NoIdentityBound { .. } => "no_identity_bound",
        }
    }
}

// ── Phase 2 (MCP-10): BusErrorKind → JSON-RPC code + kind string ─────────────

/// Map `famp_bus::BusErrorKind` to its JSON-RPC error `(code, kind_str)`.
///
/// Codes live in the application range `-32100..=-32109` (RESEARCH §2 Item 6).
/// Each variant gets a unique code; each kind string is unique and non-empty.
///
/// **Compile-time exhaustiveness gate (MCP-10):** the match below has no
/// wildcard arm. Adding a `BusErrorKind` variant in `famp_bus` will fail
/// to compile here until the new variant is given a code + kind string,
/// preventing silent misclassification at the wire boundary.
///
/// The companion test `tests/mcp_error_kind_exhaustive.rs` iterates
/// `BusErrorKind::ALL` to assert every variant produces a unique code in
/// the documented range plus a non-empty `kind_str`.
#[must_use]
pub const fn bus_error_to_jsonrpc(kind: BusErrorKind) -> (i64, &'static str) {
    match kind {
        BusErrorKind::NotRegistered => (-32100, "not_registered"),
        BusErrorKind::NameTaken => (-32101, "name_taken"),
        BusErrorKind::ChannelNameInvalid => (-32102, "channel_name_invalid"),
        BusErrorKind::NotJoined => (-32103, "not_joined"),
        BusErrorKind::EnvelopeInvalid => (-32104, "envelope_invalid"),
        BusErrorKind::EnvelopeTooLarge => (-32105, "envelope_too_large"),
        BusErrorKind::TaskNotFound => (-32106, "task_not_found"),
        BusErrorKind::BrokerProtoMismatch => (-32107, "broker_proto_mismatch"),
        BusErrorKind::BrokerUnreachable => (-32108, "broker_unreachable"),
        BusErrorKind::Internal => (-32109, "internal"),
    }
}
