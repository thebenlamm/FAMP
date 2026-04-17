//! Self-signed TLS cert generation for `famp init`.
//!
//! Parameters (RESEARCH `TLS` Cert Parameters):
//! - key algorithm: `PKCS_ECDSA_P256_SHA256` (conservative-compat; v0.7 proven)
//! - SANs: `["localhost", "127.0.0.1", "::1"]`
//! - CN: `"famp-local"`
//! - validity: 397 days (CA/B Forum baseline; Apple enforces 398-day max)
//! - KU: digitalSignature
//! - EKU: serverAuth

use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use time::{Duration, OffsetDateTime};

// Apple's Security framework rejects serverAuth leaves whose validity exceeds
// 398 days with errSecCertificateNotStandardsCompliant (-67901). Stay a day under.
const MAX_VALIDITY_DAYS: i64 = 397;

fn tls_params() -> Result<CertificateParams, rcgen::Error> {
    let mut params = CertificateParams::new(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ])?;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "famp-local");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + Duration::days(MAX_VALIDITY_DAYS);

    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);

    Ok(params)
}

/// Returns `(cert_pem, key_pem)` — a self-signed ECDSA P-256 cert for local use.
///
/// Valid for 397 days, covering `localhost`, `127.0.0.1`, and `::1`, with the
/// digitalSignature `KeyUsage` and serverAuth `ExtendedKeyUsage` bits set so
/// macOS (and other strict verifiers) accept it as a TLS server leaf.
pub fn generate_tls() -> Result<(String, String), rcgen::Error> {
    let params = tls_params()?;
    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Apple's Security framework (macOS 10.15+, iOS 13+) rejects any TLS
    /// serverAuth leaf whose validity exceeds 398 days with
    /// `errSecCertificateNotStandardsCompliant` (-67901). The policy mirrors
    /// the CA/B Forum baseline effective 2020-09-01. FAMP issues 397-day
    /// certs to stay one day under the limit while remaining renewable
    /// annually. Regression guard for the onboarding TLS path.
    #[test]
    fn tls_params_within_apple_398_day_limit() {
        let params = tls_params().expect("tls_params");
        let validity = params.not_after - params.not_before;
        assert!(
            validity.whole_days() <= 397,
            "validity {} days exceeds Apple 398-day serverAuth limit",
            validity.whole_days()
        );
    }

    /// macOS's platform verifier requires serverAuth in `ExtendedKeyUsage`
    /// for any cert presented as a TLS server leaf. Without it, validation
    /// fails even on otherwise-valid self-signed certs.
    #[test]
    fn tls_params_declares_server_auth_eku() {
        let params = tls_params().expect("tls_params");
        assert!(
            params
                .extended_key_usages
                .contains(&ExtendedKeyUsagePurpose::ServerAuth),
            "ExtendedKeyUsage must include ServerAuth"
        );
    }

    /// Apple and strict RFC 5280 verifiers expect TLS server leaves to
    /// declare the digitalSignature `KeyUsage` bit explicitly.
    #[test]
    fn tls_params_declares_digital_signature_ku() {
        let params = tls_params().expect("tls_params");
        assert!(
            params
                .key_usages
                .contains(&KeyUsagePurpose::DigitalSignature),
            "KeyUsage must include DigitalSignature"
        );
    }

    #[test]
    fn generate_tls_returns_two_nonempty_pems() {
        let (cert, key) = generate_tls().expect("generate_tls");
        assert!(cert.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(key.contains("PRIVATE KEY"));
        assert!(!cert.is_empty() && !key.is_empty());
    }

    /// Cross-phase conformance gate: output must load via the Phase 2
    /// (`famp-transport-http`) PEM loaders byte-for-byte
    /// (RESEARCH Open Question #2).
    #[test]
    fn generated_pems_load_via_transport_http_loader() {
        use std::io::Write;
        let (cert, key) = generate_tls().expect("generate_tls");

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let cert_path = tmp.path().join("tls.cert.pem");
        let key_path = tmp.path().join("tls.key.pem");
        std::fs::File::create(&cert_path)
            .and_then(|mut f| f.write_all(cert.as_bytes()))
            .expect("write cert");
        std::fs::File::create(&key_path)
            .and_then(|mut f| f.write_all(key.as_bytes()))
            .expect("write key");

        let certs = famp_transport_http::tls::load_pem_cert(&cert_path).expect("load_pem_cert");
        let k = famp_transport_http::tls::load_pem_key(&key_path).expect("load_pem_key");
        let _cfg =
            famp_transport_http::tls::build_server_config(certs, k).expect("build_server_config");
    }
}
