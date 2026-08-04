//! Vendored BIP-39 English word list (2048 words, 11 bits/word) — the
//! entropy source for the five-word pairing code (PAIR-02, T-18-01).
//!
//! UPSTREAM LICENSE DETERMINATION: RESOLVED — Ben, 2026-08-03: BIP-39's MIT
//! license permits vendoring this data file. Recorded as the project owner's
//! determination at Phase 18 Task 3's blocking `checkpoint:human-verify`.
//! The findings below are retained verbatim as the record of what was
//! actually checked when that call was made — not as a counter-argument to
//! it, but so the determination can be revisited cheaply if it ever needs
//! to be. Swapping to another 2048-word list costs only this module and
//! `WORDLIST_SHA256`; nothing downstream depends on which list is used.
//!
//! `bip39-english.txt` (sibling file) was fetched verbatim, this session,
//! from `https://raw.githubusercontent.com/bitcoin/bips/master/bip-0039/english.txt`.
//! Its measured SHA-256 is pinned below as [`WORDLIST_SHA256`] and is
//! re-asserted by [`tests::wordlist_matches_pinned_sha256`] against the
//! `include_str!`-ed bytes on every test run.
//!
//! **What was found when the wordlist was fetched, quoted literally. These
//! notes deliberately carry no characterization of their own — the
//! determination above is the project owner's, not the planner's or the
//! executor's:**
//!
//! 1. No repository-root `LICENSE` file exists in `bitcoin/bips`.
//!    `https://raw.githubusercontent.com/bitcoin/bips/master/LICENSE`
//!    returned HTTP 404 (body: `404: Not Found`) when fetched this
//!    session. The GitHub API directory listing
//!    (`https://api.github.com/repos/bitcoin/bips/contents/`), also
//!    fetched this session, lists no `LICENSE`, `LICENSE.md`, or
//!    `LICENSE.txt` entry at the repository root — only
//!    `.gitattributes`, `.github`, `.gitignore`, `.typos.toml`,
//!    `CONTRIBUTING.md`, `README.mediawiki`, and the numbered
//!    `bip-NNNN.mediawiki` / `bip-NNNN` files/directories.
//!
//! 2. `README.mediawiki` (repository root, fetched this session) contains
//!    no `license`, `copyright`, or `public domain` text (a case-insensitive
//!    grep over the fetched file found zero matches).
//!
//! 3. `bip-0039.mediawiki` (the BIP-39 specification document itself,
//!    fetched this session, same commit as the wordlist) contains, quoted
//!    verbatim:
//!
//!    ```text
//!    <pre>
//!      BIP: 39
//!      Layer: Applications
//!      Title: Mnemonic code for generating deterministic keys
//!      Authors: Marek Palatinus <slush@satoshilabs.com>
//!               Pavol Rusnak <stick@satoshilabs.com>
//!               Aaron Voisine <voisine@gmail.com>
//!               Sean Bowe <ewillbefull@gmail.com>
//!      Status: Deployed
//!      Type: Specification
//!      Assigned: 2013-09-10
//!      License: MIT
//!    </pre>
//!
//!    ==Copyright==
//!
//!    This BIP falls under the MIT License.
//!    ```
//!
//! That block comment is the entire text this session found asserting any
//! license for BIP-39 or its associated data. Whether an MIT license
//! stated on the BIP-39 *specification document* (a mediawiki text file
//! describing the mnemonic algorithm) extends to the separately-hosted
//! `bip-0039/english.txt` *data file* this module vendors — a distinct
//! file, not textually part of the mediawiki document quoted above, and
//! itself carrying no license header of its own — is the question Task 3
//! put to the project owner. It was resolved in the affirmative; see the
//! determination at the top of this comment.

use std::sync::LazyLock;

use sha2::{Digest, Sha256};

use crate::pairing::PairingError;

/// Raw vendored wordlist text: LF line endings, no trailing blank line.
/// See `bip39-english.txt` (sibling file) for the fetched bytes.
const WORDLIST_TXT: &str = include_str!("bip39-english.txt");

/// SHA-256 of [`WORDLIST_TXT`]'s raw bytes, measured this session via
/// `shasum -a 256` over the fetched file — the pin is whatever was
/// actually measured, never a value copied from a spec document.
pub const WORDLIST_SHA256: &str =
    "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda";

