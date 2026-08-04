//! `famp pair revoke` — the operator kill switch PAIR-03 explicitly
//! requires.
//!
//! An outstanding invite (or every currently `Pending` one) can be
//! killed before its 24-hour window closes, and the kill is durable
//! across a gateway restart because it lives in the same persisted
//! `pairing.json` record every other write goes through.
//!
//! Sync (no network I/O) — dispatches alongside `invite`/`status`, not
//! through the async `redeem` path (`cli::pair::mod`'s `run`).

use std::path::Path;

use crate::pairing::invite::{pairing_store_path, InviteStore, StoreLock};
use crate::pairing::PairingError;

use crate::cli::error::CliError;
use crate::cli::home;

/// CLI args for `famp pair revoke`.
///
/// `--id` and `--all-pending` are mutually exclusive; exactly one is
/// required (clap-enforced): neither given is a clap error, both given
/// at once is a clap conflict error.
#[derive(clap::Args, Debug)]
pub struct PairRevokeArgs {
    /// Revoke the single invite with this id.
    #[arg(
        long,
        conflicts_with = "all_pending",
        required_unless_present = "all_pending"
    )]
    pub id: Option<String>,
    /// Revoke every currently `Pending` invite. Reports the count
    /// revoked; a store with zero `Pending` records reports zero and
    /// still exits 0.
    #[arg(long, conflicts_with = "id", required_unless_present = "id")]
    pub all_pending: bool,
}

/// Production entry point.
pub fn run(args: &PairRevokeArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = super::now_canonical_utc();
    run_at(&home_path, args, &mut std::io::stdout(), &now)
}

