//! Versioned public cards and idempotent trust configuration.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::process::Command;

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use famp_keyring::{KeyState, Keyring};
use serde::{Deserialize, Serialize};

use crate::cli::error::CliError;

pub const MAX_BUNDLE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub schema: u32,
    pub version: String,
    pub name: String,
    pub domain: String,
    pub address: IpAddr,
    pub port: u16,
    pub public_key: String,
    pub certificate: String,
}

impl Card {
    pub fn principal(&self) -> String {
        format!("agent:{}/{}", self.domain, self.name)
    }

    pub fn endpoint(&self) -> String {
        format!("https://{}", SocketAddr::new(self.address, self.port))
    }

    /// Version equality is checked only when a card is EXCHANGED (connect/import),
    /// never when a stored card is re-read: otherwise the first release after
    /// the one that wrote `network.json` would stop every gateway at `serve`.
    pub fn check_version(&self) -> Result<(), CliError> {
        if self.version != env!("CARGO_PKG_VERSION") {
            return Err(error(format!(
                "Network card for {} was made by FAMP {} but this is {}. Install the same FAMP build on every computer.",
                self.name,
                self.version,
                env!("CARGO_PKG_VERSION")
            )));
        }
        Ok(())
    }

    /// Short public-key fingerprint (`key_id`) to compare out of band.
    pub fn fingerprint(&self) -> Result<String, CliError> {
        Ok(famp_crypto::key_id(
            &TrustedVerifyingKey::from_b64url(&self.public_key).map_err(error)?,
        ))
    }

    /// Same computer, same trust material; only the build that wrote it may differ.
    pub fn same_ignoring_version(&self, other: &Self) -> bool {
        let mut a = self.clone();
        a.version.clone_from(&other.version);
        a == *other
    }

    /// Everything except certificate expiry. `serve` uses this for PEERS so one
    /// peer's lapsed certificate cannot stop the gateway for everyone else.
    pub fn validate_shape(&self) -> Result<(), CliError> {
        if self.schema != 1 {
            return Err(error("Unsupported network card schema"));
        }
        validate_name(&self.name)?;
        let _: Principal = self.principal().parse().map_err(error)?;
        if self.domain.contains(['/', '\n', '\r']) || self.domain.len() > 253 {
            return Err(error("Invalid network domain"));
        }
        if self.port == 0 || self.address.is_unspecified() || self.address.is_multicast() {
            return Err(error("Use a reachable unicast address and a nonzero port"));
        }
        TrustedVerifyingKey::from_b64url(&self.public_key).map_err(error)?;
        if self.certificate.len() > 16 * 1024
            || self.certificate.contains("PRIVATE KEY")
            || self
                .certificate
                .matches("-----BEGIN CERTIFICATE-----")
                .count()
                != 1
        {
            return Err(error(
                "Expected one public TLS certificate, never a private key",
            ));
        }
        // Read from the certificate's DER, never from `openssl -text`. Exactly this
        // address and never a CA: a card must not be able to mint certificates for
        // another computer's address once it is a trust anchor.
        self.check_consumers_accept_the_certificate()?;
        let facts = super::cert::facts(&self.certificate).map_err(error)?;
        if facts.ips != [self.address] || facts.other_names != 0 {
            return Err(error(format!(
                "Certificate for {} must list exactly {} as its only subjectAltName",
                self.name, self.address
            )));
        }
        if facts.is_ca != Some(false) {
            return Err(error(format!(
                "Certificate for {} must be an end-entity certificate (CA:FALSE)",
                self.name
            )));
        }
        Ok(())
    }

