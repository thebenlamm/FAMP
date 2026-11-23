//! `connect` orchestration against fake remotes that run the REAL setup /
//! export / import logic on isolated temp homes. No ssh, launchd or systemd.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unused_self
)]

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use famp_crypto::TrustedVerifyingKey;
use famp_keyring::Keyring;

use super::connect::{orchestrate, Remote};
use super::state::{self, Card, ImportOpts};
use super::{setup, ConnectArgs, Host, SetupArgs};
use crate::cli::error::CliError;
use crate::cli::peer::identity;

struct Fake {
    homes: BTreeMap<String, PathBuf>,
    log: RefCell<Vec<(String, String)>>,
    /// Fail once when (host, action) matches exactly.
    fail: RefCell<Option<(String, String)>>,
    expect_no_services: Cell<bool>,
}

fn prepare_home(home: &Path, name: &str, port: u16) {
    setup::prepare(
        home,
        &SetupArgs {
            name: Some(name.to_owned()),
            address: Some("127.0.0.1".parse().unwrap()),
            allow_non_tailnet: true,
            port: Some(port),
            host: Host::None,
            no_services: true,
        },
    )
    .unwrap();
}

impl Remote for Fake {
    fn run(&self, host: &str, action: &str, input: Option<&[u8]>) -> Result<Vec<u8>, CliError> {
        self.log
            .borrow_mut()
            .push((host.to_owned(), action.to_owned()));
        let hit = self
            .fail
            .borrow()
            .as_ref()
            .is_some_and(|(h, a)| h == host && a == action);
        if hit {
            self.fail.borrow_mut().take();
            return Err(state::error(format!(
                "{host}: injected failure at {action}"
            )));
        }
        let home = &self.homes[host];
        match action {
            "network --help" | "network stage" | "network enable" | "doctor" => Ok(Vec::new()),
            a if a.starts_with("setup") => {
                assert_eq!(
                    a.contains("--no-services"),
                    self.expect_no_services.get(),
                    "setup flags"
                );
                let index = self.homes.keys().position(|k| k == host).unwrap();
                prepare_home(home, host, 7100 + u16::try_from(index).unwrap());
                Ok(Vec::new())
            }
            "network export" => Ok(serde_json::to_vec(&state::load(home)?.local).unwrap()),
            a if a.starts_with("network import") => {
                let cards = state::parse_cards(input.unwrap())?;
                state::import(
                    home,
                    &cards,
                    ImportOpts {
                        check: a.contains("--check"),
                        prune: a.contains("--prune"),
                    },
                )?;
                Ok(Vec::new())
            }
            other => panic!("unexpected remote action {other}"),
        }
    }
}

struct Net {
    _tmp: tempfile::TempDir,
    mac: PathBuf,
    fake: Fake,
}

impl Net {
    fn new(hosts: &[&str]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let mac = tmp.path().join("mac");
        std::fs::create_dir_all(&mac).unwrap();
        prepare_home(&mac, "mac", 7001);
        let mut homes = BTreeMap::new();
        for h in hosts {
            let dir = tmp.path().join(h);
            std::fs::create_dir_all(&dir).unwrap();
            homes.insert((*h).to_owned(), dir);
        }
        Self {
            _tmp: tmp,
            mac,
            fake: Fake {
                homes,
                log: RefCell::default(),
                fail: RefCell::default(),
                expect_no_services: Cell::new(false),
            },
        }
    }

    fn args(&self, stage: bool, pairwise: bool, prune: bool) -> ConnectArgs {
        self.fake.expect_no_services.set(stage);
        ConnectArgs {
            hosts: self.fake.homes.keys().cloned().collect(),
            stage,
            pairwise,
            prune,
            dry_run: false,
            expect: Vec::new(),
        }
    }

    fn connect(&self, args: &ConnectArgs) -> Result<(), CliError> {
        orchestrate(&self.mac, args, &self.fake, false)
    }

    fn peers(&self, home: &Path) -> BTreeSet<String> {
        state::load(home).unwrap().peers.keys().cloned().collect()
    }

    fn pins(&self, home: &Path) -> BTreeSet<String> {
        state::keyring_principals(&home.join("gateway/peers.keyring"))
            .unwrap()
            .into_iter()
            .collect()
    }

    fn actions(&self) -> Vec<String> {
        self.fake
            .log
            .borrow()
            .iter()
            .map(|(h, a)| format!("{h}: {a}"))
            .collect()
    }
}

