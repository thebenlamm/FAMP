//! TOFU-pinning HTTPS client for `famp send`.
//!
//! Uses a custom `rustls::client::danger::ServerCertVerifier` that:
//! - on first contact (`pinned = None`): records the leaf cert SHA-256
//!   into a shared `Mutex<Option<String>>`, accepts the cert;
//! - on subsequent contacts (`pinned = Some(hex)`): computes the leaf
//!   SHA-256 and returns `Err` on mismatch, surfacing as
//!   [`CliError::TlsFingerprintMismatch`] after the client builder error
//!   is translated.
//!
//! After a successful first send, the caller persists the captured
//! fingerprint back into `peers.toml` via `write_peers_atomic`.
//!
//! The sig/host verification methods are stubbed (`Ok`) because the
//! fingerprint IS the trust anchor — we're pinning the leaf bytes
//! verbatim, not negotiating an X.509 chain.

use std::sync::{Arc, Mutex};

use reqwest::tls::Version;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::cli::error::CliError;

#[derive(Debug)]
pub struct TofuVerifier {
    pinned: Option<String>,
    captured: Arc<Mutex<Option<String>>>,
}

impl TofuVerifier {
    pub const fn new(pinned: Option<String>, captured: Arc<Mutex<Option<String>>>) -> Self {
        Self { pinned, captured }
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
                return Err(rustls::Error::General(format!(
                    "famp-tofu-mismatch:{pinned}:{got}"
                )));
            }
        } else if let Ok(mut guard) = self.captured.lock() {
            *guard = Some(got);
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

pub struct SendOutcome {
    pub captured_fingerprint: Option<String>,
}

fn install_default_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

/// POST `bytes` to `<endpoint>/famp/v0.5.1/inbox/<recipient_principal>`.
///
/// Uses a TOFU-pinning TLS client. Returns the captured leaf fingerprint on
/// first contact (so the caller can persist it), or `None` if it was already
/// pinned.
pub async fn post_envelope(
    endpoint: &str,
    recipient_principal: &str,
    bytes: Vec<u8>,
    pinned: Option<String>,
    alias: &str,
) -> Result<SendOutcome, CliError> {
    install_default_provider();

    let had_pin = pinned.is_some();
    let pinned_clone = pinned.clone();
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let verifier = Arc::new(TofuVerifier::new(pinned_clone, captured.clone()));

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
            // Translate a TOFU-mismatch rustls error into a typed CliError.
            if had_pin {
                let msg = format!("{e:#}");
                if let Some(mismatch) = msg.find("famp-tofu-mismatch:") {
                    let tail = &msg[mismatch + "famp-tofu-mismatch:".len()..];
                    if let Some((pinned_s, rest)) = tail.split_once(':') {
                        let got: String =
                            rest.chars().take_while(char::is_ascii_hexdigit).collect();
                        return Err(CliError::TlsFingerprintMismatch {
                            alias: alias.to_string(),
                            pinned: pinned_s.to_string(),
                            got,
                        });
                    }
                }
            }
            return Err(CliError::SendFailed(Box::new(e)));
        }
    };

    if !resp.status().is_success() {
        return Err(CliError::SendFailed(Box::new(std::io::Error::other(
            format!("HTTP {}", resp.status()),
        ))));
    }

    let captured = captured.lock().ok().and_then(|g| g.clone());
    Ok(SendOutcome {
        captured_fingerprint: if had_pin { None } else { captured },
    })
}