    /// The gateway loads every peer certificate into one trust file and exits at
    /// startup if any of them cannot be used as a pinned root, which would stop it
    /// for every peer. Run the exact functions it runs, so a card is only accepted
    /// if it cannot do that.
    fn check_consumers_accept_the_certificate(&self) -> Result<(), CliError> {
        let der = super::cert::pem_to_der(&self.certificate).map_err(error)?;
        let mut file = tempfile::NamedTempFile::new().map_err(error)?;
        file.write_all(self.certificate.as_bytes()).map_err(error)?;
        let loaded = famp_transport_http::tls::load_pem_cert(file.path()).map_err(error)?;
        if loaded.len() != 1 || loaded[0].as_ref() != der.as_slice() {
            return Err(error(format!(
                "Certificate for {} is not exactly one clean certificate",
                self.name
            )));
        }
        famp_transport_http::tls::build_pinned_client_config(file.path())
            .map(|_| ())
            .map_err(|e| {
                error(format!(
                    "Certificate for {} cannot be used as a pinned root: {e}",
                    self.name
                ))
            })
    }

    /// Shape plus: the certificate must outlive the next 24 hours.
    pub fn validate(&self) -> Result<(), CliError> {
        self.validate_shape()?;
        if self.cert_expires_within(86_400)? {
            return Err(error(format!(
                "TLS certificate for {} expires within 24 hours or is unreadable",
                self.name
            )));
        }
        Ok(())
    }

    /// `-checkend` exits nonzero when the cert expires within `secs`. `-checkip`
    /// is deliberately NOT used: LibreSSL (/usr/bin/openssl, what launchd's PATH
    /// resolves) rejects the flag, and OpenSSL 3 exits 0 on a mismatch.
    pub fn cert_expires_within(&self, secs: u64) -> Result<bool, CliError> {
        let mut cert = tempfile::NamedTempFile::new().map_err(error)?;
        cert.write_all(self.certificate.as_bytes()).map_err(error)?;
        let status = Command::new("openssl")
            .args(["x509", "-noout", "-checkend", &secs.to_string(), "-in"])
            .arg(cert.path())
            .output()
            .map_err(|e| error(format!("Could not run openssl: {e}")))?
            .status;
        Ok(!status.success())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub local: Card,
    pub peers: BTreeMap<String, Card>,
}

pub fn error(e: impl std::fmt::Display) -> CliError {
    CliError::Generic(e.to_string())
}

pub fn validate_name(name: &str) -> Result<(), CliError> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || name.starts_with('-')
    {
        return Err(error("Machine name must be 1–48 lowercase letters, digits or hyphens, starting with a letter or digit. Use --name."));
    }
    Ok(())
}

