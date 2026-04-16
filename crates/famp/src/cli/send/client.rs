//! TOFU-pinning HTTPS client for `famp send`.
//!
//! ## Trust model
//!
//! The reference deployment uses self-signed TLS certs (no public CA), so
//! trust is anchored to a SHA-256 hash of the leaf certificate. There are
//! two operating modes:
//!
//! - **Pinned (default).** `peers.toml` carries `tls_fingerprint_sha256` for
//!   the alias. Every connection verifies the live leaf hash against the pin
//!   and rejects on mismatch ([`CliError::TlsFingerprintMismatch`]).
//! - **First-contact bootstrap.** No pin is recorded yet. By default this
//!   mode FAILS CLOSED — silently pinning whatever cert the network returns
//!   is a permanent MITM hazard (a one-time on-path attacker captures the
//!   alias forever). The operator must opt in explicitly by setting the
//!   environment variable `FAMP_TOFU_BOOTSTRAP=1` for the duration of the
//!   first send. When set, the leaf hash is captured and persisted by the
//!   caller via `write_peers_atomic`.
//!
//! ## Handshake signature verification
//!
//! Even when the leaf fingerprint matches a pin, we still validate the
//! TLS handshake signature against the leaf cert's public key. Without that
//! check, an attacker who can present the cert bytes (e.g., from a stale
//! disk image) but does not own the corresponding private key would still
//! be accepted. Signature verification is delegated to the active rustls
//! crypto provider's standard algorithms (`aws-lc-rs`).

use std::sync::{Arc, Mutex};

use reqwest::tls::Version;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::cli::error::CliError;

/// Environment variable that opts the operator into insecure first-contact
/// TOFU pinning. Set to `1` (any other value disables it).
pub const TOFU_BOOTSTRAP_ENV: &str = "FAMP_TOFU_BOOTSTRAP";

/// Outcome captured by the verifier during the TLS handshake. The caller
/// inspects this slot after the reqwest send completes (success or failure)
/// to construct a precisely-typed [`CliError`].
///
/// This replaces the previous sentinel-string-in-rustls-error scheme, which
/// was fragile to upstream changes in how rustls / hyper / reqwest format
/// or wrap errors. The mutex slot is the canonical rustls way to signal
/// from a custom verifier back to the caller.
#[derive(Debug, Clone)]
pub enum VerifierOutcome {
    /// First-contact success: leaf hash captured. Caller must persist into
    /// `peers.toml`.
    Captured(String),
    /// First contact, but operator did not opt in to TOFU bootstrap.
    /// Connection refused; caller surfaces as `CliError::TofuBootstrapRefused`.
    Refused { got: String },
    /// Pinned hash did not match the live leaf. Caller surfaces as
    /// `CliError::TlsFingerprintMismatch`.
    Mismatch { pinned: String, got: String },
}

