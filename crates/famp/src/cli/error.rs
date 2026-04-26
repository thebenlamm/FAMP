//! Typed CLI errors. D-04 variants; D-05 structural exclusion of key material.
//!
//! Every variant carries at most a `PathBuf` label and a `#[source]`-wrapped
//! inner error. No variant embeds raw seed bytes, `FampSigningKey`, or any
//! rcgen secret. This is enforced by acceptance-criteria grep in Plan 01 and
//! by Plan 03's `init_no_leak.rs` integration test.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("FAMP_HOME is not set and $HOME is not set")]
    HomeNotSet,

    #[error("FAMP_HOME must be an absolute path, got: {}", path.display())]
    HomeNotAbsolute { path: PathBuf },

    #[error("FAMP_HOME parent directory does not exist: {}", path.display())]
    HomeHasNoParent { path: PathBuf },

    #[error("failed to create FAMP_HOME directory at {}", path.display())]
    HomeCreateFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "FAMP_HOME already initialized ({} existing files); pass --force to overwrite",
        existing_files.len()
    )]
    AlreadyInitialized { existing_files: Vec<PathBuf> },

    #[error("FAMP_HOME identity incomplete: missing {}", missing.display())]
    IdentityIncomplete { missing: PathBuf },

    #[error("keygen failed")]
    KeygenFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("TLS cert generation failed")]
    CertgenFailed(#[source] rcgen::Error),

    #[error("io error at {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("toml serialize failed")]
    TomlSerialize(#[source] toml::ser::Error),

    #[error("toml parse failed at {}", path.display())]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("another famp listen is already bound to {addr}")]
    PortInUse { addr: SocketAddr },

    #[error("inbox error")]
    Inbox(#[from] famp_inbox::InboxError),

    #[error("TLS config error")]
    Tls(#[from] famp_transport_http::TlsError),

    #[error("peer not found: {alias}")]
    PeerNotFound { alias: String },

    #[error("peer already exists: {alias}")]
    PeerDuplicate { alias: String },

    #[error("invalid peer endpoint: {value}")]
    PeerEndpointInvalid { value: String },

    #[error("invalid peer pubkey (must be 32 bytes base64url-unpadded): {value}")]
    PeerPubkeyInvalid { value: String },

    #[error("invalid peer card JSON: {reason}")]
    PeerCardInvalid { reason: String },

    #[error("invalid agent name '{name}': {reason}")]
    InvalidAgentName { name: String, reason: String },

    #[error("task record not found: {task_id}")]
    TaskNotFound { task_id: String },

    #[error("task already terminal: {task_id}")]
    TaskTerminal { task_id: String },

    #[error("send failed")]
    SendFailed(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("taskdir error")]
    TaskDir(#[from] famp_taskdir::TaskDirError),

    #[error("envelope encode/sign failed")]
    Envelope(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// FSM transition refused by `famp-fsm`. Distinct from `Envelope` —
    /// the failure is a protocol-state violation (e.g. attempting to
    /// re-commit a task already in COMMITTED), not an envelope encode/sign
    /// problem. The inner `TaskFsmError`'s Display carries the detail
    /// (`"illegal transition: cannot apply class=… terminal_status=…
    /// from state=…"`) and surfaces via `std::error::Error::source`
    /// chaining; the top-line here intentionally stays short so it does
    /// not duplicate the inner message.
    #[error("illegal task state transition")]
    FsmTransition(#[from] famp_fsm::TaskFsmError),

    /// On-disk task state string (in the `TaskRecord.state` field) does
    /// not parse to a known `TaskState`. Distinct from `Envelope` — the
    /// failure is on-disk record corruption, not anything envelope-related.
    #[error("invalid task state on disk: {value}")]
    InvalidTaskState { value: String },

    #[error("tls fingerprint mismatch for peer {alias}: pinned={pinned}, got={got}")]
    TlsFingerprintMismatch {
        alias: String,
        pinned: String,
        got: String,
    },

    /// First-contact TOFU pinning was refused because the operator did not
    /// opt in via `FAMP_TOFU_BOOTSTRAP=1`. The `got` field carries the leaf
    /// SHA-256 the server presented, so the operator can verify it
    /// out-of-band and pre-pin the fingerprint in `peers.toml`.
    #[error(
        "first-contact TOFU bootstrap refused for peer {alias}: \
         observed leaf sha256={got}. Either pre-pin the fingerprint in \
         peers.toml (tls_fingerprint_sha256) or rerun with \
         FAMP_TOFU_BOOTSTRAP=1 to accept this leaf as the trust anchor."
    )]
    TofuBootstrapRefused { alias: String, got: String },

    /// A configured principal value (in `config.toml` or `peers.toml`) is
    /// present but does not parse as a valid FAMP principal. Surfaced as a
    /// hard failure so callers do not silently sign or address traffic
    /// under a fallback identity.
    #[error("invalid principal {value:?} in {}: {reason}", path.display())]
    PrincipalInvalid {
        path: PathBuf,
        value: String,
        reason: String,
    },

    #[error("send args invalid: {reason}")]
    SendArgsInvalid { reason: String },

    #[error("await timed out after {timeout}")]
    AwaitTimeout { timeout: String },

    #[error("invalid duration: {value}")]
    InvalidDuration { value: String },

    /// Fatal error building the keyring from peers.toml at daemon startup.
    /// An invalid peer entry (bad pubkey length, bad base64, bad principal)
    /// is not recoverable — the daemon refuses to start rather than silently
    /// operating with a narrowed trust set (T-04-01 mitigated).
    #[error("keyring build failed for peer '{alias}': {reason}")]
    KeyringBuildFailed { alias: String, reason: String },
}

/// Parse a user-supplied duration string via `humantime`. Accepts the
/// common forms `"30s"`, `"5m"`, `"1h"`, `"250ms"`. Any other input
/// surfaces as [`CliError::InvalidDuration`].
pub fn parse_duration(s: &str) -> Result<std::time::Duration, CliError> {
    humantime::parse_duration(s).map_err(|_| CliError::InvalidDuration {
        value: s.to_string(),
    })
}
