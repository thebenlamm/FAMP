//! `famp pair invite` — generate a five-word invite code (PAIR-01) and
//! persist a `Pending` invite record to `~/.famp/gateway/pairing.json`.
//!
//! Prints ONE artifact in one call to one writer (PAIR-08): a one-line
//! opening, the install step, [`crate::pairing::consent::CONSENT_WARNING`]
//! (before the code, so it is read while the decision is still open), the
//! `famp pair redeem --from <url>` step, a note that the code is typed at
//! a prompt, and the five-word code alone on the FINAL line — so the code
//! outlives the follower's slowest step (install). Refuses to run without
//! `--confirm-installed` (PAIR-08's generate-after-install clause,
//! T-18-19): the 24-hour window starts the moment this invite is created,
//! so it must not start burning down while the follower is still
//! installing `famp`.

use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use famp_core::Principal;
use rand::rngs::OsRng;

use crate::cli::error::CliError;
use crate::cli::home;
use crate::pairing::consent::CONSENT_WARNING;
use crate::pairing::invite::{
    pairing_store_path, InviteRecord, InviteState, InviteStore, StoreLock,
};
use crate::pairing::wordlist::{code_digest, draw_code, PairingCode};
use crate::pairing::PairingError;

/// Placeholder text substituted into the artifact's redeem step when
/// `--url` is absent. Contains characters (`<`, `>`) outside the
/// base64url alphabet by design, so it can never be mistaken for a
/// PAIR-04-prohibited key/fingerprint/id token by an artifact-scanning
/// test.
const URL_PLACEHOLDER: &str = "<PASTE-THIS-GATEWAYS-URL-HERE>";

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
    /// POST. When absent, the artifact prints a clearly-marked
    /// placeholder and a stderr note reminding the inviter to fill it in
    /// before sending.
    #[arg(long)]
    pub url: Option<String>,
    /// Required: confirms the follower's `famp` install already works
    /// (e.g. `famp --version` succeeded on their machine) BEFORE this
    /// invite is created. Without it, `run_at` exits 2 and creates no
    /// record — see this module's doc comment for why (PAIR-08,
    /// T-18-19).
    #[arg(long)]
    pub confirm_installed: bool,
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
///
/// Without `args.confirm_installed`: returns `CliError::Exit(2)` after
/// printing why to stderr, writes NOTHING to `out`, and creates NO
/// record — mirrors `famp peer rotate`'s missing-`--confirm-rotation`
/// exit-2-and-mutate-nothing contract exactly (T-18-19).
pub fn run_at<R: rand::Rng>(
    home: &Path,
    args: &PairInviteArgs,
    out: &mut dyn Write,
    now: &str,
    rng: &mut R,
) -> Result<(), CliError> {
    if !args.confirm_installed {
        eprintln!(
            "Refusing to create an invite without --confirm-installed: this invite's \
             24-hour window starts the moment it is created, so it must not start \
             burning down while the other person is still installing famp."
        );
        eprintln!(
            "Confirm their install works first (they should be able to run \
             `famp --version` successfully), then re-run with --confirm-installed."
        );
        return Err(CliError::Exit(2));
    }

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
    let invite_id = record.id.clone();

    let store_path = pairing_store_path(home);
    {
        let _lock = StoreLock::acquire(&store_path).map_err(pairing_err_to_cli)?;
        let mut store = InviteStore::load(&store_path).map_err(pairing_err_to_cli)?;
        store.invites.push(record);
        store.save_atomic(&store_path).map_err(pairing_err_to_cli)?;
    }

    let artifact = build_artifact(args, &code, &principal);
    out.write_all(artifact.as_bytes())
        .map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: e,
        })?;

    if args.url.is_none() {
        eprintln!(
            "NOTE: no --url given; the artifact above has a placeholder where this \
             gateway's base URL belongs. Fill it in before sending the artifact."
        );
    }
    eprintln!(
        "NOTE: this invite (id {invite_id}) is valid for 24 hours and allows 5 wrong \
         guesses before it locks. Run `famp pair status` on this machine once it has \
         been redeemed, or `famp pair revoke --id {invite_id}` to cancel it."
    );

    Ok(())
}

/// Build the ONE artifact string this invite prints, in the exact
/// PAIR-08 section order: opening line, install step, consent warning,
/// redeem step, "type at the prompt" note, and the code alone on the
/// FINAL line. Built and returned as a single `String` — `run_at` writes
/// it to `out` in exactly one call, never piecemeal.
fn build_artifact(args: &PairInviteArgs, code: &PairingCode, principal: &Principal) -> String {
    let from_value = args.url.as_deref().unwrap_or(URL_PLACEHOLDER);

    let mut buf = String::new();
    let _ = writeln!(buf, "{principal} is inviting you to pair FAMP agents.");
    buf.push('\n');

    buf.push_str("Step 1 -- install famp (skip this if it is already installed):\n");
    buf.push_str(
        "  curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh\n",
    );
    buf.push_str("  Then check it worked:\n");
    buf.push_str("  famp --version\n");
    buf.push('\n');

    buf.push_str(CONSENT_WARNING);
    buf.push('\n');
    buf.push('\n');

    buf.push_str("Step 2 -- on that same machine, run:\n");
    // `--as <your-agent-name>` is a marked placeholder (D4): `redeem`
    // requires `--as` and the inviter cannot know the follower's chosen
    // name, so the printed command names the flag without a value.
    let _ = writeln!(
        buf,
        "  famp pair redeem --from {from_value} --as <your-agent-name>"
    );
    buf.push('\n');

    buf.push_str(
        "Step 3 -- when it asks for the code, type it there. Do not add it to the command above.\n",
    );
    buf.push('\n');

    buf.push_str(code.as_str());
    buf.push('\n');
    buf
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
    use crate::cli::error::CliError;
    use crate::pairing::invite::{pairing_store_path, InviteState, InviteStore};
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn run_at_persists_pending_record_and_prints_code_last() {
        let tmp = tempfile::tempdir().unwrap();
        let args = PairInviteArgs {
            as_principal: "agent:inviter.test/gateway".to_string(),
            url: Some("https://gateway.inviter.test:8443".to_string()),
            confirm_installed: true,
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

    #[test]
    fn run_at_without_confirm_installed_exits_2_writes_nothing_creates_no_record() {
        let tmp = tempfile::tempdir().unwrap();
        let args = PairInviteArgs {
            as_principal: "agent:inviter.test/gateway".to_string(),
            url: Some("https://gateway.inviter.test:8443".to_string()),
            confirm_installed: false,
        };
        let mut out = Vec::new();
        let mut rng = StdRng::seed_from_u64(1);
        let err = run_at(
            tmp.path(),
            &args,
            &mut out,
            "2026-08-03T00:00:00Z",
            &mut rng,
        )
        .unwrap_err();

        assert!(
            matches!(err, CliError::Exit(2)),
            "expected Exit(2), got {err:?}"
        );
        assert!(out.is_empty(), "writer must receive zero bytes");
        assert!(
            !pairing_store_path(tmp.path()).exists(),
            "no pairing.json must be created without --confirm-installed"
        );
    }
}
