//! A minimal X.509 reader for the facts a peer card must prove about its certificate.
//!
//! These are read from the certificate's DER bytes, never from `openssl -text`
//! output: that output is free text, and a hostile certificate can embed text that
//! looks like a subjectAltName header (for example inside a Netscape Comment
//! extension), which a line scraper would take for the real one.
//!
//! Only three facts are extracted: the iPAddress subjectAltName entries, how many
//! other SAN entries there are, and the `BasicConstraints` cA flag. Anything this
//! reader does not understand is an error, so a malformed certificate fails closed.
use std::net::IpAddr;

use base64::Engine as _;

const OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];
const OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13];

/// What a certificate declares about itself.
#[derive(Debug, PartialEq, Eq)]
pub struct CertFacts {
    /// iPAddress subjectAltName entries, in order.
    pub ips: Vec<IpAddr>,
    /// subjectAltName entries that are not IP addresses (DNS names and so on).
    pub other_names: usize,
    /// `BasicConstraints` cA flag; `None` when the extension is absent.
    pub is_ca: Option<bool>,
}

/// One DER element: (tag, content, remaining bytes).
fn tlv(b: &[u8]) -> Result<(u8, &[u8], &[u8]), String> {
    let (&tag, rest) = b.split_first().ok_or("truncated DER")?;
    let (&first, rest) = rest.split_first().ok_or("truncated DER length")?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 || rest.len() < n {
            return Err("unsupported DER length".to_owned());
        }
        let (len_bytes, rest) = rest.split_at(n);
        let len = len_bytes
            .iter()
            .fold(0_usize, |acc, &x| (acc << 8) | usize::from(x));
        (len, rest)
    };
    if rest.len() < len {
        return Err("DER element runs past its parent".to_owned());
    }
    let (content, rest) = rest.split_at(len);
    Ok((tag, content, rest))
}

/// The iPAddress entries and the count of every other entry in a subjectAltName value.
fn parse_san(san: &[u8]) -> Result<(Vec<IpAddr>, usize), String> {
    let (tag, mut names, trailing) = tlv(san)?;
    if tag != 0x30 || !trailing.is_empty() {
        return Err("malformed subjectAltName".to_owned());
    }
    let (mut ips, mut other_names) = (Vec::new(), 0);
    while !names.is_empty() {
        let (tag, content, after) = tlv(names)?;
        names = after;
        match (tag, content.len()) {
            (0x87, 4) => {
                let octets: [u8; 4] = content.try_into().map_err(|_| "bad IPv4")?;
                ips.push(IpAddr::from(octets));
            }
            (0x87, 16) => {
                let octets: [u8; 16] = content.try_into().map_err(|_| "bad IPv6")?;
                ips.push(IpAddr::from(octets));
            }
            (0x87, _) => return Err("iPAddress of an impossible length".to_owned()),
            _ => other_names += 1,
        }
    }
    Ok((ips, other_names))
}

