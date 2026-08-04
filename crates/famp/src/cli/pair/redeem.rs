//! `famp pair redeem` — read a five-word code from stdin, POST it to the
//! inviter's gateway, and pin the inviter's key into this machine's own
//! peer keyring on a verified response (PAIR-01).
//!
//! PAIR-06 (T-18-05) is a STRUCTURAL property of [`PairRedeemArgs`], not a
//! runtime check: the struct declares no field capable of carrying the
//! code and no positional argument at all. `famp pair redeem <anything>`
//! is rejected by clap as an unexpected argument BEFORE any code is ever
//! read — the only path a code enters this process is the `reader`
//! parameter [`run_at`] reads one line from, which `run` binds to stdin.

use std::io::BufRead;
use std::path::Path;

use famp_core::Principal;
use famp_crypto::{key_id, FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::own_domain;
use crate::cli::peer::identity::{
    gateway_identity_path, gateway_peers_keyring_path, load_or_generate,
};
use crate::pairing::wordlist::parse_code;
use crate::pairing::{
    PairingError, RedemptionReject, RedemptionRequest, RedemptionResponse, Signed,
};

/// CLI args for `famp pair redeem`. Deliberately carries NO field for the
/// code — see this module's doc comment (PAIR-06).
#[derive(clap::Args, Debug)]
pub struct PairRedeemArgs {
    /// Base URL of the inviter's gateway pairing route (from the invite
    /// artifact), e.g. `https://gateway.inviter.test:8443`.
    #[arg(long)]
    pub from: String,
    /// Optional pinned CA/leaf certificate for the outbound TLS
    /// connection — same flag shape as `famp-gateway --trust-cert`.
    #[arg(long)]
    pub trust_cert: Option<std::path::PathBuf>,
}

/// Production entry point.
///
/// Not `Send`: `run_at`'s `reader` parameter is bound here to
/// `std::io::StdinLock`, which wraps a `std::sync::MutexGuard` —
/// deliberately `!Send`. This is benign: `block_on_async`
/// (`crates/famp/src/cli/mod.rs`) calls `tokio::Runtime::block_on`, which
/// polls this future exclusively on the calling thread and never spawns
/// it onto tokio's work-stealing pool, so the guard never needs to cross
/// a thread boundary while held across the `.await` below.
#[allow(clippy::future_not_send)]
pub async fn run(args: &PairRedeemArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = super::now_canonical_utc();
    let client = build_client(args.trust_cert.as_deref())?;
    eprintln!("Enter the five-word pairing code (from the inviter), then press Enter:");
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    run_at(&home_path, args, &mut reader, &client, &now).await
}

/// Test-facing entry point: explicit `&Path` + reader + client + `now`.
///
/// `reader` is ALWAYS bound to stdin in [`run`] — never a `source`
/// file-or-stdin toggle like `peer import`/`rotate` — matching PAIR-06.
///
/// Not `Send`: `reader: &mut dyn BufRead` may be bound to
/// `std::io::StdinLock` (a `!Send` `std::sync::MutexGuard`) by [`run`].
/// See [`run`]'s doc comment for why holding it across the `.await` below
/// is benign here.
#[allow(clippy::future_not_send)]
pub async fn run_at(
    home: &Path,
    args: &PairRedeemArgs,
    reader: &mut dyn BufRead,
    client: &reqwest::Client,
    now: &str,
) -> Result<(), CliError> {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;
    let code = parse_code(line.trim())
        .map_err(|e| CliError::Generic(format!("pairing code rejected: {e}")))?;

    let identity_path = gateway_identity_path(home);
    let sk: FampSigningKey = load_or_generate(&identity_path)?;
    let vk = sk.verifying_key();

    // The redeemer's own principal: this gateway's own identity,
    // `agent:<own_domain>/gateway` — the same fixed-name convention
    // `famp peer export`/`import` already establish for a machine's own
    // gateway-level trust (see `peer_roundtrip.rs`/`peer_rotate_cli.rs`
    // fixtures, both of which use `agent:<host>/gateway`).
    let own_domain = own_domain::resolve_own_domain(None, home)?;
    let own_principal: Principal = format!("agent:{own_domain}/gateway").parse().map_err(|e| {
        CliError::Generic(format!(
            "resolved own-domain '{own_domain}' does not form a valid principal: {e}"
        ))
    })?;

    let request = RedemptionRequest {
        code: code.as_str().to_string(),
        principal: own_principal.to_string(),
        pubkey_b64url: vk.to_b64url(),
        nonce: uuid::Uuid::now_v7().to_string(),
    };
    // Proof of possession (T-18-04): signed with the SAME fresh key being
    // proposed for pinning, never a second key.
    let signed_request = Signed::new(request, &sk).map_err(pairing_err_to_cli)?;

    let url = format!("{}/famp/v1/pair/redeem", args.from.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&signed_request)
        .send()
        .await
        .map_err(|e| {
            CliError::Generic(format!("pairing redemption request to {url} failed: {e}"))
        })?;

    if !resp.status().is_success() {
        let reject: RedemptionReject = resp.json().await.unwrap_or_else(|_| RedemptionReject {
            reason: "unknown".to_string(),
        });
        return Err(CliError::Generic(format!(
            "pairing redemption rejected: {}",
            reject.reason
        )));
    }

    let signed_response: Signed<RedemptionResponse> = resp
        .json()
        .await
        .map_err(|e| CliError::Generic(format!("malformed pairing redemption response: {e}")))?;

    // Verify the response's signature against the pubkey it CARRIES
    // (proof of possession, T-18-04) — never trust the transport alone.
    let inviter_vk =
        TrustedVerifyingKey::from_b64url(&signed_response.statement.inviter_pubkey_b64url)
            .map_err(|e| CliError::Generic(format!("malformed inviter pubkey in response: {e}")))?;
    signed_response
        .verify(&inviter_vk)
        .map_err(pairing_err_to_cli)?;

    let inviter_principal: Principal = signed_response
        .statement
        .inviter_principal
        .parse()
        .map_err(|e| {
            CliError::Generic(format!(
                "invalid inviter principal in response '{}': {e}",
                signed_response.statement.inviter_principal
            ))
        })?;

    let keyring_path = gateway_peers_keyring_path(home);
    let mut keyring = if keyring_path.exists() {
        Keyring::load_from_file(&keyring_path).map_err(|e| {
            CliError::Generic(format!(
                "failed to load peer keyring at {}: {e}",
                keyring_path.display()
            ))
        })?
    } else {
        Keyring::new()
    };
    let inviter_key_id = key_id(&inviter_vk);
    keyring
        .rotate_to(inviter_principal.clone(), inviter_vk, now, None, true)
        .map_err(|e| CliError::Generic(e.to_string()))?;
    if let Some(parent) = keyring_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    keyring.save_to_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    eprintln!(
        "Pinned {inviter_principal} (key_id {inviter_key_id}) as Active. The inviter's own \
         side pins YOU only after they run `famp pair status`."
    );
    eprintln!(
        "NOTE: famp-gateway loads its keyring once at startup and will keep honoring the \
         previous trust set until it restarts. Run `famp daemon restart` (or manually \
         restart the gateway if it is not daemon-managed) to pick up the newly pinned key."
    );

    Ok(())
}

/// Build the TLS-preconfigured outbound `reqwest::Client`, reusing
/// `famp_transport_http`'s own `build_client_config` — the SAME
/// construction `famp-gateway`'s `relay_fetch.rs` uses internally —
/// rather than assembling a second rustls client stack.
fn build_client(trust_cert: Option<&Path>) -> Result<reqwest::Client, CliError> {
    let tls = famp_transport_http::tls::build_client_config(trust_cert)
        .map_err(|e| CliError::Generic(format!("failed to build outbound TLS config: {e}")))?;
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::Generic(format!("failed to build outbound HTTPS client: {e}")))
}

// Takes `PairingError` by value so it can be passed as a bare fn pointer
// to `.map_err(pairing_err_to_cli)`, matching the precedent at
// `crates/famp/src/cli/install/grok.rs:42`.
#[allow(clippy::needless_pass_by_value)]
fn pairing_err_to_cli(e: PairingError) -> CliError {
    CliError::Generic(e.to_string())
}
