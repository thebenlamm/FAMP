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
MIIBZDCCAQqgAwIBAgIUZIUy3qsk6H3ZnppqnHH7KAvwxkEwCgYIKoZIzj0EAwIw
ITEfMB0GA1UEAwwWcmNnZW4gc2VsZiBzaWduZWQgY2VydDAgFw03NTAxMDEwMDAw
MDBaGA80MDk2MDEwMTAwMDAwMFowITEfMB0GA1UEAwwWcmNnZW4gc2VsZiBzaWdu
ZWQgY2VydDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABED/G6Kz8YMC43+XC9G/
oSeAF5q2dA/pfUaOjbICPUmq2L5MrQmiRFiJO89D1X0OI3YG3GzLWK/mUlZtUr8d
XoejHjAcMBoGA1UdEQQTMBGCCWxvY2FsaG9zdIcEfwAAATAKBggqhkjOPQQDAgNI
ADBFAiAS4ZwgkV9tm9Hs38qs3GAHTNiTC19zXvKqKNJLC84HlQIhALzNwjcawzDV
d5Fgf3wO4+uOXDA7I3TNOOcbwtphm53S
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

/// D-08 falsification control (RESEARCH Open Q2): a dedicated system-trust
/// (real CA -> leaf delegation) TLS test, isolated from the E2E's
/// `--trust-cert` config.
///
/// **Why a separate config is needed:** the E2E (`e2e_cross_host_delivery.rs`)
/// and `docs/GATEWAY-SETUP.md` both use `--trust-cert <peer's own leaf cert>`
/// — i.e. the peer's self-signed leaf is added DIRECTLY as an extra trust
/// anchor via [`Verifier::new_with_extra_roots`], and that SAME cert is what
/// the peer presents as its server cert. Running the actual E2E on macOS
/// with the pre-regen fixtures (ECDSA, no `extendedKeyUsage`) on 2026-07-28
/// PASSED — confirming Open Q2's hypothesis that this "leaf pinned directly
/// as its own anchor" shortcut does not enforce Apple SecTrust's normal
/// leaf-EKU policy the way a real CA-delegation chain does. A post-regen
/// green under that same config would therefore prove nothing about EKU
/// enforcement specifically (both old and new fixtures pass it identically).
///
/// This test instead builds a genuine two-cert chain — a self-signed CA
/// (added as the ONLY extra root) that ISSUES two different leaves — so a
/// live TLS handshake goes through real chain validation instead of the
/// leaf-pinning shortcut. `#[cfg(target_os = "macos")]` because this is
/// specifically pinning Apple SecTrust's EKU divergence (finding #5);
/// webpki (Linux) tolerates a missing EKU, so the must-fail pole would not
/// hold there and is out of scope for this control.
#[cfg(all(test, target_os = "macos"))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod system_trust_eku_control {
    use std::{net::SocketAddr, path::PathBuf, process::Command, sync::Arc};

    use axum::{routing::get, Router};

    struct GeneratedChain {
        dir: PathBuf,
    }

    impl GeneratedChain {
        fn path(&self, name: &str) -> PathBuf {
            self.dir.join(name)
        }
    }

    impl Drop for GeneratedChain {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn run_openssl(args: &[&str]) {
        let out = Command::new("openssl")
            .args(args)
            .output()
            .expect("openssl CLI must be available (RESEARCH Environment Availability)");
        assert!(
            out.status.success(),
            "openssl {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Generate a self-signed CA plus two leaves it issues: one with NO
    /// `extendedKeyUsage`, one with `extendedKeyUsage=serverAuth` (the D-08
    /// canonical recipe). Both leaves share the same loopback SANs.
    fn gen_ca_and_leaves() -> GeneratedChain {
        let dir = std::env::temp_dir().join(format!(
            "famp-system-trust-eku-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let chain = GeneratedChain { dir };

        let ca_key = chain.path("ca.key");
        let ca_crt = chain.path("ca.crt");
        run_openssl(&[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "800",
            "-keyout",
            ca_key.to_str().unwrap(),
            "-out",
            ca_crt.to_str().unwrap(),
            "-subj",
            "/CN=test-ca",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ]);

        for (stub, eku_ext) in [("leaf_no_eku", None), ("leaf_eku", Some("serverAuth"))] {
            let key = chain.path(&format!("{stub}.key"));
            let csr = chain.path(&format!("{stub}.csr"));
            let crt = chain.path(&format!("{stub}.crt"));
            let extfile = chain.path(&format!("{stub}.ext"));

            run_openssl(&[
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                key.to_str().unwrap(),
                "-out",
                csr.to_str().unwrap(),
                "-subj",
                "/CN=localhost",
            ]);

            let mut ext_contents = String::from(
                "basicConstraints=critical,CA:FALSE\n\
                 subjectAltName=IP:127.0.0.1,DNS:localhost\n\
                 keyUsage=critical,digitalSignature,keyEncipherment\n",
            );
            if let Some(eku) = eku_ext {
                use std::fmt::Write as _;
                let _ = writeln!(ext_contents, "extendedKeyUsage={eku}");
            }
            std::fs::write(&extfile, ext_contents).unwrap();

            run_openssl(&[
                "x509",
                "-req",
                "-in",
                csr.to_str().unwrap(),
                "-CA",
                ca_crt.to_str().unwrap(),
                "-CAkey",
                ca_key.to_str().unwrap(),
                "-CAcreateserial",
                "-days",
                "800",
                "-out",
                crt.to_str().unwrap(),
                "-extfile",
                extfile.to_str().unwrap(),
            ]);
        }

        chain
    }

    /// Spin up a bare HTTPS server presenting `leaf_crt`/`leaf_key`, build a
    /// system-trust client whose ONLY extra root is `ca_crt` (never the leaf
    /// itself), and attempt one GET. Returns `Err` on any TLS/connect/status
    /// failure.
    async fn try_tls_get(
        ca_crt: &std::path::Path,
        leaf_crt: &std::path::Path,
        leaf_key: &std::path::Path,
    ) -> Result<(), String> {
        let cert = super::load_pem_cert(leaf_crt).map_err(|e| e.to_string())?;
        let key = super::load_pem_key(leaf_key).map_err(|e| e.to_string())?;
        let server_cfg = super::build_server_config(cert, key).map_err(|e| e.to_string())?;

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        std_listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let addr: SocketAddr = std_listener.local_addr().map_err(|e| e.to_string())?;

        let router = Router::new().route("/", get(|| async { "ok" }));
        let handle =
            crate::tls_server::serve_std_listener(std_listener, router, Arc::new(server_cfg));

        // Bounded wait for the listener to actually accept, no fixed sleep
        // on the steady-state path.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if std::net::TcpStream::connect(addr).is_ok() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                handle.abort();
                return Err("server never accepted a TCP connection".to_string());
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        let client_cfg = super::build_client_config(Some(ca_crt)).map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(client_cfg)
            .timeout(std::time::Duration::from_secs(5))
            .http1_only()
            .build()
            .map_err(|e| e.to_string())?;

        let result = client
            .get(format!("https://localhost:{}/", addr.port()))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string());

        handle.abort();
        result
    }

    /// Falsification control: with a real CA -> leaf chain (not the
    /// leaf-pinned-as-its-own-anchor shortcut), Apple SecTrust rejects a
    /// leaf with no `extendedKeyUsage` (must-fail pole) and accepts the
    /// D-08 canonical recipe's `extendedKeyUsage=serverAuth` leaf (must-pass
    /// pole). Green on both poles would mean the control carries zero
    /// information; this test names each pole explicitly so that can never
    /// happen silently.
    #[tokio::test]
    async fn ca_delegated_leaf_enforces_eku_on_apple_sectrust() {
        let chain = gen_ca_and_leaves();
        let ca_crt = chain.path("ca.crt");

        let no_eku = try_tls_get(
            &ca_crt,
            &chain.path("leaf_no_eku.crt"),
            &chain.path("leaf_no_eku.key"),
        )
        .await;
        assert!(
            no_eku.is_err(),
            "MUST-FAIL pole: a CA-delegated leaf with no extendedKeyUsage must be \
             REJECTED by Apple SecTrust under real chain validation; got {no_eku:?}"
        );

        let with_eku = try_tls_get(
            &ca_crt,
            &chain.path("leaf_eku.crt"),
            &chain.path("leaf_eku.key"),
        )
        .await;
        assert!(
            with_eku.is_ok(),
            "MUST-PASS pole: a CA-delegated leaf with extendedKeyUsage=serverAuth \
             (D-08 canonical recipe) must be ACCEPTED; got {with_eku:?}"
        );
    }
}