/// Number of words in the vendored list.
///
/// The requirement's own math (`5 words x 11 bits = 55 bits`,
/// REQUIREMENTS.md) requires an exactly-2048-word, power-of-two list so
/// each word encodes exactly 11 bits with a rejection-sampled uniform
/// draw.
pub const WORDLIST_LEN: usize = 2048;

/// Number of words drawn per pairing code.
pub const CODE_WORD_COUNT: usize = 5;

/// The parsed word list, split on first use. A `LazyLock`-backed
/// `Vec<&'static str>` rather than a `[&str; WORDLIST_LEN]` const array —
/// this plan's own text states either shape is acceptable ("the array
/// shape is not load-bearing"); splitting an `include_str!`-provided
/// runtime string into a genuine `const` array is not expressible in
/// stable Rust without a build script, so the `LazyLock<Vec<_>>` form is
/// used instead.
static WORDLIST: LazyLock<Vec<&'static str>> = LazyLock::new(|| WORDLIST_TXT.lines().collect());

/// Borrow the full word list.
#[must_use]
pub fn wordlist() -> &'static [&'static str] {
    &WORDLIST
}

/// A drawn or parsed five-word pairing code, canonically stored as five
/// space-joined lowercase words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Borrow the canonical space-joined lowercase form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Draw a fresh, uniformly-random five-word code.
///
/// `rng.gen_range(0..WORDLIST_LEN)` — `rand` 0.8's `Rng::gen_range`
/// already rejection-samples internally, so this draw carries no modulo
/// bias (a naive `byte % 2048` would). Production callers pass
/// `rand::rngs::OsRng`; tests pass a seeded `StdRng` for the
/// uniform-coverage proof below.
pub fn draw_code<R: rand::Rng>(rng: &mut R) -> PairingCode {
    let words = wordlist();
    let picked: Vec<&str> = (0..CODE_WORD_COUNT)
        .map(|_| words[rng.gen_range(0..WORDLIST_LEN)])
        .collect();
    PairingCode(picked.join(" "))
}

/// Parse a user-typed code: lowercase, collapse whitespace runs to a
/// single space, trim, and require exactly five tokens each present in
/// the word list.
pub fn parse_code(raw: &str) -> Result<PairingCode, PairingError> {
    let lowered = raw.to_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();
    if tokens.len() != CODE_WORD_COUNT {
        return Err(PairingError::CodeMalformed {
            reason: format!(
                "expected exactly {CODE_WORD_COUNT} words, got {}",
                tokens.len()
            ),
        });
    }
    let words = wordlist();
    for tok in &tokens {
        if !words.contains(tok) {
            return Err(PairingError::CodeMalformed {
                reason: format!("'{tok}' is not in the pairing word list"),
            });
        }
    }
    Ok(PairingCode(tokens.join(" ")))
}

/// SHA-256 digest of a code's canonical space-joined lowercase form.
#[must_use]
pub fn code_digest(code: &PairingCode) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(code.as_str().as_bytes());
    hasher.finalize().into()
}

