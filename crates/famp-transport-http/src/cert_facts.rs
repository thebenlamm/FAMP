//! A minimal X.509 reader for the facts a pinned peer certificate must prove.
//!
//! These are read from the certificate's DER bytes, never from `openssl -text`
//! output: that output is free text, and a hostile certificate can embed text that
//! looks like a subjectAltName header (for example inside a Netscape Comment
//! extension), which a line scraper would take for the real one.
//!
//! Only three facts are extracted: the iPAddress subjectAltName entries, how many
//! other SAN entries there are, and the `BasicConstraints` cA flag. Anything this
//! reader does not understand is an error, so a malformed certificate fails closed.
//! It lives in this crate so the pin-only TLS verifier can enforce the rule at the
//! point of use, whoever built the trust file.
use std::net::IpAddr;

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

/// Parse DER certificate bytes and report what they declare.
///
/// # Errors
///
/// A description of the first thing that is not a well-formed X.509 v3 certificate
/// with at most one subjectAltName and one `BasicConstraints` extension.
pub fn facts_der(der: &[u8]) -> Result<CertFacts, String> {
    let (tag, cert, trailing) = tlv(der)?;
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
    use super::*;

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
}