/// The cA flag of a `BasicConstraints` value (DEFAULT FALSE).
fn parse_basic_constraints(value: &[u8]) -> Result<bool, String> {
    let (tag, fields, trailing) = tlv(value)?;
    if tag != 0x30 || !trailing.is_empty() {
        return Err("malformed BasicConstraints".to_owned());
    }
    let mut ca = false;
    let mut rest = fields;
    if rest.first() == Some(&0x01) {
        let (_, flag, after) = tlv(rest)?;
        if flag.len() != 1 {
            return Err("BasicConstraints cA is not a one-byte BOOLEAN".to_owned());
        }
        ca = flag[0] != 0;
        rest = after;
    }
    // Only an optional pathLenConstraint INTEGER may follow.
    if !rest.is_empty() {
        let (tag, _, after) = tlv(rest)?;
        if tag != 0x02 || !after.is_empty() {
            return Err("unexpected data in BasicConstraints".to_owned());
        }
    }
    Ok(ca)
}

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
/// A description of the first thing that is not a well-formed X.509 v3 certificate
/// with at most one subjectAltName and one `BasicConstraints` extension.
pub fn facts(pem: &str) -> Result<CertFacts, String> {
    let der = pem_to_der(pem)?;
    let (tag, cert, trailing) = tlv(&der)?;
    if tag != 0x30 || !trailing.is_empty() {
        return Err("not a certificate".to_owned());
    }
    let (tag, tbs, _) = tlv(cert)?;
    if tag != 0x30 {
        return Err("no TBSCertificate".to_owned());
    }
    let mut rest = tbs;
    // [0] version (optional), then serial, signature, issuer, validity, subject, spki.
    if rest.first() == Some(&0xa0) {
        rest = tlv(rest)?.2;
    }
    for _ in 0..6 {
        rest = tlv(rest)?.2;
    }
    // [1] issuerUniqueID and [2] subjectUniqueID are optional.
    while matches!(rest.first(), Some(&0x81 | &0x82)) {
        rest = tlv(rest)?.2;
    }
    let (tag, ext_wrapper, after_wrapper) =
        tlv(rest).map_err(|_| "certificate has no extensions".to_owned())?;
    if tag != 0xa3 || !after_wrapper.is_empty() {
        return Err("certificate has no extensions, or has data after them".to_owned());
    }
    let (tag, mut exts, after_exts) = tlv(ext_wrapper)?;
    if tag != 0x30 || !after_exts.is_empty() {
        return Err("malformed extensions".to_owned());
    }
    let (mut san, mut basic): (Option<&[u8]>, Option<&[u8]>) = (None, None);
    while !exts.is_empty() {
        let (tag, ext, after) = tlv(exts)?;
        exts = after;
        if tag != 0x30 {
            return Err("malformed extension".to_owned());
        }
        let (tag, oid, mut fields) = tlv(ext)?;
        if tag != 0x06 {
            return Err("extension has no OID".to_owned());
        }
        let (mut tag, mut value, mut next) = tlv(fields)?;
        if tag == 0x01 {
            // optional `critical` BOOLEAN
            fields = next;
            (tag, value, next) = tlv(fields)?;
        }
        if tag != 0x04 || !next.is_empty() {
            return Err("malformed extension value".to_owned());
        }
        let slot = if oid == OID_SAN {
            &mut san
        } else if oid == OID_BASIC_CONSTRAINTS {
            &mut basic
        } else {
            continue;
        };
        if slot.replace(value).is_some() {
            return Err("duplicate extension".to_owned());
        }
    }
    let (ips, other_names) = parse_san(san.ok_or("certificate has no subjectAltName")?)?;
    let is_ca = basic.map(parse_basic_constraints).transpose()?;
    Ok(CertFacts {
        ips,
        other_names,
        is_ca,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fmt::Write as _;

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
    fn trailing_data_and_odd_encodings_are_errors_not_ignored() {
        let ip = |last: u8| vec![0x87, 0x04, 100, 64, 0, last];
        let seq = |body: &[u8]| {
            let mut v = vec![0x30, u8::try_from(body.len()).unwrap()];
            v.extend_from_slice(body);
            v
        };
        // One well-formed SAN, then the same with a SECOND sequence appended.
        let good_san = seq(&ip(7));
        assert_eq!(parse_san(&good_san).unwrap().0.len(), 1);
        let mut two = good_san;
        two.extend(seq(&ip(8)));
        assert!(parse_san(&two).is_err());
        // BasicConstraints: cA TRUE parses; a hidden second SEQUENCE does not.
        assert_eq!(parse_basic_constraints(&seq(&[0x01, 0x01, 0xff])), Ok(true));
        assert_eq!(parse_basic_constraints(&seq(&[])), Ok(false));
        let mut hidden = seq(&[]);
        hidden.extend(seq(&[0x01, 0x01, 0xff]));
        assert!(parse_basic_constraints(&hidden).is_err());
        // A two-byte BOOLEAN, and pathLen before cA, are refused rather than guessed at.
        assert!(parse_basic_constraints(&seq(&[0x01, 0x02, 0xff, 0xff])).is_err());
        assert!(parse_basic_constraints(&seq(&[0x02, 0x01, 0x00, 0x01, 0x01, 0xff])).is_err());
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