/// Test-facing entry point: takes an explicit `&Path`, writer, and `now`.
pub fn run_at(
    home: &Path,
    args: &PairRevokeArgs,
    writer: &mut dyn std::io::Write,
    now: &str,
) -> Result<(), CliError> {
    let store_path = pairing_store_path(home);
    let lock = StoreLock::acquire(&store_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to acquire pairing store lock at {}: {e}",
            store_path.display()
        ))
    })?;
    let mut store = InviteStore::load(&store_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to load pairing store at {}: {e}",
            store_path.display()
        ))
    })?;

    if let Some(id) = args.id.as_deref() {
        match store.revoke(id, now) {
            Ok(()) => {
                store.save_atomic(&store_path).map_err(|e| {
                    CliError::Generic(format!(
                        "failed to save pairing store at {}: {e}",
                        store_path.display()
                    ))
                })?;
                drop(lock);
                writeln!(writer, "revoked invite {id}").map_err(|e| CliError::Io {
                    path: store_path.clone(),
                    source: e,
                })?;
                Ok(())
            }
            Err(PairingError::UnknownInvite { id }) => {
                drop(lock);
                let known_ids: Vec<&str> = store.invites.iter().map(|r| r.id.as_str()).collect();
                eprintln!(
                    "no invite with id '{id}' — known ids: [{}]",
                    known_ids.join(", ")
                );
                Err(CliError::Exit(1))
            }
            Err(other) => {
                drop(lock);
                Err(CliError::Generic(other.to_string()))
            }
        }
    } else {
        debug_assert!(args.all_pending, "clap must guarantee one of the two flags");
        let count = store.revoke_all_pending(now);
        store.save_atomic(&store_path).map_err(|e| {
            CliError::Generic(format!(
                "failed to save pairing store at {}: {e}",
                store_path.display()
            ))
        })?;
        drop(lock);
        writeln!(writer, "revoked {count} pending invite(s)").map_err(|e| CliError::Io {
            path: store_path.clone(),
            source: e,
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::pairing::invite::{pairing_store_path, InviteRecord, InviteState, InviteStore};

    use super::{run_at, PairRevokeArgs};
    use crate::cli::error::CliError;

    fn sample_record(id: &str) -> InviteRecord {
        InviteRecord {
            id: id.to_string(),
            principal: "agent:inviter.test/gateway".to_string(),
            code_digest: "a".repeat(64),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            expires_at: "2026-08-04T00:00:00Z".to_string(),
            attempts: 0,
            state: InviteState::Pending,
        }
    }

    #[test]
    fn revoke_by_id_transitions_to_revoked_and_prints_confirmation() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = pairing_store_path(tmp.path());
        InviteStore {
            invites: vec![sample_record("inv-1"), sample_record("inv-2")],
        }
        .save_atomic(&store_path)
        .unwrap();

        let mut out = Vec::new();
        run_at(
            tmp.path(),
            &PairRevokeArgs {
                id: Some("inv-1".to_string()),
                all_pending: false,
            },
            &mut out,
            "2026-08-03T00:00:01Z",
        )
        .unwrap();

        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("inv-1"));

        let store = InviteStore::load(&store_path).unwrap();
        assert!(matches!(
            store.invites[0].state,
            InviteState::Revoked { .. }
        ));
        assert!(
            matches!(store.invites[1].state, InviteState::Pending),
            "the other record must be untouched"
        );
    }

    #[test]
    fn revoke_unknown_id_exits_1_and_leaves_store_byte_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = pairing_store_path(tmp.path());
        InviteStore {
            invites: vec![sample_record("inv-1")],
        }
        .save_atomic(&store_path)
        .unwrap();
        let before = std::fs::read(&store_path).unwrap();

        let mut out = Vec::new();
        let err = run_at(
            tmp.path(),
            &PairRevokeArgs {
                id: Some("does-not-exist".to_string()),
                all_pending: false,
            },
            &mut out,
            "2026-08-03T00:00:01Z",
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Exit(1)));

        let after = std::fs::read(&store_path).unwrap();
        assert_eq!(
            before, after,
            "an unknown-id revoke must leave pairing.json byte-identical"
        );
    }

    #[test]
    fn revoke_all_pending_reports_count_and_revokes_every_pending_record() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = pairing_store_path(tmp.path());
        let mut redeemed = sample_record("inv-2");
        redeemed.state = InviteState::Redeemed {
            by: "agent:redeemer.test/gateway".to_string(),
            key_id: "key-1".to_string(),
            pubkey_b64url: "pk".to_string(),
            at: "2026-08-03T00:01:00Z".to_string(),
        };
        InviteStore {
            invites: vec![sample_record("inv-1"), redeemed],
        }
        .save_atomic(&store_path)
        .unwrap();

        let mut out = Vec::new();
        run_at(
            tmp.path(),
            &PairRevokeArgs {
                id: None,
                all_pending: true,
            },
            &mut out,
            "2026-08-03T00:00:01Z",
        )
        .unwrap();

        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains('1'), "expected the count 1 in: {printed}");

        let store = InviteStore::load(&store_path).unwrap();
        assert!(matches!(
            store.invites[0].state,
            InviteState::Revoked { .. }
        ));
        assert!(
            matches!(store.invites[1].state, InviteState::Redeemed { .. }),
            "an already-Redeemed record must not be touched by revoke_all_pending"
        );
    }

    #[test]
    fn revoke_all_pending_on_zero_pending_reports_zero_and_exits_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = pairing_store_path(tmp.path());
        InviteStore { invites: vec![] }
            .save_atomic(&store_path)
            .unwrap();

        let mut out = Vec::new();
        run_at(
            tmp.path(),
            &PairRevokeArgs {
                id: None,
                all_pending: true,
            },
            &mut out,
            "2026-08-03T00:00:01Z",
        )
        .unwrap();

        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains('0'), "expected the count 0 in: {printed}");
    }

    #[test]
    fn revoked_invite_is_rejected_as_no_pending_invite_by_decide() {
        let tmp = tempfile::tempdir().unwrap();
        let store_path = pairing_store_path(tmp.path());
        let record = sample_record("inv-1");
        let digest = crate::pairing::wordlist::digest_from_hex(&record.code_digest).unwrap();
        InviteStore {
            invites: vec![record],
        }
        .save_atomic(&store_path)
        .unwrap();

        let mut out = Vec::new();
        run_at(
            tmp.path(),
            &PairRevokeArgs {
                id: Some("inv-1".to_string()),
                all_pending: false,
            },
            &mut out,
            "2026-08-03T00:00:01Z",
        )
        .unwrap();

        let store = InviteStore::load(&store_path).unwrap();
        assert_eq!(
            store.decide(&digest, "2026-08-03T00:00:02Z"),
            crate::pairing::invite::RedemptionDecision::NoPendingInvite,
            "a revoked invite's code must never be redeemable again"
        );
    }

    /// Process-level proof of clap's conflict enforcement: `--id` and
    /// `--all-pending` together must be rejected before any code in this
    /// module ever runs (mirrors `peer_rotate_cli.rs`'s process-level
    /// exit-code cases).
    #[test]
    fn cli_rejects_id_and_all_pending_together() {
        use assert_cmd::Command as AssertCommand;
        let tmp = tempfile::tempdir().unwrap();
        let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
        let output = cmd
            .env("FAMP_HOME", tmp.path())
            .args(["pair", "revoke", "--id", "inv-1", "--all-pending"])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "expected clap to reject both flags together"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot be used with"),
            "expected clap's conflict message, got: {stderr}"
        );
    }

    /// Process-level proof that neither flag given is ALSO a clap error
    /// (the "exactly one required" half of the contract).
    #[test]
    fn cli_rejects_neither_id_nor_all_pending() {
        use assert_cmd::Command as AssertCommand;
        let tmp = tempfile::tempdir().unwrap();
        let mut cmd = AssertCommand::cargo_bin("famp").unwrap();
        let output = cmd
            .env("FAMP_HOME", tmp.path())
            .args(["pair", "revoke"])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "expected clap to require one of --id / --all-pending"
        );
    }
}