fn sha(home: &Path) -> Vec<u8> {
    std::fs::read(identity::gateway_identity_path(home)).unwrap()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn stage_mesh_pins_everyone_keeps_identities_and_starts_nothing() {
    let net = Net::new(&["alpha", "beta"]);
    let before_mac = sha(&net.mac);
    let args = net.args(true, false, false);
    net.connect(&args).unwrap();

    assert_eq!(net.peers(&net.mac), set(&["alpha", "beta"]));
    assert_eq!(net.peers(&net.fake.homes["alpha"]), set(&["mac", "beta"]));
    assert_eq!(net.peers(&net.fake.homes["beta"]), set(&["mac", "alpha"]));
    // Exact pin sets: 2 peers each, nothing else.
    assert_eq!(net.pins(&net.mac).len(), 2);
    assert_eq!(net.pins(&net.fake.homes["alpha"]).len(), 2);
    // Staged: no service was enabled and no doctor probe ran.
    let log = net.actions();
    assert!(
        !log.iter().any(|a| a.ends_with("network enable")),
        "{log:?}"
    );
    assert!(!log.iter().any(|a| a.ends_with("doctor")), "{log:?}");
    assert_eq!(
        log.iter().filter(|a| a.ends_with("network stage")).count(),
        2
    );
    // Private identity untouched by connect.
    assert_eq!(sha(&net.mac), before_mac);
}

#[test]
fn enable_mode_enables_then_checks_each_host() {
    let net = Net::new(&["alpha"]);
    net.connect(&net.args(false, false, false)).unwrap();
    let log = net.actions();
    let enable = log
        .iter()
        .position(|a| a.ends_with("network enable"))
        .unwrap();
    let doctor = log.iter().position(|a| a.ends_with("doctor")).unwrap();
    assert!(enable < doctor, "{log:?}");
}

#[test]
fn pairwise_pins_only_the_initiator_pair() {
    let net = Net::new(&["alpha", "beta"]);
    net.connect(&net.args(true, true, false)).unwrap();
    assert_eq!(net.peers(&net.mac), set(&["alpha", "beta"]));
    assert_eq!(net.peers(&net.fake.homes["alpha"]), set(&["mac"]));
    assert_eq!(net.peers(&net.fake.homes["beta"]), set(&["mac"]));
}

#[test]
fn preflight_failure_writes_nothing_anywhere() {
    let net = Net::new(&["alpha", "beta"]);
    *net.fake.fail.borrow_mut() = Some(("beta".into(), "network --help".into()));
    assert!(net.connect(&net.args(true, false, false)).is_err());
    let log = net.actions();
    assert!(!log.iter().any(|a| a.contains("setup")), "{log:?}");
    assert!(net.peers(&net.mac).is_empty());
    assert!(!net.fake.homes["alpha"].join("network.json").exists());
}

#[test]
fn check_failure_on_a_remote_writes_no_trust_locally_or_remotely() {
    let net = Net::new(&["alpha", "beta"]);
    *net.fake.fail.borrow_mut() = Some(("beta".into(), "network import --check".into()));
    assert!(net.connect(&net.args(true, false, false)).is_err());
    assert!(
        net.peers(&net.mac).is_empty(),
        "local trust written before all checks passed"
    );
    assert!(net.peers(&net.fake.homes["alpha"]).is_empty());
}

#[test]
fn partial_failure_is_recoverable_by_rerunning_the_same_command() {
    let net = Net::new(&["alpha", "beta"]);
    let args = net.args(true, false, false);
    *net.fake.fail.borrow_mut() = Some(("beta".into(), "network import".into()));
    assert!(net.connect(&args).is_err());
    // Half applied: this computer and alpha trust, beta does not.
    assert!(!net.peers(&net.mac).is_empty());
    assert!(net.peers(&net.fake.homes["beta"]).is_empty());
    let ids: Vec<Vec<u8>> = ["alpha", "beta"]
        .iter()
        .map(|h| sha(&net.fake.homes[*h]))
        .collect();
    net.connect(&args).expect("rerun converges");
    assert_eq!(net.peers(&net.fake.homes["beta"]), set(&["mac", "alpha"]));
    assert_eq!(net.pins(&net.mac).len(), 2, "no duplicate pins after rerun");
    assert_eq!(sha(&net.fake.homes["alpha"]), ids[0]);
    assert_eq!(sha(&net.fake.homes["beta"]), ids[1]);
}

#[test]
fn stale_pin_blocks_import_until_prune_then_exact_set() {
    let net = Net::new(&["alpha", "beta"]);
    // beta already trusts THIS computer's real key under an old name, as devhome did.
    let mac_key =
        TrustedVerifyingKey::from_b64url(&state::load(&net.mac).unwrap().local.public_key).unwrap();
    prepare_home(&net.fake.homes["beta"], "beta", 7101);
    let beta_ring = net.fake.homes["beta"].join("gateway/peers.keyring");
    std::fs::create_dir_all(beta_ring.parent().unwrap()).unwrap();
    Keyring::new()
        .with_peer("agent:100.99.215.33/alice".parse().unwrap(), mac_key)
        .unwrap()
        .save_to_file(&beta_ring)
        .unwrap();

    // Without --prune the stale pin is never silently dropped: the run is refused.
    assert!(net.connect(&net.args(true, false, false)).is_err());
    assert!(net
        .pins(&net.fake.homes["beta"])
        .contains("agent:100.99.215.33/alice"));
    assert!(
        net.peers(&net.mac).is_empty(),
        "nothing may be pinned locally either"
    );

    // With --prune the peer set is exactly the cards, and the old file is kept.
    net.connect(&net.args(true, false, true)).unwrap();
    let pins = net.pins(&net.fake.homes["beta"]);
    assert_eq!(pins.len(), 2, "{pins:?}");
    assert!(!pins.contains("agent:100.99.215.33/alice"));
    assert!(net.fake.homes["beta"]
        .join("gateway/peers.keyring.bak")
        .exists());
}

#[test]
fn expect_mismatch_aborts_before_any_trust_is_written() {
    let net = Net::new(&["alpha"]);
    let mut args = net.args(true, false, false);
    args.expect = vec!["alpha=NOTTHEKEYID0000".into()];
    let err = net.connect(&args).unwrap_err().to_string();
    assert!(err.contains("does not match --expect"), "{err}");
    assert!(net.peers(&net.mac).is_empty());
    assert!(!net.actions().iter().any(|a| a.contains("network import")));
}

#[test]
fn expect_match_proceeds() {
    let net = Net::new(&["alpha"]);
    let mut args = net.args(true, false, false);
    // Learn alpha's key_id the way a human would: from a prior export.
    net.fake
        .run("alpha", "setup --host none --no-services", None)
        .unwrap();
    let card: Card =
        serde_json::from_slice(&net.fake.run("alpha", "network export", None).unwrap()).unwrap();
    args.expect = vec![format!("alpha={}", card.fingerprint().unwrap())];
    net.connect(&args).unwrap();
    assert_eq!(net.peers(&net.mac), set(&["alpha"]));
}

#[test]
fn dry_run_executes_nothing() {
    let net = Net::new(&["alpha"]);
    let mut args = net.args(true, false, true);
    args.dry_run = true;
    net.connect(&args).unwrap();
    assert!(net.actions().is_empty());
    assert!(net.peers(&net.mac).is_empty());
}

#[test]
fn duplicate_hosts_are_refused() {
    let net = Net::new(&["alpha"]);
    let mut args = net.args(true, false, false);
    args.hosts = vec!["alpha".into(), "alpha".into()];
    assert!(net.connect(&args).is_err());
    assert!(net.actions().is_empty());
}

#[test]
fn changed_peer_card_is_refused_unless_pruning_and_versions_gate_only_exchange() {
    let net = Net::new(&["alpha"]);
    net.connect(&net.args(true, false, false)).unwrap();
    let mut alpha = state::load(&net.fake.homes["alpha"]).unwrap().local;
    let other_key = state::load(&net.mac).unwrap().local.public_key;
    alpha.public_key = other_key;
    // Accidental trust replacement: same name, different key.
    assert!(state::import(&net.mac, &[alpha.clone()], ImportOpts::default()).is_err());
    // Even with --prune, a peer may never present this computer's own key.
    assert!(state::import(
        &net.mac,
        &[alpha],
        ImportOpts {
            check: true,
            prune: true
        }
    )
    .is_err());

    // A stored card from an older build still validates (serve must not break),
    // but a card from a different build is refused at exchange time.
    let mut old = state::load(&net.fake.homes["alpha"]).unwrap().local;
    old.version = "0.0.1".into();
    assert!(old.validate().is_ok());
    assert!(old.check_version().is_err());
    assert!(state::import(
        &net.mac,
        &[old],
        ImportOpts {
            check: true,
            prune: false
        }
    )
    .is_err());
}

fn set_version(home: &Path, version: &str) {
    let mut config = state::load(home).unwrap();
    config.local.version = version.to_owned();
    for peer in config.peers.values_mut() {
        peer.version = version.to_owned();
    }
    state::save(home, &config).unwrap();
}

#[test]
fn connect_keeps_working_after_every_computer_is_upgraded() {
    let net = Net::new(&["alpha", "beta"]);
    net.connect(&net.args(true, false, false)).unwrap();
    // Simulate the next release: every stored card still carries the old version.
    for home in [
        net.mac.clone(),
        net.fake.homes["alpha"].clone(),
        net.fake.homes["beta"].clone(),
    ] {
        set_version(&home, "0.0.1");
    }
    net.connect(&net.args(true, false, false))
        .expect("rerun after an upgrade must converge, not fail on the old version stamp");
    for home in [&net.mac, &net.fake.homes["alpha"], &net.fake.homes["beta"]] {
        assert_eq!(
            state::load(home).unwrap().local.version,
            env!("CARGO_PKG_VERSION")
        );
    }
    assert_eq!(net.pins(&net.mac).len(), 2, "no duplicate or lost pins");
}

fn revoked_ring(path: &Path, principal: &str, key_b64: &str) {
    let principal: famp_core::Principal = principal.parse().unwrap();
    let key = TrustedVerifyingKey::from_b64url(key_b64).unwrap();
    let mut ring = Keyring::new()
        .with_peer(principal.clone(), key.clone())
        .unwrap();
    ring.revoke(
        &principal,
        &famp_crypto::key_id(&key),
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ring.save_to_file(path).unwrap();
}

#[test]
fn prune_cannot_resurrect_a_revoked_key() {
    let net = Net::new(&["alpha"]);
    net.fake.expect_no_services.set(true);
    net.fake
        .run("alpha", "setup --host none --no-services", None)
        .unwrap();
    let alpha = state::load(&net.fake.homes["alpha"]).unwrap().local;
    // This computer revoked alpha's key earlier.
    revoked_ring(
        &net.mac.join("gateway/peers.keyring"),
        &alpha.principal(),
        &alpha.public_key,
    );
    for prune in [false, true] {
        let err = net
            .connect(&net.args(true, false, prune))
            .unwrap_err()
            .to_string();
        assert!(err.contains("REVOKED"), "prune={prune}: {err}");
    }
    assert!(net.peers(&net.mac).is_empty());
}

#[test]
fn rerunning_prune_never_overwrites_the_original_backup() {
    let net = Net::new(&["alpha"]);
    net.connect(&net.args(true, false, false)).unwrap();
    let ring = net.mac.join("gateway/peers.keyring");
    let original = std::fs::read(&ring).unwrap();
    net.connect(&net.args(true, false, true)).unwrap();
    // A second prune run must not replace the first backup with the pruned file.
    std::fs::write(&ring, "").unwrap();
    net.connect(&net.args(true, false, true)).unwrap();
    assert_eq!(
        std::fs::read(net.mac.join("gateway/peers.keyring.bak")).unwrap(),
        original,
        "the first backup must survive reruns"
    );
    assert!(net.mac.join("gateway/peers.keyring.bak.1").exists());
}

fn card_with_cert(base: &Card, extra: &[&str]) -> Card {
    let dir = tempfile::tempdir().unwrap();
    let (crt, key) = (dir.path().join("c.pem"), dir.path().join("k.pem"));
    let mut cmd = std::process::Command::new("openssl");
    cmd.args([
        "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "365",
    ])
    .args(["-subj", "/CN=IP Address:10.9.9.9", "-keyout"])
    .arg(&key)
    .arg("-out")
    .arg(&crt)
    .args(extra);
    assert!(cmd.output().unwrap().status.success());
    let mut card = base.clone();
    card.certificate = std::fs::read_to_string(crt).unwrap();
    card
}

#[test]
fn cards_cannot_carry_a_ca_a_foreign_san_or_a_subject_that_mimics_one() {
    let net = Net::new(&[]);
    let base = state::load(&net.mac).unwrap().local;
    let ip = base.address.to_string();
    let good = [
        "-addext",
        &format!("subjectAltName=IP:{ip}"),
        "-addext",
        "basicConstraints=critical,CA:FALSE",
    ];
    assert!(card_with_cert(&base, &good).validate_shape().is_ok());
    // The subject says "IP Address:10.9.9.9" but the real SAN is something else.
    let mut other = base.clone();
    other.address = "10.9.9.9".parse().unwrap();
    assert!(
        card_with_cert(&other, &good).validate_shape().is_err(),
        "subject text must not count as a SAN"
    );
    // A CA certificate would become a trust anchor for any address.
    let ca = [
        "-addext",
        &format!("subjectAltName=IP:{ip}"),
        "-addext",
        "basicConstraints=critical,CA:TRUE",
    ];
    assert!(card_with_cert(&base, &ca).validate_shape().is_err());
    // Extra SAN addresses are refused.
    let extra = [
        "-addext",
        &format!("subjectAltName=IP:{ip},IP:10.0.0.9"),
        "-addext",
        "basicConstraints=critical,CA:FALSE",
    ];
    assert!(card_with_cert(&base, &extra).validate_shape().is_err());
}

#[test]
fn prune_carries_old_revocation_records_into_the_new_keyring() {
    let net = Net::new(&["alpha"]);
    // This computer revoked a key for a principal that is NOT in this run's set.
    let gone: famp_core::Principal = "agent:gone.famp/gone".parse().unwrap();
    let key = famp_crypto::FampSigningKey::from_bytes([77u8; 32]).verifying_key();
    let ring_path = net.mac.join("gateway/peers.keyring");
    std::fs::create_dir_all(ring_path.parent().unwrap()).unwrap();
    let mut ring = Keyring::new().with_peer(gone.clone(), key.clone()).unwrap();
    ring.revoke(&gone, &famp_crypto::key_id(&key), "2026-01-01T00:00:00Z")
        .unwrap();
    ring.save_to_file(&ring_path).unwrap();

    let revoked_lines = |p: &Path| {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .filter(|l| l.split_whitespace().nth(2) == Some("revoked"))
            .count()
    };
    assert_eq!(revoked_lines(&ring_path), 1);
    // Two prune runs in a row: the revocation must survive both, or a second run
    // (or a later card) could re-pin the revoked key.
    net.connect(&net.args(true, false, true)).unwrap();
    assert_eq!(
        revoked_lines(&ring_path),
        1,
        "revocation lost by the first prune"
    );
    net.connect(&net.args(true, false, true)).unwrap();
    assert_eq!(
        revoked_lines(&ring_path),
        1,
        "revocation lost by the second prune"
    );
    // And the new file is still a valid keyring that holds the real peer.
    let reloaded = Keyring::load_from_file(&ring_path).unwrap();
    assert!(
        reloaded.get(&gone).is_none(),
        "a revoked key must not be active"
    );
    assert_eq!(net.peers(&net.mac), set(&["alpha"]));
}

#[test]
fn two_cards_cannot_claim_the_same_address_and_port() {
    let net = Net::new(&["alpha", "beta"]);
    net.connect(&net.args(true, false, false)).unwrap();
    let mut alpha = state::load(&net.fake.homes["alpha"]).unwrap().local;
    let beta = state::load(&net.fake.homes["beta"]).unwrap().local;
    // Control: as set up, the endpoints differ and everything was accepted above.
    assert_ne!((alpha.address, alpha.port), (beta.address, beta.port));
    // alpha now claims beta's endpoint (same address and port).
    alpha.port = beta.port;
    let err = state::import(
        &net.mac,
        &[alpha, beta],
        ImportOpts {
            check: true,
            prune: true,
        },
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("already uses"), "{err}");
}

#[test]
fn a_card_whose_certificate_has_stray_text_around_it_is_refused() {
    let net = Net::new(&[]);
    let good = state::load(&net.mac).unwrap().local;
    assert!(
        good.validate_shape().is_ok(),
        "control: the untouched card is fine"
    );
    for framing in [
        format!(
            "-----BEGIN CERTIFICATE- - - - -\nAAAA\n{}",
            good.certificate
        ),
        format!("junk\n{}", good.certificate),
        format!("{}trailing", good.certificate),
    ] {
        let mut card = good.clone();
        card.certificate = framing;
        assert!(
            card.validate_shape().is_err(),
            "stray text must not reach the gateway's trust file"
        );
    }
}
