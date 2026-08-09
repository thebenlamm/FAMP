//! `famp pair status` — poll the invite store for `Redeemed` records and
//! pin the redeemer's key into this machine's own peer keyring.
//!
//! (PAIR-01/PAIR-07). The ONLY place the inviter-side pin ever happens:
//! never inside the redemption endpoint, never in a background loop.
//! Prints the fixed, greppable `REDEEMED BY: <principal>  key_id=<key_id>`
//! identity line BEFORE pinning (asymmetric completion, PAIR-07) — an
//! operator sees who is about to be trusted before the trust write
//! happens, not just after. [`pin_redeemed_record`] makes that ordering
//! STRUCTURAL rather than incidental: it writes and flushes the identity
//! line via [`observe_line`] in one call, then calls [`pin`] — which
//! contains 100% of this module's filesystem mutation — only afterward.
//! A test can wrap the `out` writer to snapshot the keyring file the
//! moment the identity line's bytes land, before `pin` has run.

use std::io::Write;
use std::path::Path;

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use famp_keyring::{KeyLookupOutcome, Keyring};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::gateway_peers_keyring_path;
use crate::pairing::invite::{
    pairing_store_path, InviteRecord, InviteState, InviteStore, StoreLock,
};
use crate::pairing::PairingError;

/// CLI args for `famp pair status`. No fields — this subcommand takes no
/// input, only `$FAMP_HOME`.
#[derive(clap::Args, Debug)]
pub struct PairStatusArgs {}

/// Production entry point.
pub fn run(_args: &PairStatusArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = super::now_canonical_utc();
    let mut stdout = std::io::stdout().lock();
    run_at(&home_path, &mut stdout, &now)
}

/// One-line message printed when the store holds no `Redeemed` records
/// yet — the inviter is expected to poll, so this is success (`Ok(())`),
/// never an error, and touches nothing on disk.
const NOTHING_REDEEMED_YET: &str = "Nothing redeemed yet. Wait for the other person to run \
     `famp pair redeem`, or check they received the code.";

/// Test-facing entry point: explicit `&Path` + writer + `now`.
pub fn run_at(home: &Path, out: &mut dyn Write, now: &str) -> Result<(), CliError> {
    let store_path = pairing_store_path(home);
    let keyring_path = gateway_peers_keyring_path(home);

    let _lock = StoreLock::acquire(&store_path).map_err(pairing_err_to_cli)?;
    let mut store = InviteStore::load(&store_path).map_err(pairing_err_to_cli)?;

    let mut any_pinned = false;
    let mut pin_failed = false;
    let mut any_redeemed_seen = false;

    for record in &mut store.invites {
        // Clone the owned fields out (rather than borrowing `record.state`)
        // so this loop body is free to reassign `record.state` further
        // down without fighting the borrow checker over a live immutable
        // borrow.
        let InviteState::Redeemed {
            by,
            key_id,
            pubkey_b64url,
            at: _,
        } = record.state.clone()
        else {
            continue;
        };
        any_redeemed_seen = true;

        let info = RedeemedInfo {
            by: &by,
            key_id: &key_id,
            pubkey_b64url: &pubkey_b64url,
        };
        match pin_redeemed_record(out, home, &keyring_path, record, &info, now)? {
            PinOutcome::Pinned => any_pinned = true,
            PinOutcome::Failed => pin_failed = true,
        }
    }

    if any_pinned {
        store.save_atomic(&store_path).map_err(pairing_err_to_cli)?;
    }

    if pin_failed {
        return Err(CliError::Exit(1));
    }

    if !any_redeemed_seen {
        writeln!(out, "{NOTHING_REDEEMED_YET}").map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;
    }

    Ok(())
}

/// Outcome of [`pin_redeemed_record`] for one `Redeemed` invite record.
enum PinOutcome {
    Pinned,
    Failed,
}

/// The three `InviteState::Redeemed` fields [`pin_redeemed_record`] needs,
/// bundled into one struct so the function stays under the repo's
/// `too_many_arguments` budget rather than taking `by`/`key_id`/
/// `pubkey_b64url` as three separate parameters.
struct RedeemedInfo<'a> {
    by: &'a str,
    key_id: &'a str,
    pubkey_b64url: &'a str,
}

/// Build the fixed, greppable observe-before-pin identity line (PAIR-07):
/// `REDEEMED BY: <principal>  key_id=<key_id>`. A pure function — no I/O —
/// so [`pin_redeemed_record`] can hand its bytes to `out` in exactly one
/// `write_all` call. That single call is what lets a test wrap `out` in a
/// `Write` adapter that snapshots the keyring file the instant these bytes
/// are handed to it, before [`pin`] (this function's ONLY caller for any
/// filesystem mutation) has run.
fn observe_line(by: &str, key_id: &str) -> String {
    format!("REDEEMED BY: {by}  key_id={key_id}\n")
}