/// Process-wide test override. When `true`, first-contact TOFU bootstrap is
/// allowed even without `FAMP_TOFU_BOOTSTRAP=1` in the environment. This
/// avoids `unsafe { env::set_var }` in every integration test that does
/// first contact, while keeping the production default fail-closed.
///
/// Hidden from public docs — only test code should touch this.
#[doc(hidden)]
pub static ALLOW_TOFU_BOOTSTRAP_FOR_TESTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Test-only helper to enable first-contact TOFU bootstrap programmatically.
/// Production code should never call this — operators must set
/// `FAMP_TOFU_BOOTSTRAP=1` instead.
#[doc(hidden)]
pub fn allow_tofu_bootstrap_for_tests() {
    ALLOW_TOFU_BOOTSTRAP_FOR_TESTS.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn tofu_bootstrap_opted_in() -> bool {
    if ALLOW_TOFU_BOOTSTRAP_FOR_TESTS.load(std::sync::atomic::Ordering::SeqCst) {
        return true;
    }
    matches!(std::env::var(TOFU_BOOTSTRAP_ENV).as_deref(), Ok("1"))
}

#[derive(Debug)]
pub struct TofuVerifier {
    /// Pinned leaf SHA-256 hex from `peers.toml`. `None` means no prior pin.
    pinned: Option<String>,
    /// Operator opted into first-contact TOFU for this invocation.
    bootstrap_allowed: bool,
    /// Slot the verifier writes to communicate its decision back to the
    /// caller — robust against any upstream changes to how rustls / hyper /
    /// reqwest format or wrap errors.
    outcome: Arc<Mutex<Option<VerifierOutcome>>>,
    /// Active rustls crypto provider — used for real handshake signature
    /// verification (Ed25519/ECDSA/RSA via the underlying provider).
    provider: Arc<CryptoProvider>,
}

impl TofuVerifier {
    pub const fn new(
        pinned: Option<String>,
        bootstrap_allowed: bool,
        outcome: Arc<Mutex<Option<VerifierOutcome>>>,
        provider: Arc<CryptoProvider>,
    ) -> Self {
        Self {
            pinned,
            bootstrap_allowed,
            outcome,
            provider,
        }
    }

    fn record(&self, o: VerifierOutcome) {
        if let Ok(mut guard) = self.outcome.lock() {
            *guard = Some(o);
        }
    }
}

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let mut h = Sha256::new();
        h.update(end_entity.as_ref());
        let got = hex::encode(h.finalize());
        if let Some(pinned) = &self.pinned {
            if pinned != &got {
                self.record(VerifierOutcome::Mismatch {
                    pinned: pinned.clone(),
                    got: got.clone(),
                });
                // The error string here is purely diagnostic — the typed
                // CliError is constructed from the `outcome` slot above.
                return Err(rustls::Error::General(format!(
                    "famp-tofu-mismatch: pinned={pinned} got={got}"
                )));
            }
        } else {
            if !self.bootstrap_allowed {
                // Fail closed. The operator has not opted in to TOFU
                // bootstrap, and there is no pinned fingerprint to
                // verify against. Refusing here prevents a one-time
                // active attacker from permanently hijacking the alias.
                self.record(VerifierOutcome::Refused { got: got.clone() });
                return Err(rustls::Error::General(format!(
                    "famp-bootstrap-refused: got={got}"
                )));
            }
            self.record(VerifierOutcome::Captured(got));
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub struct SendOutcome {
    pub captured_fingerprint: Option<String>,
}

/// Install the default crypto provider once and return a clone of it for
/// the verifier to use for handshake signature checks.
fn install_default_provider() -> Arc<CryptoProvider> {
    let provider = rustls::crypto::aws_lc_rs::default_provider();
    // `install_default` is idempotent in spirit: it returns Err if a
    // provider is already installed, which we ignore. Either way, we
    // hand back the provider value we just constructed for use as the
    // verifier's source of signature algorithms.
    let _ = provider.clone().install_default();
    Arc::new(provider)
}

/// POST `bytes` to `<endpoint>/famp/v0.5.1/inbox/<recipient_principal>`.
///
/// Uses a TOFU-pinning TLS client. Returns the captured leaf fingerprint on
/// first contact (so the caller can persist it), or `None` if it was already
/// pinned. First contact requires `FAMP_TOFU_BOOTSTRAP=1` in the environment;
/// otherwise the connection is refused with [`CliError::TofuBootstrapRefused`].
pub async fn post_envelope(
    endpoint: &str,
    recipient_principal: &str,
    bytes: Vec<u8>,
    pinned: Option<String>,
    alias: &str,
) -> Result<SendOutcome, CliError> {
    let provider = install_default_provider();

    let had_pin = pinned.is_some();
    let bootstrap_allowed = !had_pin && tofu_bootstrap_opted_in();
    let outcome: Arc<Mutex<Option<VerifierOutcome>>> = Arc::new(Mutex::new(None));
    let verifier = Arc::new(TofuVerifier::new(
        pinned,
        bootstrap_allowed,
        outcome.clone(),
        provider,
    ));

    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .min_tls_version(Version::TLS_1_2)
        .http1_only()
        .build()
        .map_err(|e| CliError::SendFailed(Box::new(e)))?;

    // Percent-encode the principal segment via url::Url so `:` / `/` in
    // `agent:localhost/self` become `%3A` / `%2F`.
    let base = format!("{}/", endpoint.trim_end_matches('/'));
    let mut url = url::Url::parse(&base)
        .map_err(|e| CliError::SendFailed(Box::new(std::io::Error::other(e.to_string()))))?;
    {
        let mut segs = url.path_segments_mut().map_err(|()| {
            CliError::SendFailed(Box::new(std::io::Error::other(
                "endpoint has no path segments",
            )))
        })?;
        segs.pop_if_empty();
        segs.extend(["famp", "v0.5.1", "inbox"]);
        segs.push(recipient_principal);
    }

    let resp = match client
        .post(url)
        .header("content-type", "application/famp+json")
        .body(bytes)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // Translate via the verifier's outcome slot — the canonical
            // rustls way to signal from a custom verifier. This avoids any
            // dependence on how rustls / hyper / reqwest format errors,
            // which was the previous fragility (string-scanning the
            // source chain for sentinel tags).
            let captured_outcome = outcome.lock().ok().and_then(|g| g.clone());
            return Err(match captured_outcome {
                Some(VerifierOutcome::Refused { got }) => CliError::TofuBootstrapRefused {
                    alias: alias.to_string(),
                    got,
                },
                Some(VerifierOutcome::Mismatch { pinned, got }) => {
                    CliError::TlsFingerprintMismatch {
                        alias: alias.to_string(),
                        pinned,
                        got,
                    }
                }
                // None or Captured: TLS handshake reached a stage past our
                // verifier without the verifier writing a refusal — must
                // be a transport-level failure (DNS, TCP, post-handshake).
                None | Some(VerifierOutcome::Captured(_)) => CliError::SendFailed(Box::new(e)),
            });
        }
    };

    if !resp.status().is_success() {
        return Err(CliError::SendFailed(Box::new(std::io::Error::other(
            format!("HTTP {}", resp.status()),
        ))));
    }

    // Successful send. The captured fingerprint (if any) was written to
    // the outcome slot during the TLS handshake.
    let captured = match outcome.lock().ok().and_then(|g| g.clone()) {
        Some(VerifierOutcome::Captured(fp)) => Some(fp),
        // Refused / Mismatch can't co-occur with a successful HTTP response.
        // Captured-None is the pinned-success case (no fingerprint to persist).
        _ => None,
    };
    Ok(SendOutcome {
        captured_fingerprint: if had_pin { None } else { captured },
    })
}
