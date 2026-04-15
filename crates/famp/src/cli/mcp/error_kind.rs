//! `CliError::mcp_error_kind()` — typed discriminator strings for MCP tool errors.
//!
//! The match is intentionally exhaustive with NO `_ =>` fallback. If a new
//! `CliError` variant is added without an arm here, the compiler fails the build
//! (T-04-13 mitigation). This is the compile-time gate described in the plan.

use crate::cli::error::CliError::{
    AlreadyInitialized, AwaitTimeout, CertgenFailed, Envelope, HomeCreateFailed, HomeHasNoParent,
    HomeNotAbsolute, HomeNotSet, IdentityIncomplete, Inbox, InvalidDuration, Io, KeygenFailed,
    KeyringBuildFailed, PeerDuplicate, PeerEndpointInvalid, PeerNotFound, PeerPubkeyInvalid,
    PortInUse, SendArgsInvalid, SendFailed, TaskDir, TaskNotFound, TaskTerminal,
    TlsFingerprintMismatch, Tls, TomlParse, TomlSerialize,
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
            TlsFingerprintMismatch { .. } => "tls_fingerprint_mismatch",
            SendArgsInvalid { .. } => "send_args_invalid",
            AwaitTimeout { .. } => "await_timeout",
            InvalidDuration { .. } => "invalid_duration",
            KeyringBuildFailed { .. } => "keyring_build_failed",
        }
    }
}
