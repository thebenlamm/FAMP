//! TLS helpers — rustls 0.23 only. NEVER OpenSSL, NEVER native-tls (D-B8, D-F4).
//!
//! D-B5: server loads PEM cert + private key from disk; client uses
//! [`rustls_platform_verifier::Verifier::new_with_extra_roots`] to trust the
//! OS root store **plus** an explicit `--trust-cert` anchor.
//!
//! Crypto provider: `aws-lc-rs`. The plan originally proposed `ring`, but the
//! workspace dep graph (rustls 0.23 pulled with the `aws_lc_rs` feature via
//! reqwest 0.13.2 → rustls-platform-verifier) does not include `ring` at all —
//! aws-lc-rs is what's actually compiled in. Switching to ring would force a
//! second crypto provider into the graph for no benefit. See SUMMARY for the
//! full deviation note.

use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ClientConfig, ServerConfig,
};
use rustls_platform_verifier::Verifier;

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("io error reading PEM: {0}")]
    Io(#[from] std::io::Error),
    #[error("no private key found in PEM file")]
    NoPrivateKey,
    #[error("no certificates found in PEM file: {0}")]
    NoCertificatesInPem(std::path::PathBuf),
    #[error("rustls error: {0}")]
    Rustls(#[from] rustls::Error),
    #[error("platform verifier error: {0}")]
    Verifier(String),
}

/// Install the default rustls crypto provider (aws-lc-rs) if no provider is
/// already installed for the current process. Idempotent: a second call (or
/// a call from another module that already installed one) is a no-op — the
/// `Result` returned by `install_default` is ignored intentionally.
fn install_default_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

/// Load all certificates from a PEM file at `path`.
///
/// Returns [`TlsError::NoCertificatesInPem`] if the file parses but yields
/// zero certificates. `rustls_pemfile::certs` treats non-PEM input as "no
/// items" (an empty iterator) rather than an error; surfacing that as a
/// distinct typed error prevents a typo'd `--trust-cert` path from silently
/// degrading to the OS-roots-only code path (MED-01).
pub fn load_pem_cert(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsError> {
    let mut rd = BufReader::new(File::open(path)?);
    let out: Vec<_> = rustls_pemfile::certs(&mut rd).collect::<Result<_, _>>()?;
    if out.is_empty() {
        return Err(TlsError::NoCertificatesInPem(path.to_path_buf()));
    }
    Ok(out)
}

/// Load the first supported private key (PKCS8 / RSA / SEC1) from a PEM file.
pub fn load_pem_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsError> {
    let mut rd = BufReader::new(File::open(path)?);
    rustls_pemfile::private_key(&mut rd)?.ok_or(TlsError::NoPrivateKey)
}

/// Build a server-side rustls `ServerConfig` from a cert chain + key. Installs
/// the default crypto provider if none is set yet.
pub fn build_server_config(
    cert: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> Result<ServerConfig, TlsError> {
    install_default_provider();
    Ok(ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)?)
}

/// D-B5 full combination: OS root store + explicit extra trust anchor via
/// [`rustls_platform_verifier::Verifier::new_with_extra_roots`].
///
/// * `Some(path)` — adds the PEM(s) at `path` as additional trust anchors on
///   top of the OS roots (useful for self-signed dev certs).
/// * `None` — trust only the OS root store.
pub fn build_client_config(trust_cert_path: Option<&Path>) -> Result<ClientConfig, TlsError> {
    install_default_provider();

    let extra_roots: Vec<CertificateDer<'static>> = match trust_cert_path {
        Some(p) => load_pem_cert(p)?,
        None => Vec::new(),
    };

    let verifier = Verifier::new_with_extra_roots(extra_roots)
        .map_err(|e| TlsError::Verifier(format!("{e:?}")))?;

    Ok(ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Self-signed dev cert + private key (PKCS#8) generated once with rcgen
    /// for `localhost` / `127.0.0.1`. Embedded so the unit tests do not need a
    /// fixture file checked into the repo (those land in Plan 04-04 alongside
    /// the example binary). The cert is byte-identical across test runs.
    const TEST_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIBVzCB/qADAgECAghjs+sqkj/HujAFBgMrZXAwITEfMB0GA1UEAwwWcmNnZW4g
c2VsZiBzaWduZWQgY2VydDAgFw03NTAxMDEwMDAwMDBaGA80MDk2MDEwMTAwMDAw
MFowITEfMB0GA1UEAwwWcmNnZW4gc2VsZiBzaWduZWQgY2VydDAqMAUGAytlcAMh
AHWELa+sjwH/v9oUZWyjUiClHvrIVWTGrtWy/JpAGs2do2cwZTAjBgNVHREEHDAa
gglsb2NhbGhvc3SHBH8AAAGHEAAAAAAAAAAAAAAAAAAAAAEwHQYDVR0OBBYEFEnW
PnsbpDHMqqPlFcZdvShEeERTMB8GA1UdIwQYMBaAFEnWPnsbpDHMqqPlFcZdvShE
eERTMAUGAytlcANBAEFTuc/MOK0LXEhE3xlcOfXKEa/G2x2Pid6kqUTSxBzdnz4U
nZfkw0BBSUI0VBVrYTjpoFMrygJTvMtT5xsP4w8=
-----END CERTIFICATE-----
";

    /// Helper: write a string to a tempfile-style path under `std::env::temp_dir`.
    fn write_tmp(name: &str, contents: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("famp-tls-{}-{name}", std::process::id()));
        let mut f = File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn load_pem_cert_parses_self_signed() {
        let path = write_tmp("cert.pem", TEST_CERT_PEM);
        let certs = load_pem_cert(&path).expect("parse self-signed cert");
        assert_eq!(certs.len(), 1, "exactly one cert in fixture");
    }

    #[test]
    fn load_pem_cert_rejects_garbage() {
        let path = write_tmp("garbage.pem", "this is not a pem file\n");
        // `rustls_pemfile::certs` returns an empty iterator on garbage rather
        // than an error. MED-01: we surface that as a distinct typed error so
        // a typo'd `--trust-cert` path fails loudly instead of silently
        // degrading `build_client_config` to the OS-roots-only code path.
        match load_pem_cert(&path) {
            Err(TlsError::NoCertificatesInPem(p)) => assert_eq!(p, path),
            other => panic!("expected NoCertificatesInPem, got {other:?}"),
        }
    }

    #[test]
    fn load_pem_key_missing_file_is_io_error() {
        let bogus = std::env::temp_dir().join("famp-tls-does-not-exist.pem");
        let _ = std::fs::remove_file(&bogus);
        match load_pem_key(&bogus) {
            Err(TlsError::Io(_)) => {}
            other => panic!("expected Io error, got {other:?}"),
        }
    }

    #[test]
    fn load_pem_key_no_key_in_file_is_typed_error() {
        // Cert-only file → no private key → NoPrivateKey.
        let path = write_tmp("cert-only.pem", TEST_CERT_PEM);
        match load_pem_key(&path) {
            Err(TlsError::NoPrivateKey) => {}
            other => panic!("expected NoPrivateKey, got {other:?}"),
        }
    }

    #[test]
    fn build_client_config_without_extra_root_succeeds() {
        // OS-roots-only path. Should yield a working ClientConfig (no extra
        // anchors loaded). Verifier installs the default crypto provider as a
        // side effect — running this test before others ensures the rest of
        // the suite can rely on a provider being present.
        let _client = build_client_config(None).expect("client config builds");
    }

    #[test]
    fn build_client_config_with_explicit_anchor_succeeds() {
        let path = write_tmp("anchor.pem", TEST_CERT_PEM);
        let _client = build_client_config(Some(&path)).expect("client config with anchor");
    }
}
