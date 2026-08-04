//! `famp pair invite` — generate a five-word invite code (PAIR-01) and
//! persist a `Pending` invite record to `~/.famp/gateway/pairing.json`.
//!
//! In THIS plan the printed artifact is minimal: an install line naming
//! `famp pair redeem --from <url>` followed by the five-word code on the
//! LAST line. Plan 03 owns the real artifact text, the consent warning,
//! and the PAIR-08 layout-ordering assertions — not attempted here.

use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use famp_core::Principal;
use rand::rngs::OsRng;

use crate::cli::error::CliError;
use crate::cli::home;
use crate::pairing::invite::{
    pairing_store_path, InviteRecord, InviteState, InviteStore, StoreLock,
};
use crate::pairing::wordlist::{code_digest, draw_code, PairingCode};
use crate::pairing::PairingError;

/// CLI args for `famp pair invite`.
#[derive(clap::Args, Debug)]
pub struct PairInviteArgs {
    /// The principal this invite is FOR — a full principal
    /// (`agent:<authority>/<name>`, e.g. `agent:inviter.test/gateway`).
    /// This is the identity the redeemer will be pinned under once
    /// redeemed and `famp pair status` confirms it.
    #[arg(long = "as")]
    pub as_principal: String,
    /// Base URL of THIS gateway's pairing route (e.g.
    /// `https://gateway.example.com:8443`), printed in the artifact so
    /// the redeemer's `famp pair redeem --from <url>` knows where to
    /// POST. Optional in this plan (Plan 03 owns the full artifact
    /// text); when absent, the artifact prints a placeholder reminder
    /// instead of a real URL.
    #[arg(long)]
    pub url: Option<String>,
}

/// Production entry point.
pub fn run(args: &PairInviteArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = super::now_canonical_utc();
    let mut stdout = std::io::stdout().lock();
    let mut rng = OsRng;
    run_at(&home_path, args, &mut stdout, &now, &mut rng)
}

/// Test-facing entry point.
///
/// Explicit `&Path` + writer + `now` + an injected `rand::Rng` so tests
/// can substitute a seeded `StdRng` for the uniform-coverage proof,
/// matching the `run`/`run_at` split convention.
pub fn run_at<R: rand::Rng>(
    home: &Path,
    args: &PairInviteArgs,
    out: &mut dyn Write,
    now: &str,
    rng: &mut R,
) -> Result<(), CliError> {
    let principal: Principal = args.as_principal.parse().map_err(|e| {
        CliError::Generic(format!(
            "invalid --as principal '{}': {e}",
            args.as_principal
        ))
    })?;

    let code = draw_code(rng);
    let digest = code_digest(&code);
    let expires_at = add_24h(now)?;

    let record = InviteRecord {
        id: uuid::Uuid::now_v7().to_string(),
        principal: principal.to_string(),
        code_digest: hex::encode(digest),
        created_at: now.to_string(),
        expires_at,
        attempts: 0,
        state: InviteState::Pending,
    };

    let store_path = pairing_store_path(home);
    {
        let _lock = StoreLock::acquire(&store_path).map_err(pairing_err_to_cli)?;
        let mut store = InviteStore::load(&store_path).map_err(pairing_err_to_cli)?;
        store.invites.push(record);
        store.save_atomic(&store_path).map_err(pairing_err_to_cli)?;
    }

    write_artifact(out, home, args, &code, &principal)?;
    Ok(())
}

/// Write the minimal Plan-01 artifact: install instructions, code LAST.
fn write_artifact(
    out: &mut dyn Write,
    home: &Path,
    args: &PairInviteArgs,
    code: &PairingCode,
    principal: &Principal,
) -> Result<(), CliError> {
    let from_hint = args
        .url
        .clone()
        .unwrap_or_else(|| "<this gateway's base url>".to_string());
    let mut buf = String::new();
    let _ = writeln!(
        buf,
        "Pairing invite for {principal} — valid 24 hours, single use."
    );
    buf.push_str("On the OTHER machine, run:\n");
    let _ = writeln!(buf, "  famp pair redeem --from {from_hint}");
    buf.push_str("Then read out (or text) this five-word code:\n");
    buf.push_str(code.as_str());
    buf.push('\n');
    out.write_all(buf.as_bytes()).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })
}

/// `now` plus 24 hours (PAIR-03's explicit 24-hour window), formatted in
/// the same canonical-UTC 20-byte shape `now` itself is already in.
fn add_24h(now: &str) -> Result<String, CliError> {
    let ts = time::OffsetDateTime::parse(now, &time::format_description::well_known::Rfc3339)
        .map_err(|e| CliError::Generic(format!("invalid now timestamp '{now}': {e}")))?;
    let expires = ts + time::Duration::hours(24);
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        expires.year(),
        u8::from(expires.month()),
        expires.day(),
        expires.hour(),
        expires.minute(),
        expires.second(),
    ))
}

// Takes `PairingError` by value (not `&PairingError`) so it can be passed
// as a bare fn pointer to `.map_err(pairing_err_to_cli)` at every call
// site in this file, matching the precedent at
// `crates/famp/src/cli/install/grok.rs:42`.
#[allow(clippy::needless_pass_by_value)]
fn pairing_err_to_cli(e: PairingError) -> CliError {
    CliError::Generic(e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{run_at, PairInviteArgs};
    use crate::pairing::invite::{pairing_store_path, InviteState, InviteStore};
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn run_at_persists_pending_record_and_prints_code_last() {
        let tmp = tempfile::tempdir().unwrap();
        let args = PairInviteArgs {
            as_principal: "agent:inviter.test/gateway".to_string(),
            url: Some("https://gateway.inviter.test:8443".to_string()),
        };
        let mut out = Vec::new();
        let mut rng = StdRng::seed_from_u64(1);
        run_at(
            tmp.path(),
            &args,
            &mut out,
            "2026-08-03T00:00:00Z",
            &mut rng,
        )
        .unwrap();

        let store = InviteStore::load(&pairing_store_path(tmp.path())).unwrap();
        assert_eq!(store.invites.len(), 1);
        assert_eq!(store.invites[0].principal, "agent:inviter.test/gateway");
        assert!(matches!(store.invites[0].state, InviteState::Pending));
        assert_eq!(store.invites[0].expires_at, "2026-08-04T00:00:00Z");

        let printed = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = printed.trim_end().lines().collect();
        let last_line = lines.last().unwrap();
        assert_eq!(
            last_line.split(' ').count(),
            5,
            "code (last line) must be exactly 5 space-joined words: {last_line}"
        );
    }
}
