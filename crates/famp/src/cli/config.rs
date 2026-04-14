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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen_addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8443),
        }
    }
}

/// Phase 1 placeholder. Fields land in Phase 3 (`famp peer add`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerEntry {}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peers {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
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
}
