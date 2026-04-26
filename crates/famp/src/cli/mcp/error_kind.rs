//! `CliError::mcp_error_kind()` — typed discriminator strings for MCP tool errors.
//!
//! The match is intentionally exhaustive with NO `_ =>` fallback. If a new
//! `CliError` variant is added without an arm here, the compiler fails the build
//! (T-04-13 mitigation). This is the compile-time gate described in the plan.

use famp_fsm::TaskFsmError;

use crate::cli::error::CliError::{
    AlreadyInitialized, AwaitTimeout, CertgenFailed, Envelope, FsmTransition, HomeCreateFailed,
    HomeHasNoParent, HomeNotAbsolute, HomeNotSet, IdentityIncomplete, Inbox, InvalidAgentName,
    InvalidDuration, InvalidIdentityName, InvalidTaskState, Io, KeygenFailed, KeyringBuildFailed,
    NotRegistered, PeerCardInvalid, PeerDuplicate, PeerEndpointInvalid, PeerNotFound,
    PeerPubkeyInvalid, PortInUse, PrincipalInvalid, SendArgsInvalid, SendFailed, TaskDir,
    TaskNotFound, TaskTerminal, Tls, TlsFingerprintMismatch, TofuBootstrapRefused, TomlParse,
    TomlSerialize, UnknownIdentity,
};

impl crate::cli::error::CliError {
    /// Return a `snake_case` discriminator string that names this error in the
    /// MCP JSON-RPC error `data.famp_error_kind` field.
    ///
    /// Rules:
    /// - No `_ =>` arm — must remain exhaustive.
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
        }
    }
}
