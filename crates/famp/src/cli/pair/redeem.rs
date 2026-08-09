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
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::own_domain;
use crate::cli::peer::identity::{
    gateway_identity_path, gateway_peers_keyring_path, load_or_generate,
};
use crate::pairing::wordlist::parse_code;
use crate::pairing::{
    reject_reason_to_pairing_error, PairingError, RedemptionReject, RedemptionRequest,
    RedemptionResponse, Signed,
};

/// CLI args for `famp pair redeem`. Deliberately carries NO field for the
/// code — see this module's doc comment (PAIR-06).
#[derive(clap::Args, Debug)]
pub struct PairRedeemArgs {
    /// Base URL of the inviter's gateway pairing route (from the invite
    /// artifact), e.g. `https://gateway.inviter.test:8443`.
    #[arg(long)]
    pub from: String,
    /// Required: the identity this machine will send as (a bare leaf like
    /// `alice`, or a full `agent:<authority>/<name>` principal whose
    /// authority must equal this machine's own-domain). Pinned under this
    /// exact value so it matches the `from` `famp send` later builds for
    /// the SAME identity (`send/mod.rs:679`) — see this module's doc
    /// comment. Required, not optional-with-default: defaulting to a fixed
    /// name would silently preserve the mismatch for anyone who omits it.
    #[arg(long = "as")]
    pub as_identity: String,
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
    // Client-side check BEFORE any network call (PAIR-05): a malformed
    // code is rejected with the fixed, jargon-free message from Task 1,
    // never touching the network.
    let code = parse_code(line.trim()).map_err(pairing_err_to_cli)?;

    let identity_path = gateway_identity_path(home);
    let sk: FampSigningKey = load_or_generate(&identity_path)?;
    let vk = sk.verifying_key();

    // The redeemer's own principal: it MUST equal the `from` principal
    // `famp send`'s (private) `build_remote_envelope_value` builds for the
    // SAME identity (`send/mod.rs:679`: `agent:{own_domain}/{identity}`),
    // or the inviter pins a principal no envelope this redeemer later
    // sends will ever carry, and every follower→inviter message is
    // rejected `UnpinnedKey`. `--as` (D2) supplies that identity — see
    // this module's doc comment.
    let own_domain = own_domain::resolve_own_domain(None, home)?;
    let own_principal = resolve_as_principal(&args.as_identity, &own_domain)?;

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
    // A transport-level failure (DNS, connection refused, TLS handshake,
    // timeout) is not an HTTP rejection from the endpoint — map it to the
    // unreachable message with the URL interpolated (PAIR-05), never the
    // raw transport error text.
    let resp = client
        .post(&url)
        .json(&signed_request)
        .send()
        .await
        .map_err(|_e| pairing_err_to_cli(PairingError::GatewayUnreachable { url: url.clone() }))?;

    if !resp.status().is_success() {
        let reject: RedemptionReject = resp.json().await.unwrap_or_else(|_| RedemptionReject {
            reason: "unknown".to_string(),
        });
        // Five HTTP reject reasons map 1:1 onto five of the seven
        // redeemer-facing messages via the shared `reject_reason_to_pairing_error`
        // table (Task 1). The wire's `RedemptionReject` carries no
        // remaining-tries field today, so the wrong-code message is used
        // unchanged rather than interpolating a number this endpoint
        // never reports (PAIR-05: never invent a number).
        return Err(pairing_err_to_cli(reject_reason_to_pairing_error(
            &reject.reason,
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
    if !super::rotate_to_with_validation(&keyring_path, &inviter_principal, inviter_vk, &now, true)?
    {
        eprintln!(
            "pin did not verify on reload at {} — pin failed. \
             The inviter's key was not saved to the keyring.",
            keyring_path.display()
        );
        return Err(CliError::Exit(1));
    }

    // One-sentence done-signal (PAIR-07: "not FSM JSON", naming the peer
    // in plain words, nothing else is needed on this side).
    eprintln!("Paired with {inviter_principal}; nothing else is needed on this side.");
    eprintln!(
        "NOTE: famp-gateway loads its keyring once at startup and will keep honoring the \
         previous key until it restarts. Run `famp daemon restart` (or manually restart \
         the gateway if it is not daemon-managed) to pick up the newly pinned key."
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

/// Resolve `--as` into the principal this redeemer pins itself under (D2).
///
/// A bare leaf (e.g. `alice`, no `agent:` scheme) is combined with the
/// resolved own-domain: `agent:{own_domain}/{as_identity}`. A full
/// `agent:<authority>/<name>` principal is accepted as-is ONLY if its
/// authority byte-equals `own_domain` — a different authority would pin a
/// principal `famp send` never builds `from` for on this machine, so it is
/// rejected with a `CliError` naming both values rather than silently
/// honored. Both forms are accepted deliberately: the invite artifact
/// shows the bare-leaf form, but a follower may instead paste the full
/// principal from the inviter's own `--as` example.
fn resolve_as_principal(as_identity: &str, own_domain: &str) -> Result<Principal, CliError> {
    if let Ok(full) = as_identity.parse::<Principal>() {
        if full.authority() == own_domain {
            return Ok(full);
        }
        return Err(CliError::Generic(format!(
            "--as '{as_identity}' has authority '{}' but this machine's own-domain is \
             '{own_domain}'; famp send will build envelopes from agent:{own_domain}/<name>, so a \
             pin under a different authority would never match",
            full.authority()
        )));
    }

    format!("agent:{own_domain}/{as_identity}")
        .parse()
        .map_err(|e| CliError::Generic(format!("invalid --as identity '{as_identity}': {e}")))
}
