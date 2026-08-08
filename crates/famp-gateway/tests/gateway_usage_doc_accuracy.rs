//! DOC-04 / D-07: accuracy gate — `docs/GATEWAY-SETUP.md`'s documented
//! `famp-gateway` flags must be a subset of the flags the shipping binary
//! actually accepts.
//!
//! `famp-gateway` is a hand-rolled arg parser (no clap, no `--help`). The
//! closest thing to `--help` is a no-args invocation: with zero positional
//! principal names, `parse_args` hits the `names.is_empty()` branch and
//! prints the full `usage: famp-gateway ...` string to stderr, then exits
//! non-zero (see `crates/famp-gateway/src/main.rs`). This test runs that
//! probe and asserts every gateway flag documented in the guide appears in
//! the live usage string — guide flags subset binary flags, never the
//! reverse. Fails if `docs/GATEWAY-SETUP.md` documents a flag the binary no
//! longer accepts (or misspells one).

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

/// The exact set of `famp-gateway` flags this doc-accuracy gate checks are
/// documented at all. Sourced from `crates/famp-gateway/src/main.rs`'s
/// `parse_args` match arms — kept in sync manually; if a flag is
/// added/removed from `main.rs`, this list (and the guide) must be updated
/// together.
const GATEWAY_FLAGS: &[&str] = &[
    "--socket",
    "--listen",
    "--tls-cert",
    "--tls-key",
    "--peer",
    "--backs",
    "--trust-cert",
    "--relay-fetch",
    "--pairing-store",
];

fn gateway_setup_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/GATEWAY-SETUP.md")
}

/// Extract every `--flag`-shaped token that appears inside a fenced code
/// block whose contents mention `famp-gateway` as a command. This is the
/// non-vacuous half of the gate: it does NOT rely on a fixed whitelist, so
/// a stray/typo'd flag added to a guide command example (e.g.
/// `--bogus-flag`) is caught even though it was never in `GATEWAY_FLAGS`.
fn extract_documented_gateway_flags(doc: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let mut in_block = false;
    let mut block_lines: Vec<&str> = Vec::new();

    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            if in_block {
                // Closing fence: process the collected block if it looks
                // like a famp-gateway command example.
                if block_lines
                    .iter()
                    .any(|l| l.trim_start().starts_with("famp-gateway"))
                {
                    for block_line in &block_lines {
                        for tok in block_line.split_whitespace() {
                            let tok = tok.trim_end_matches('\\');
                            if tok.starts_with("--") {
                                flags.push(tok.to_owned());
                            }
                        }
                    }
                }
                block_lines.clear();
            }
            in_block = !in_block;
            continue;
        }
        if in_block {
            block_lines.push(line);
        }
    }

    flags.sort();
    flags.dedup();
    flags
}

#[test]
fn gateway_usage_doc_accuracy() {
    // Probe: no-args invocation prints the full usage string to stderr and
    // exits non-zero (no positional principal names supplied).
    let out = Command::cargo_bin("famp-gateway")
        .unwrap()
        .output()
        .expect("famp-gateway must be runnable as a subprocess");
    assert!(
        !out.status.success(),
        "famp-gateway with no args must exit non-zero; got {:?}",
        out.status
    );
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Sanity: the probe itself must actually be the usage string, not some
    // unrelated error — otherwise this gate would vacuously pass.
    assert!(
        stderr.contains("usage: famp-gateway"),
        "expected the no-args probe to print the usage string; got stderr:\n{stderr}"
    );

    for flag in GATEWAY_FLAGS {
        assert!(
            stderr.contains(flag),
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: expected famp-gateway's usage string to contain '{flag}', \
             but it did not. Usage string was:\n{stderr}"
        );
    }

    // Guide-content half: every flag we know the binary accepts (this list)
    // must actually be documented in the guide too, so the guide can't
    // silently drop coverage of a flag while still passing the binary-side
    // check above.
    let doc_path = gateway_setup_doc_path();
    let doc = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| {
        panic!(
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: could not read {}: {e}",
            doc_path.display()
        )
    });
    for flag in GATEWAY_FLAGS {
        assert!(
            doc.contains(flag),
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: guide is missing documented flag '{flag}'"
        );
    }

    // Non-vacuity check (the actual drift detector): every `--flag` token
    // that appears in one of the guide's `famp-gateway` command examples
    // must be a flag the live binary actually accepts (present in its
    // usage string). This catches a stray/typo'd flag added to a command
    // example — not just missing coverage of a known-good flag.
    let documented_flags = extract_documented_gateway_flags(&doc);
    assert!(
        !documented_flags.is_empty(),
        "expected at least one `--flag` token inside a famp-gateway command example \
         in docs/GATEWAY-SETUP.md; extraction found none — the guide's example block \
         may have changed shape"
    );
    for flag in &documented_flags {
        assert!(
            stderr.contains(flag.as_str()),
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: guide's famp-gateway command example uses '{flag}', which is \
             not present in the live famp-gateway usage string:\n{stderr}"
        );
    }
}
