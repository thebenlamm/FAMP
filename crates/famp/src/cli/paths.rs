//! Canonical filenames under `FAMP_HOME` (D-06) and an `IdentityLayout` helper
//! that joins each constant onto the home directory.

use std::path::{Path, PathBuf};

pub const KEY_ED25519: &str = "key.ed25519";
pub const PUB_ED25519: &str = "pub.ed25519";
pub const TLS_CERT_PEM: &str = "tls.cert.pem";
pub const TLS_KEY_PEM: &str = "tls.key.pem";
pub const CONFIG_TOML: &str = "config.toml";
pub const PEERS_TOML: &str = "peers.toml";

#[derive(Debug, Clone)]
pub struct IdentityLayout {
    pub home: PathBuf,
    pub key_ed25519: PathBuf,
    pub pub_ed25519: PathBuf,
    pub tls_cert_pem: PathBuf,
    pub tls_key_pem: PathBuf,
    pub config_toml: PathBuf,
    pub peers_toml: PathBuf,
}

/// Directory that holds per-task TOML records (`famp-taskdir` root).
pub fn tasks_dir(home: &Path) -> PathBuf {
    home.join("tasks")
}

/// `inbox.jsonl` sidecar cursor file.
pub fn inbox_cursor_path(home: &Path) -> PathBuf {
    home.join("inbox.cursor")
}

/// Append-only inbox jsonl file.
pub fn inbox_jsonl_path(home: &Path) -> PathBuf {
    home.join("inbox.jsonl")
}

/// `peers.toml` path under `home`.
pub fn peers_toml_path(home: &Path) -> PathBuf {
    home.join(PEERS_TOML)
}

impl IdentityLayout {
    pub fn at(home: PathBuf) -> Self {
        Self {
            key_ed25519: home.join(KEY_ED25519),
            pub_ed25519: home.join(PUB_ED25519),
            tls_cert_pem: home.join(TLS_CERT_PEM),
            tls_key_pem: home.join(TLS_KEY_PEM),
            config_toml: home.join(CONFIG_TOML),
            peers_toml: home.join(PEERS_TOML),
            home,
        }
    }

    /// All six expected entries — used by the init probe and the
    /// `IdentityIncomplete` loader in Plan 03.
    pub fn entries(&self) -> [(&'static str, &Path); 6] {
        [
            (KEY_ED25519, self.key_ed25519.as_path()),
            (PUB_ED25519, self.pub_ed25519.as_path()),
            (TLS_CERT_PEM, self.tls_cert_pem.as_path()),
            (TLS_KEY_PEM, self.tls_key_pem.as_path()),
            (CONFIG_TOML, self.config_toml.as_path()),
            (PEERS_TOML, self.peers_toml.as_path()),
        ]
    }
}