pub fn checked(cmd: &mut Command) -> Result<Vec<u8>, CliError> {
    let output = cmd.output().map_err(|e| {
        error(format!(
            "Could not run {}: {e}",
            cmd.get_program().display()
        ))
    })?;
    if !output.status.success() {
        return Err(error(format!(
            "{} failed ({}): {}",
            cmd.get_program().display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

pub fn load(home: &Path) -> Result<Config, CliError> {
    let bytes = std::fs::read(home.join("network.json")).map_err(|e| {
        error(format!(
            "Network is not configured or unreadable: {e}. Run `famp network setup`."
        ))
    })?;
    serde_json::from_slice(&bytes).map_err(error)
}

/// Stamp this computer's stored card with the running build's version.
///
/// Trust material is untouched; this only keeps exchange working after an upgrade.
pub fn refresh_version(home: &Path) -> Result<Config, CliError> {
    let mut config = load(home)?;
    if config.local.version != env!("CARGO_PKG_VERSION") {
        env!("CARGO_PKG_VERSION").clone_into(&mut config.local.version);
        save(home, &config)?;
    }
    Ok(config)
}

pub fn atomic(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let parent = path
        .parent()
        .ok_or_else(|| error("Missing parent directory"))?;
    std::fs::create_dir_all(parent).map_err(error)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    temp.write_all(bytes).map_err(error)?;
    temp.as_file().sync_all().map_err(error)?;
    temp.persist(path).map_err(error)?;
    Ok(())
}

pub fn save(home: &Path, config: &Config) -> Result<(), CliError> {
    atomic(
        &home.join("network.json"),
        &serde_json::to_vec_pretty(config).map_err(error)?,
    )
}

pub fn parse_cards(bytes: &[u8]) -> Result<Vec<Card>, CliError> {
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err(error("Network bundle exceeds 1 MiB"));
    }
    let cards: Vec<Card> = serde_json::from_slice(bytes).map_err(error)?;
    if cards.is_empty() || cards.len() > 64 {
        return Err(error("A network bundle must contain 1–64 computers"));
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ImportOpts {
    /// Validate and print the diff; write nothing.
    pub check: bool,
    /// Make the peer set EXACTLY the cards given: drop every other peer and every
    /// other keyring pin (the old keyring is kept as `peers.keyring.bak`).
    pub prune: bool,
}

/// Principals named in a keyring file (first token of each non-comment line).
pub fn keyring_principals(path: &Path) -> Result<Vec<String>, CliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path).map_err(error)?;
    Ok(text
        .lines()
        .filter(|l| !l.trim_start().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
        .collect())
}

/// Validate the whole proposed configuration before writing any trust.
///
/// Prints what would change. Without `prune`, existing pins are kept (and
/// unmanaged ones reported); a changed card for a known peer fails closed.
pub fn import(home: &Path, cards: &[Card], opts: ImportOpts) -> Result<(), CliError> {
    let mut config = load(home)?;
    config.local.validate()?;
    let path = home.join("gateway/peers.keyring");
    let existing = keyring_principals(&path)?;
    let old_peers = config.peers.clone();
    let old_ring = if path.exists() {
        Some(Keyring::load_from_file(&path).map_err(|e| {
            error(format!(
                "{} is unreadable ({e}); move it aside to start over",
                path.display()
            ))
        })?)
    } else {
        None
    };
    // --prune starts from an empty keyring, which forgets revocations; the
    // loop below therefore re-checks every key against the OLD keyring.
    let mut keyring = if opts.prune {
        Keyring::new()
    } else {
        old_ring.clone().unwrap_or_default()
    };
    if opts.prune {
        config.peers.clear();
    }
    let mut seen = BTreeSet::new();
    for card in cards {
        card.validate()?;
        if !seen.insert(&card.name) {
            return Err(error("Duplicate machine names in network bundle"));
        }
        if card.name == config.local.name {
            if !card.same_ignoring_version(&config.local) {
                return Err(error("A peer conflicts with this computer's identity"));
            }
            continue;
        }
        card.check_version()?;
        if card.public_key == config.local.public_key {
            return Err(error(format!(
                "Peer {} presents this computer's own signing key",
                card.name
            )));
        }
        if card.domain == config.local.domain {
            return Err(error("Two computers cannot share a federation domain"));
        }
        // Checked against the peers as they were BEFORE `--prune` emptied the map:
        // prune removes pins that are not in this run's set, it must never let a
        // listed host replace the pin of a peer that is already trusted. To rotate a
        // peer, prune it out in one run and add it back in the next.
        if let Some(old) = old_peers.get(&card.name) {
            if !old.same_ignoring_version(card) {
                return Err(error(format!(
                    "Peer {} changed its card. Refusing to replace existing trust automatically.",
                    card.name
                )));
            }
        }
        let principal: Principal = card.principal().parse().map_err(error)?;
        let key = TrustedVerifyingKey::from_b64url(&card.public_key).map_err(error)?;
        if old_ring.as_ref().is_some_and(|old| {
            old.entries(&principal)
                .iter()
                .any(|e| e.state() == KeyState::Revoked && e.key().as_bytes() == key.as_bytes())
        }) {
            return Err(error(format!(
                "{} presents a key that was REVOKED in this computer's keyring. Refusing to pin it, with or without --prune.",
                card.principal()
            )));
        }
        keyring.pin_tofu(principal, key).map_err(error)?;
        config.peers.insert(card.name.clone(), card.clone());
    }
    let mut domains = BTreeSet::new();
    for peer in config.peers.values() {
        if !domains.insert(&peer.domain) {
            return Err(error("Two peers cannot share a federation domain"));
        }
    }
    check_unique_endpoints(&config)?;
    report_diff(&old_peers, &config, &existing, opts.prune)?;
    // pin_tofu and file loading have different validation: round-trip before
    // modifying live files, including the duplicate-public-key restriction.
    let temp = tempfile::NamedTempFile::new().map_err(error)?;
    keyring.save_to_file(temp.path()).map_err(error)?;
    if opts.prune {
        carry_revocations(&path, temp.path())?;
    }
    Keyring::load_from_file(temp.path()).map_err(error)?;
    if !opts.check {
        if opts.prune {
            backup(&path)?;
            backup(&home.join("network.json"))?;
        }
        atomic(&path, &std::fs::read(temp.path()).map_err(error)?)?;
        save(home, &config)?;
    }
    Ok(())
}

/// Two computers may not claim one endpoint.
///
/// A card for peer B's address and port would let its holder terminate TLS meant for
/// B. IPv4-mapped IPv6 is unmapped first, so `::ffff:a.b.c.d` and `a.b.c.d` are the
/// same endpoint. Extracted from `import` solely to satisfy `clippy::too_many_lines`.
fn check_unique_endpoints(config: &Config) -> Result<(), CliError> {
    let canonical = |ip: IpAddr| match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 @ IpAddr::V4(_) => v4,
    };
    let mut endpoints = BTreeSet::new();
    endpoints.insert((canonical(config.local.address), config.local.port));
    for peer in config.peers.values() {
        if !endpoints.insert((canonical(peer.address), peer.port)) {
            return Err(error(format!(
                "{} claims an address and port that another computer already uses",
                peer.name
            )));
        }
    }
    Ok(())
}

/// Append the old keyring's `revoked` lines to `dest`, verbatim.
///
/// A fresh keyring has no revocation records. Carrying them over keeps a later run
/// (or a later card) from re-pinning a key that was revoked here. The caller's
/// load of the combined file validates it.
fn carry_revocations(old: &Path, dest: &Path) -> Result<(), CliError> {
    if !old.exists() {
        return Ok(());
    }
    let mut kept = String::new();
    for line in std::fs::read_to_string(old).map_err(error)?.lines() {
        if line.split_whitespace().nth(2) == Some("revoked") {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    if kept.is_empty() {
        return Ok(());
    }
    std::fs::OpenOptions::new()
        .append(true)
        .open(dest)
        .and_then(|mut f| f.write_all(kept.as_bytes()))
        .map_err(error)
}

/// Copy `path` to the first unused `<path>.bak[.N]`; never overwrites a backup,
/// so rerunning a failed `--prune` cannot destroy the original.
fn backup(path: &Path) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }
    let name = path
        .file_name()
        .ok_or_else(|| error("Missing file name"))?
        .to_string_lossy()
        .into_owned();
    for n in 0.. {
        let suffix = if n == 0 {
            ".bak".to_owned()
        } else {
            format!(".bak.{n}")
        };
        let target = path.with_file_name(format!("{name}{suffix}"));
        if !target.exists() {
            return atomic(&target, &std::fs::read(path).map_err(error)?);
        }
    }
    Ok(())
}

fn report_diff(
    old: &BTreeMap<String, Card>,
    config: &Config,
    existing_pins: &[String],
    prune: bool,
) -> Result<(), CliError> {
    let host = &config.local.name;
    for (name, card) in &config.peers {
        let state = match old.get(name) {
            None => "ADD",
            Some(o) if o.same_ignoring_version(card) => "keep",
            Some(o) if o.public_key != card.public_key => "KEY CHANGED",
            Some(_) => "CHANGE",
        };
        eprintln!(
            "[{host}] peer {state} {} key_id={}",
            card.principal(),
            card.fingerprint()?
        );
    }
    for name in old.keys().filter(|n| !config.peers.contains_key(*n)) {
        eprintln!("[{host}] peer REMOVE {name}");
    }
    let wanted: BTreeSet<String> = config.peers.values().map(Card::principal).collect();
    for pin in existing_pins.iter().filter(|p| !wanted.contains(*p)) {
        if prune {
            eprintln!("[{host}] pin REMOVE {pin} (old file kept as gateway/peers.keyring.bak[.N])");
        } else {
            eprintln!(
                "[{host}] pin UNMANAGED {pin} is still trusted; rerun with --prune to drop it"
            );
        }
    }
    Ok(())
}
