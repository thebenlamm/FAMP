//! Phase 1 config and peers file schemas.
//!
//! D-12: `config.toml` has exactly one field (`listen_addr`).
//!       Phase 1 narrowing of `REQUIREMENTS.md` IDENT-03 — the `principal`
//!       and `inbox_path` fields specified in the original IDENT-03
//!       acceptance criterion are intentionally deferred (see `CONTEXT.md`
//!       D-12 and `ROADMAP` v0.8 Phase 1). They land in the phase that
//!       first reads them (Phase 2 for principal, Phase 3 for inbox).
//!
//! D-13/D-14: `deny_unknown_fields` on both; empty `peers.toml` → empty `Peers`.
//!       Phase 1 narrowing of `REQUIREMENTS.md` IDENT-04 — `PeerEntry` has no
//!       fields in Phase 1 (see `CONTEXT.md` D-14). Fields land in Phase 3
//!       via `famp peer add` (`name`, `url`, `pubkey_b64`, `trust_cert_path`).

use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;

use crate::cli::error::CliError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen_addr: SocketAddr,
    /// Optional override for the daemon's self-principal.
    ///
    /// When absent, `run_on_listener` uses `agent:localhost/self` (the
    /// Phase 2/3 default). Set this in `config.toml` to distinguish two
    /// daemons on the same machine — e.g., `agent:localhost/alice` vs
    /// `agent:localhost/bob`. Phase 4 Plan 04-03 uses this to build the
    /// two-daemon E2E harness with distinct identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8443),
            principal: None,
        }
    }
}

/// A registered peer agent — target of `famp send`, identified via `alias`.
///
/// Phase 3 promotes this from the Phase 1 zero-field placeholder to the
/// real schema used by `famp peer add` / `famp send` (plan 03-01).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PeerEntry {
    /// Local alias (`famp send --to <alias>`).
    pub alias: String,
    /// `https://host:port` — schema validation lives in `famp peer add`.
    pub endpoint: String,
    /// base64url-unpadded ed25519 verifying key (32 raw bytes when decoded).
    pub pubkey_b64: String,
    /// Optional FAMP principal (`agent:authority/name`) used as the envelope
    /// `to` field and inbox URL segment. `None` → caller derives a default
    /// (Phase 3 `famp send` uses `agent:localhost/self` to interoperate with
    /// the Phase 2 listen daemon's self-keyring).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// TOFU-pinned TLS cert fingerprint (sha256 hex). `None` until first
    /// successful contact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_fingerprint_sha256: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peers {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
}

impl Peers {
    pub fn find(&self, alias: &str) -> Option<&PeerEntry> {
        self.peers.iter().find(|p| p.alias == alias)
    }

    pub fn find_mut(&mut self, alias: &str) -> Option<&mut PeerEntry> {
        self.peers.iter_mut().find(|p| p.alias == alias)
    }

    /// Append `entry` if its alias is not yet present. Returns the
    /// rejected entry back to the caller on duplicate.
    pub fn try_add(&mut self, entry: PeerEntry) -> Result<(), PeerEntry> {
        if self.find(&entry.alias).is_some() {
            return Err(entry);
        }
        self.peers.push(entry);
        Ok(())
    }
}

/// Read `peers.toml` from disk. Empty file → empty `Peers`.
pub fn read_peers(path: &Path) -> Result<Peers, CliError> {
    let bytes = std::fs::read(path).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.is_empty() {
        return Ok(Peers::default());
    }
    let text = std::str::from_utf8(&bytes).map_err(|err| CliError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, err),
    })?;
    toml::from_str(text).map_err(|source| CliError::TomlParse {
        path: path.to_path_buf(),
        source,
    })
}

/// Atomically write `peers.toml` via same-directory `NamedTempFile` +
/// `sync_all` + `persist`. Shared helper used by `famp peer add` and the
/// TOFU fingerprint capture path in `famp send`.
pub fn write_peers_atomic(path: &Path, peers: &Peers) -> Result<(), CliError> {
    let serialized = toml::to_string(peers).map_err(CliError::TomlSerialize)?;
    let parent = path.parent().ok_or_else(|| CliError::HomeHasNoParent {
        path: path.to_path_buf(),
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|source| CliError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    tmp.write_all(serialized.as_bytes())
        .map_err(|source| CliError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    tmp.as_file_mut()
        .sync_all()
        .map_err(|source| CliError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    tmp.persist(path).map_err(|err| CliError::Io {
        path: path.to_path_buf(),
        source: err.error,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn config_default_serializes_one_field() {
        let s = toml::to_string(&Config::default()).unwrap();
        assert_eq!(s, "listen_addr = \"127.0.0.1:8443\"\n");
    }

    #[test]
    fn config_roundtrip() {
        let s = toml::to_string(&Config::default()).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(
            parsed.listen_addr,
            "127.0.0.1:8443".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn config_rejects_unknown_fields() {
        let res = toml::from_str::<Config>(
            "listen_addr = \"127.0.0.1:8443\"\nlog_level = \"debug\"\n",
        );
        assert!(res.is_err(), "deny_unknown_fields should reject log_level");
    }

    #[test]
    fn peers_empty_file_loads_empty() {
        let p: Peers = toml::from_str("").unwrap();
        assert!(p.peers.is_empty());
    }

    #[test]
    fn peers_rejects_unknown_fields() {
        let res = toml::from_str::<Peers>("garbage = 1\n");
        assert!(res.is_err(), "deny_unknown_fields should reject garbage");
    }

    #[test]
    fn peers_roundtrip_single_entry() {
        let mut peers = Peers::default();
        peers
            .try_add(PeerEntry {
                alias: "alice".to_string(),
                endpoint: "https://127.0.0.1:9443".to_string(),
                pubkey_b64: "abc".to_string(),
                principal: None,
                tls_fingerprint_sha256: None,
            })
            .unwrap();
        let s = toml::to_string(&peers).unwrap();
        let back: Peers = toml::from_str(&s).unwrap();
        assert_eq!(back.peers.len(), 1);
        assert_eq!(back.peers[0].alias, "alice");
        assert_eq!(back.peers[0].endpoint, "https://127.0.0.1:9443");
        assert_eq!(back.peers[0].pubkey_b64, "abc");
        assert!(back.peers[0].tls_fingerprint_sha256.is_none());
    }

    #[test]
    fn peers_try_add_rejects_duplicate_alias() {
        let mut peers = Peers::default();
        let entry = PeerEntry {
            alias: "alice".to_string(),
            endpoint: "https://127.0.0.1:9443".to_string(),
            pubkey_b64: "abc".to_string(),
            principal: None,
            tls_fingerprint_sha256: None,
        };
        peers.try_add(entry.clone()).unwrap();
        let err = peers.try_add(entry).unwrap_err();
        assert_eq!(err.alias, "alice");
    }

    #[test]
    fn peers_rejects_unknown_fields_on_entry() {
        let src = "[[peers]]\nalias = \"x\"\nendpoint = \"https://x\"\npubkey_b64 = \"y\"\nbogus = 1\n";
        let res = toml::from_str::<Peers>(src);
        assert!(res.is_err(), "deny_unknown_fields should reject bogus field on entry");
    }

    #[test]
    fn peers_find_returns_none_for_unknown_alias() {
        let peers = Peers::default();
        assert!(peers.find("nope").is_none());
    }
}
