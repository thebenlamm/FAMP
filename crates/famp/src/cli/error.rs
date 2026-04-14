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

    #[error("tls fingerprint mismatch for peer {alias}: pinned={pinned}, got={got}")]
    TlsFingerprintMismatch {
        alias: String,
        pinned: String,
        got: String,
    },

    #[error("send args invalid: {reason}")]
    SendArgsInvalid { reason: String },
}