/// The verbatim `famp-gateway` keyring-reload restart notice, matching the
/// wording `crates/famp/src/cli/peer/rotate.rs` (lines 124-137) prints
/// after a confirmed rotation, adapted only in its final clause to name
/// pinning rather than rotation — the same per-surface adaptation
/// `peer/revoke.rs`, `peer/retire.rs`, and `peer/import_revocation.rs`
/// each already make for their own final clause.
const RESTART_NOTICE: &str = "NOTE: famp-gateway loads its keyring once at startup and will \
     keep honoring the previous key until it restarts. Run `famp daemon restart` (or \
     manually restart the gateway if it is not daemon-managed) to pick up the newly pinned \
     key.";

/// Pin one already-`Redeemed` record's key into `keyring_path`.
///
/// Extracted out of [`run_at`] to keep that function under the repo's
/// `too_many_lines` line budget (see `crates/famp-gateway/src/main.rs`
/// for the same extract-a-helper precedent). Structurally
/// observe-before-pin (PAIR-07): writes and flushes [`observe_line`]'s
/// bytes to `out` FIRST, then calls [`pin`] — the only place in this
/// module that touches the keyring file — second. On success, mutates
/// `record.state` to `Pinned`; on any failure it leaves `record.state`
/// untouched (still `Redeemed`) so a re-run of `famp pair status` can
/// retry.
fn pin_redeemed_record(
    out: &mut dyn Write,
    home: &Path,
    keyring_path: &Path,
    record: &mut InviteRecord,
    info: &RedeemedInfo<'_>,
    now: &str,
) -> Result<PinOutcome, CliError> {
    let by = info.by;
    let key_id = info.key_id;
    let pubkey_b64url = info.pubkey_b64url;

    // Observe BEFORE pin (PAIR-07): one write_all + flush of the fixed
    // identity line. The operator sees who is about to be trusted before
    // any trust write happens.
    out.write_all(observe_line(by, key_id).as_bytes())
        .map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;
    out.flush().map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;

    let redeemer_principal: Principal = match by.parse() {
        Ok(p) => p,
        Err(e) => {
            writeln!(out, "  refusing to pin: invalid principal '{by}': {e}").map_err(|e| {
                CliError::Io {
                    path: home.to_path_buf(),
                    source: e,
                }
            })?;
            return Ok(PinOutcome::Failed);
        }
    };
    let vk = match TrustedVerifyingKey::from_b64url(pubkey_b64url) {
        Ok(vk) => vk,
        Err(e) => {
            writeln!(out, "  refusing to pin: invalid pubkey encoding: {e}").map_err(|e| {
                CliError::Io {
                    path: home.to_path_buf(),
                    source: e,
                }
            })?;
            return Ok(PinOutcome::Failed);
        }
    };

    if pin(keyring_path, &redeemer_principal, vk, now)? {
        record.state = InviteState::Pinned {
            at: now.to_string(),
        };
        // One-sentence done-signal (PAIR-07: "not FSM JSON"), then the
        // restart notice on its own line.
        writeln!(out, "Paired with {redeemer_principal}.").map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;
        writeln!(out, "{RESTART_NOTICE}").map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;
        Ok(PinOutcome::Pinned)
    } else {
        writeln!(
            out,
            "  pin did not verify on reload at {} — leaving invite Redeemed; re-run \
             `famp pair status`",
            keyring_path.display()
        )
        .map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;
        Ok(PinOutcome::Failed)
    }
}

/// Mutate `keyring_path` on disk: load-or-create, `rotate_to`, write to a
/// same-directory temp file, reload-confirm the pin from THAT temp file,
/// and only then rename it over `keyring_path`. Returns `Ok(true)` once
/// the reload confirms `redeemer_principal` is `Active`; on `Ok(false)`
/// the temp file is removed and `keyring_path` is left byte-identical to
/// before this call.
///
/// This function contains ALL of this module's filesystem mutation. It is
/// called only AFTER [`pin_redeemed_record`] has already written and
/// flushed the observe line — that call ordering, not review discipline,
/// is what makes observe-before-pin structural (PAIR-07).
///
/// Validate-before-save (Task 3, this fix): the previous version called
/// `save_to_file` on `keyring_path` directly, THEN reload-validated it —
/// so a pin that failed validation had already overwritten the last-good
/// keyring on disk with one that (by definition of the failed validation)
/// `Keyring::load_from_file` may refuse to load, bricking the gateway at
/// its next start. Writing to a `.tmp-pin` sibling and renaming only on a
/// validated reload keeps a failed pin from ever touching the real file.
/// The rename is same-directory (`std::fs::rename`, not copy+delete), so
/// it is atomic on the filesystems this repo targets — the deliberate,
/// idempotent mitigation for `Keyring::save_to_file`'s inherited
/// non-atomic write (T-Keyring-non-atomic, Phase 15 pre-existing gap, not
/// fixed here — see this plan's `18-PATTERNS.md`).
fn pin(
    keyring_path: &Path,
    redeemer_principal: &Principal,
    vk: TrustedVerifyingKey,
    now: &str,
) -> Result<bool, CliError> {
    let mut keyring = if keyring_path.exists() {
        Keyring::load_from_file(keyring_path).map_err(|e| {
            CliError::Generic(format!(
                "failed to load peer keyring at {}: {e}",
                keyring_path.display()
            ))
        })?
    } else {
        Keyring::new()
    };
    keyring
        .rotate_to(redeemer_principal.clone(), vk, now, None, true)
        .map_err(|e| CliError::Generic(e.to_string()))?;
    if let Some(parent) = keyring_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let tmp_path = std::path::PathBuf::from(format!("{}.tmp-pin", keyring_path.display()));
    keyring.save_to_file(&tmp_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            tmp_path.display()
        ))
    })?;

    let reloaded = Keyring::load_from_file(&tmp_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to reload peer keyring at {}: {e}",
            tmp_path.display()
        ))
    })?;
    let validated = matches!(
        reloaded.active_key(redeemer_principal, now),
        KeyLookupOutcome::Active(_)
    );

    if validated {
        std::fs::rename(&tmp_path, keyring_path).map_err(|e| CliError::Io {
            path: keyring_path.to_path_buf(),
            source: e,
        })?;
    } else {
        // Best-effort cleanup: the temp file never becomes the real
        // keyring either way, so a failed removal here does not leave
        // `keyring_path` in a bad state -- it only leaves a stray
        // `.tmp-pin` sibling, which the next `pin()` call overwrites.
        let _ = std::fs::remove_file(&tmp_path);
    }

    Ok(validated)
}

