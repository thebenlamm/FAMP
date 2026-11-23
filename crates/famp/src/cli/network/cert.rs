//! Peer-card certificate facts, read from DER (see `famp_transport_http::cert_facts`).
use base64::Engine as _;

pub use famp_transport_http::cert_facts::CertFacts;

/// The single certificate's DER bytes, from PEM text holding exactly one block.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    if pem.matches(BEGIN).count() != 1 || pem.matches(END).count() != 1 {
        return Err("expected exactly one certificate".to_owned());
    }
    let begin_at = pem.find(BEGIN).ok_or("no BEGIN line")?;
    let start = begin_at + BEGIN.len();
    let end = pem.find(END).ok_or("no END line")?;
    if end < start {
        return Err("END before BEGIN".to_owned());
    }
    // Nothing but whitespace may surround the block: another parser (the one that
    // loads the trust file) must see exactly the same single certificate.
    if !pem[..begin_at].trim().is_empty() || !pem[end + END.len()..].trim().is_empty() {
        return Err("text outside the certificate block".to_owned());
    }
    let body: String = pem[start..end]
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|e| format!("certificate is not valid base64: {e}"))
}

/// Parse `pem` and report what it declares.
///
/// # Errors
///
/// A description of the first thing that is not exactly one well-formed X.509 v3
/// certificate with at most one subjectAltName and one `BasicConstraints` extension.
pub fn facts(pem: &str) -> Result<CertFacts, String> {
    famp_transport_http::cert_facts::facts_der(&pem_to_der(pem)?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fmt::Write as _;
    use std::net::IpAddr;

    use super::*;

    /// A certificate made with the openssl CLI; `extra` are `-addext` arguments.
    fn make(extra: &[&str]) -> String {
        let dir = tempfile::tempdir().unwrap();
        let (crt, key) = (dir.path().join("c.pem"), dir.path().join("k.pem"));
        let mut cmd = std::process::Command::new("openssl");
        cmd.args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
        ])
        .args(["-subj", "/CN=IP Address:10.9.9.9", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&crt);
        for e in extra {
            cmd.args(["-addext", e]);
        }
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        std::fs::read_to_string(crt).unwrap()
    }

    #[test]
    fn reads_the_ip_and_the_ca_flag_of_a_setup_style_certificate() {
        let f = facts(&make(&[
            "subjectAltName=IP:100.64.0.7",
            "basicConstraints=critical,CA:FALSE",
        ]))
        .unwrap();
        assert_eq!(f.ips, ["100.64.0.7".parse::<IpAddr>().unwrap()]);
        assert_eq!((f.other_names, f.is_ca), (0, Some(false)));
    }

    #[test]
    fn reports_ca_true_absent_constraints_extra_names_and_ipv6() {
        let ca = facts(&make(&[
            "subjectAltName=IP:100.64.0.7",
            "basicConstraints=critical,CA:TRUE",
        ]))
        .unwrap();
        assert_eq!(ca.is_ca, Some(true));
        // Whatever the local openssl defaults to without an explicit constraint, the
        // reader must report it faithfully and not guess.
        let bare = facts(&make(&["subjectAltName=IP:100.64.0.7"])).unwrap();
        assert_eq!(bare.ips.len(), 1);
        let mixed = facts(&make(&[
            "subjectAltName=IP:100.64.0.7,IP:fd7a:115c:a1e0::1,DNS:evil.test",
            "basicConstraints=critical,CA:FALSE",
        ]))
        .unwrap();
        assert_eq!(mixed.ips.len(), 2);
        assert_eq!(mixed.other_names, 1);
        assert!(mixed.ips.contains(&"fd7a:115c:a1e0::1".parse().unwrap()));
    }

    /// A Netscape Comment extension placed BEFORE the real SAN whose text imitates
    /// a SAN header, as printed by `openssl x509 -text`. A text scraper reads the
    /// fake address; the DER reader must report only the real one.
    #[test]
    fn a_comment_that_imitates_a_san_header_cannot_change_the_address() {
        let fake = "x\n X509v3 Subject Alternative Name: \n IP Address:100.64.0.9\n";
        let mut hex = format!("16{:02x}", fake.len());
        for b in fake.bytes() {
            write!(hex, "{b:02x}").unwrap();
        }
        let pem = make(&[
            &format!("nsComment=DER:{hex}"),
            "subjectAltName=IP:100.64.0.77",
            "basicConstraints=critical,CA:FALSE",
        ]);
        let text = String::from_utf8(
            std::process::Command::new("openssl")
                .args(["x509", "-noout", "-text"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write as _;
                    child.stdin.take().unwrap().write_all(pem.as_bytes())?;
                    child.wait_with_output()
                })
                .unwrap()
                .stdout,
        )
        .unwrap();
        // Control: the text really does contain the forged header ahead of the real one.
        let forged = text.find("IP Address:100.64.0.9").unwrap();
        let real = text.find("IP Address:100.64.0.77").unwrap();
        assert!(
            forged < real,
            "the control must reproduce the spoof:\n{text}"
        );
        assert_eq!(
            facts(&pem).unwrap().ips,
            ["100.64.0.77".parse::<IpAddr>().unwrap()]
        );
    }

    #[test]
    fn text_outside_the_pem_block_is_refused() {
        let good = make(&["subjectAltName=IP:100.64.0.7", "basicConstraints=CA:FALSE"]);
        assert!(facts(&good).is_ok());
        // The loader that reads the trust file must see the same single certificate.
        assert!(facts(&format!("junk\n{good}")).is_err());
        assert!(facts(&format!("{good}junk")).is_err());
        assert!(facts(&format!("-----BEGIN CERTIFICATE- - - - -\nAAAA\n{good}")).is_err());
    }

    #[test]
    fn garbage_and_truncation_fail_closed() {
        let good = make(&["subjectAltName=IP:100.64.0.7", "basicConstraints=CA:FALSE"]);
        assert!(facts("not a certificate").is_err());
        assert!(facts(&format!("{good}{good}")).is_err(), "two certificates");
        let cut = good
            .replace("-----END CERTIFICATE-----", "")
            .replace('\n', "");
        assert!(facts(&cut).is_err());
        let mut lines: Vec<&str> = good.lines().collect();
        lines.remove(3);
        assert!(facts(&lines.join("\n")).is_err(), "a missing base64 line");
    }
}