/// Fixed-length XOR-accumulate comparison over all 32 bytes, with no
/// early return — comparison time does not depend on how many leading
/// bytes matched (T-18-02).
#[must_use]
pub fn digests_equal(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Decode a hex-encoded 32-byte digest (the on-disk
/// `InviteRecord::code_digest` shape) back into fixed bytes.
///
/// `None` on any malformed input (wrong length, non-hex characters) —
/// callers treat that identically to "does not match" rather than
/// propagating a parse error, so a corrupt on-disk digest fails closed
/// the same way a legitimate mismatch does. Exposed here (rather than
/// requiring every caller, including `famp-gateway` which does not
/// depend on the `hex` crate, to decode hex itself) so the encoding
/// stays this module's concern.
#[must_use]
pub fn digest_from_hex(s: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(s).ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    Some(arr)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        code_digest, digests_equal, draw_code, parse_code, wordlist, WORDLIST_LEN, WORDLIST_SHA256,
        WORDLIST_TXT,
    };
    use rand::{rngs::StdRng, SeedableRng};
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;

    /// Every line is 3-8 lowercase ASCII letters, all 2048 are unique, and
    /// all 2048 four-character prefixes are unique.
    #[test]
    fn wordlist_has_exactly_2048_unique_entries_with_unique_prefixes() {
        let words = wordlist();
        assert_eq!(words.len(), WORDLIST_LEN, "expected exactly 2048 words");

        let mut seen = HashSet::new();
        let mut prefixes = HashSet::new();
        for w in words {
            assert!(
                (3..=8).contains(&w.len()),
                "word '{w}' has length {} outside 3-8",
                w.len()
            );
            assert!(
                w.chars().all(|c| c.is_ascii_lowercase()),
                "word '{w}' contains a non-lowercase-ASCII character"
            );
            assert!(seen.insert(*w), "duplicate word '{w}'");
            let prefix: String = w.chars().take(4).collect();
            assert!(
                prefixes.insert(prefix.clone()),
                "duplicate 4-char prefix '{prefix}' (from word '{w}')"
            );
        }
    }

    /// Recompute SHA-256 over the `include_str!`-ed bytes and assert it
    /// equals the pinned constant.
    #[test]
    fn wordlist_matches_pinned_sha256() {
        let mut hasher = Sha256::new();
        hasher.update(WORDLIST_TXT.as_bytes());
        let digest = hasher.finalize();
        let hex_digest = hex::encode(digest);
        assert_eq!(
            hex_digest, WORDLIST_SHA256,
            "recomputed SHA-256 of the vendored wordlist does not match the pinned constant"
        );
    }

    /// 20_000 seeded draws must cover every index in `0..2048` at least
    /// once and never produce an index `>= 2048` — the precision-edge
    /// proof that `gen_range` carries no modulo bias.
    #[test]
    fn uniform_draw_covers_every_index_with_no_out_of_range() {
        let mut rng = StdRng::seed_from_u64(42);
        let words = wordlist();
        let mut seen_indices: HashSet<usize> = HashSet::new();

        for _ in 0..20_000 {
            let code = draw_code(&mut rng);
            for tok in code.as_str().split(' ') {
                let idx = words.iter().position(|w| *w == tok).unwrap();
                assert!(idx < WORDLIST_LEN, "drew out-of-range index {idx}");
                seen_indices.insert(idx);
            }
        }

        assert_eq!(
            seen_indices.len(),
            WORDLIST_LEN,
            "expected all {WORDLIST_LEN} indices to be hit at least once across 20_000 draws \
             (5 words/draw = 100_000 word picks); got {} distinct indices",
            seen_indices.len()
        );
    }

    #[test]
    fn parse_code_lowercases_collapses_whitespace_and_trims() {
        let words = wordlist();
        let raw = format!(
            "  {}   {} {}\t{}  {}  ",
            words[0].to_uppercase(),
            words[1],
            words[2],
            words[3],
            words[4]
        );
        let code = parse_code(&raw).unwrap();
        assert_eq!(
            code.as_str(),
            format!(
                "{} {} {} {} {}",
                words[0], words[1], words[2], words[3], words[4]
            )
        );
    }

    #[test]
    fn parse_code_rejects_wrong_word_count() {
        let words = wordlist();
        let err = parse_code(&format!("{} {} {}", words[0], words[1], words[2])).unwrap_err();
        assert!(matches!(
            err,
            crate::pairing::PairingError::CodeMalformed { .. }
        ));
    }

    #[test]
    fn parse_code_rejects_unknown_word() {
        let words = wordlist();
        let err = parse_code(&format!(
            "{} {} {} {} notarealbip39word",
            words[0], words[1], words[2], words[3]
        ))
        .unwrap_err();
        assert!(matches!(
            err,
            crate::pairing::PairingError::CodeMalformed { .. }
        ));
    }

    #[test]
    fn digests_equal_matches_identical_and_rejects_one_bit_flip() {
        let mut rng = StdRng::seed_from_u64(7);
        let code = draw_code(&mut rng);
        let a = code_digest(&code);
        let mut b = a;
        assert!(digests_equal(&a, &b));
        b[31] ^= 0x01;
        assert!(!digests_equal(&a, &b));
    }
}