// Takes `PairingError` by value so it can be passed as a bare fn pointer
// to `.map_err(pairing_err_to_cli)`, matching the precedent at
// `crates/famp/src/cli/install/grok.rs:42`.
#[allow(clippy::needless_pass_by_value)]
fn pairing_err_to_cli(e: PairingError) -> CliError {
    CliError::Generic(e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{run_at, PairStatusArgs};
    use crate::cli::peer::identity::gateway_peers_keyring_path;
    use crate::pairing::invite::{pairing_store_path, InviteRecord, InviteState, InviteStore};
    use famp_crypto::{key_id, FampSigningKey};
    use famp_keyring::Keyring;

    #[test]
    fn run_at_pins_redeemed_record_and_marks_it_pinned() {
        let _ = PairStatusArgs {};
        let tmp = tempfile::tempdir().unwrap();
        let sk = FampSigningKey::from_bytes([9u8; 32]);
        let vk = sk.verifying_key();

        let record = InviteRecord {
            id: "inv-1".to_string(),
            principal: "agent:inviter.test/gateway".to_string(),
            code_digest: "a".repeat(64),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            expires_at: "2026-08-04T00:00:00Z".to_string(),
            attempts: 0,
            state: InviteState::Redeemed {
                by: "agent:redeemer.test/gateway".to_string(),
                key_id: key_id(&vk),
                pubkey_b64url: vk.to_b64url(),
                at: "2026-08-03T01:00:00Z".to_string(),
            },
        };
        InviteStore {
            invites: vec![record],
        }
        .save_atomic(&pairing_store_path(tmp.path()))
        .unwrap();

        let mut out = Vec::new();
        run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z").unwrap();

        let store = InviteStore::load(&pairing_store_path(tmp.path())).unwrap();
        assert!(matches!(store.invites[0].state, InviteState::Pinned { .. }));

        let keyring = Keyring::load_from_file(&gateway_peers_keyring_path(tmp.path())).unwrap();
        let principal: famp_core::Principal = "agent:redeemer.test/gateway".parse().unwrap();
        assert_eq!(keyring.get(&principal).unwrap().to_b64url(), vk.to_b64url());

        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("REDEEMED BY: agent:redeemer.test/gateway  key_id="));
        assert!(printed.contains("famp-gateway loads its keyring once at startup"));
    }

    #[test]
    fn run_at_prints_nothing_redeemed_yet_when_no_redeemed_records_exist() {
        let tmp = tempfile::tempdir().unwrap();
        InviteStore {
            invites: vec![InviteRecord {
                id: "inv-2".to_string(),
                principal: "agent:inviter.test/gateway".to_string(),
                code_digest: "b".repeat(64),
                created_at: "2026-08-03T00:00:00Z".to_string(),
                expires_at: "2026-08-04T00:00:00Z".to_string(),
                attempts: 0,
                state: InviteState::Pending,
            }],
        }
        .save_atomic(&pairing_store_path(tmp.path()))
        .unwrap();

        let mut out = Vec::new();
        run_at(tmp.path(), &mut out, "2026-08-03T02:00:00Z").unwrap();
        let printed = String::from_utf8(out).unwrap();
        assert!(
            printed.contains("Nothing redeemed yet"),
            "zero Redeemed records is success with a one-line nothing-to-do message, not an \
             empty writer: {printed}"
        );
        assert!(!gateway_peers_keyring_path(tmp.path()).exists());
    }
}
