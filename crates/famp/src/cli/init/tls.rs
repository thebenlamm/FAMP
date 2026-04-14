//! Self-signed TLS cert generation for `famp init`.
//!
//! Parameters (RESEARCH `TLS` Cert Parameters):
//! - key algorithm: `PKCS_ECDSA_P256_SHA256` (conservative-compat; v0.7 proven)
//! - SANs: `["localhost", "127.0.0.1", "::1"]`
//! - CN: `"famp-local"`
//! - validity: 3650 days

use rcgen::{
    CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256,
};
use time::{Duration, OffsetDateTime};

/// Returns `(cert_pem, key_pem)` — a self-signed ECDSA P-256 cert valid for
/// ten years, covering `localhost`, `127.0.0.1`, and `::1`.
pub fn generate_tls() -> Result<(String, String), rcgen::Error> {
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
    params.not_after = now + Duration::days(3650);

    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

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

        let certs =
            famp_transport_http::tls::load_pem_cert(&cert_path).expect("load_pem_cert");
        let k = famp_transport_http::tls::load_pem_key(&key_path).expect("load_pem_key");
        let _cfg = famp_transport_http::tls::build_server_config(certs, k)
            .expect("build_server_config");
    }
}
