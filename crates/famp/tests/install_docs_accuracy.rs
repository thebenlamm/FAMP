//! DIST-04 / 16-04: accuracy gate across all four onboarding docs -- the
//! prebuilt-binary curl installer must lead every doc, every surviving
//! from-source fallback must be a command that actually runs against this
//! repo, the documented fallback must be the exact string
//! `.github/workflows/smoke-test.yml` exercises (closing the root cause of
//! research Pitfall 1: the docs printed one command while CI exercised a
//! different one), and the checksum security claim must match D-06's
//! locked wording exactly, never a stronger paraphrase.
//!
//! Mirrors `crates/famp/tests/gateway_setup_doc_accuracy.rs`'s shape: doc
//! paths resolved relative to `CARGO_MANIFEST_DIR`, read at runtime via
//! `fs::read_to_string` (never `include_str!`), whitespace-normalized
//! substring assertions, and a `.find(...).expect(...)` byte-offset
//! ordering comparison. Assertions are scoped to the four onboarding docs
//! (plus `.github/workflows/smoke-test.yml` for the fallback-parity check
//! and `docs/DISTRIBUTION.md` for the claim-boundary check) -- there is no
//! repo-wide grep, so this test file's own source text can never match
//! itself.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
// Four docs, three tests, each holding every assertion for its own concern
// in one place so a failure message stays next to the context that
// explains it -- splitting further would scatter that context away from
// the docs it validates, mirroring gateway_setup_doc_accuracy.rs's own
// file-level allow for the same reason.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::path::PathBuf;

/// The prebuilt-binary installer URL every onboarding doc must lead with.
const INSTALLER_URL: &str = "releases/latest/download/famp-installer.sh";

/// The exact D-06 locked sentence. `checksum_security_claim_matches_the_locked_wording`
/// asserts every mention of the *outcome clause* below is exactly this
/// full sentence, never a paraphrase.
const D06_LOCKED_CLAIM: &str =
    "it does not, by itself, prove the release workflow was not compromised";

/// The outcome clause alone, used to detect a paraphrase: if this appears
/// more often than `D06_LOCKED_CLAIM`, something claims the outcome with
/// different (possibly stronger) wording around it.
const D06_OUTCOME_CLAUSE: &str = "release workflow was not compromised";

struct Doc {
    label: &'static str,
    rel_path: &'static str,
}

/// The four onboarding docs DIST-04 targets, in the order D-01's table
/// lists them.
const ONBOARDING_DOCS: &[Doc] = &[
    Doc {
        label: "README.md",
        rel_path: "../../README.md",
    },
    Doc {
        label: "docs/GETTING-STARTED.md",
        rel_path: "../../docs/GETTING-STARTED.md",
    },
    Doc {
        label: "docs/GATEWAY-SETUP.md",
        rel_path: "../../docs/GATEWAY-SETUP.md",
    },
    Doc {
        label: "docs/ONBOARDING.md",
        rel_path: "../../docs/ONBOARDING.md",
    },
];

fn read_doc(doc: &Doc) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(doc.rel_path);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "update the doc or the pipeline: could not read {} at {}: {e}",
            doc.label,
            path.display()
        )
    })
}

fn read_repo_file(rel_path: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "update the doc or the pipeline: could not read {}: {e}",
            path.display()
        )
    })
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn binary_install_path_leads_every_onboarding_doc() {
    for doc in ONBOARDING_DOCS {
        let text = read_doc(doc);

        let installer_idx = text.find(INSTALLER_URL).unwrap_or_else(|| {
            panic!(
                "update the doc or the pipeline: {} is missing the prebuilt-binary \
                 installer URL ({INSTALLER_URL}) -- DIST-04 requires it to lead every \
                 onboarding doc",
                doc.label
            )
        });

        // If a from-source `cargo install` mention exists anywhere in the
        // doc, the installer URL must appear strictly before it -- this is
        // the same `.find(...).expect(...)` + index-comparison shape
        // gateway_setup_doc_accuracy.rs uses for its own ordering checks.
        if let Some(from_source_idx) = text.find("cargo install") {
            assert!(
                installer_idx < from_source_idx,
                "update the doc or the pipeline: {} must lead with the prebuilt-binary \
                 installer -- found `cargo install` at byte {from_source_idx}, before the \
                 installer URL at byte {installer_idx}",
                doc.label
            );
        }

        // No bare-crate-name install form survives: a line consisting
        // only of the install verb and this project's crate name, with no
        // --path and no --git argument. famp was never published to
        // crates.io (D-01) -- this exact command has never worked.
        for bare in ["cargo install famp", "cargo install famp-gateway"] {
            for line in text.lines() {
                assert!(
                    line.trim() != bare,
                    "update the doc or the pipeline: {} still has a bare-crate-name \
                     `{bare}` line -- famp was never published to crates.io (D-01); use \
                     the installer URL or a `cargo install --path`/`--git` form",
                    doc.label
                );
            }
        }
    }
}

#[test]
fn from_source_fallback_command_is_a_working_form() {
    // Every from-source install of this project's own crate(s) must carry
    // --path or --git. Scan for "cargo install ... famp[-gateway]"
    // mentions (lazily, so an earlier occurrence doesn't swallow a later
    // one) and require --path/--git to appear within that same mention.
    let re = regex::Regex::new(r"cargo install[^\n]*?\bfamp(?:-gateway)?\b")
        .expect("from-source-command regex must compile");

    for doc in ONBOARDING_DOCS {
        let text = read_doc(doc);
        for m in re.find_iter(&text) {
            let cmd = m.as_str();
            assert!(
                cmd.contains("--path") || cmd.contains("--git"),
                "update the doc or the pipeline: {} has a from-source install of this \
                 project's own crate with no --path/--git argument: `{cmd}` -- that command \
                 does not resolve against a published crate (D-01)",
                doc.label
            );
        }
    }

    // The point of this test: the exact fallback string documented in
    // README must also appear in smoke-test.yml, so the doc and the CI
    // job it's supposed to mirror can never silently diverge the way the
    // original `cargo install famp` did (research Pitfall 1's root
    // cause -- the docs printed a command CI never exercised).
    let readme = read_doc(&ONBOARDING_DOCS[0]);
    let smoke_test = read_repo_file("../../.github/workflows/smoke-test.yml");
    let fallback = "cargo install --path crates/famp";
    assert!(
        readme.contains(fallback),
        "update the doc or the pipeline: README.md must document the fallback command \
         `{fallback}`"
    );
    assert!(
        smoke_test.contains(fallback),
        "update the doc or the pipeline: .github/workflows/smoke-test.yml no longer runs \
         `{fallback}` -- the documented README fallback and the command CI actually \
         exercises have diverged again (research Pitfall 1's root cause)"
    );
}

#[test]
fn checksum_security_claim_matches_the_locked_wording() {
    // README must carry D-06's exact locked sentence at least once.
    let readme = normalize(&read_doc(&ONBOARDING_DOCS[0]));
    assert!(
        readme.contains(D06_LOCKED_CLAIM),
        "update the doc or the pipeline: README.md is missing D-06's exact locked \
         checksum-claim sentence (see 16-CONTEXT.md D-06): `{D06_LOCKED_CLAIM}`"
    );

    // Every doc that mentions the outcome clause at all must mention it as
    // the *exact* locked sentence -- not a paraphrase, not a stronger
    // claim. Comparing the count of the outcome clause against the count
    // of the full locked sentence catches both: a paraphrase changes the
    // wording around the clause, so the full-sentence count would be
    // lower than the clause count.
    let distribution = normalize(&read_repo_file("../../docs/DISTRIBUTION.md"));
    for (label, normalized) in [
        ("README.md", readme.as_str()),
        ("docs/DISTRIBUTION.md", distribution.as_str()),
    ] {
        let clause_count = normalized.matches(D06_OUTCOME_CLAUSE).count();
        let full_count = normalized.matches(D06_LOCKED_CLAIM).count();
        assert_eq!(
            clause_count, full_count,
            "update the doc: {label} mentions the release-workflow-compromise outcome \
             {clause_count} time(s) but only {full_count} of those are D-06's exact locked \
             sentence -- see 16-CONTEXT.md D-06, overclaiming here is worse than \
             underclaiming"
        );
    }

    // No doc may claim checksums verify authenticity, provenance, or
    // publisher identity -- that is a stronger claim than D-06 licenses.
    let overclaim_phrases = [
        "verifies authenticity",
        "verify authenticity",
        "proves authenticity",
        "verifies the publisher",
        "verifies publisher identity",
        "proves provenance",
        "guarantees provenance",
        "proves the release workflow was not compromised",
    ];
    for doc in ONBOARDING_DOCS {
        let normalized = normalize(&read_doc(doc));
        for phrase in overclaim_phrases {
            assert!(
                !normalized.to_lowercase().contains(phrase),
                "update the doc: {} contains the overclaim phrase `{phrase}` -- D-06 \
                 forbids claiming checksums verify authenticity, provenance, or publisher \
                 identity (see 16-CONTEXT.md D-06)",
                doc.label
            );
        }
    }
    for phrase in overclaim_phrases {
        assert!(
            !distribution.to_lowercase().contains(phrase),
            "update the doc: docs/DISTRIBUTION.md contains the overclaim phrase `{phrase}` \
             -- D-06 forbids claiming checksums verify authenticity, provenance, or \
             publisher identity (see 16-CONTEXT.md D-06)"
        );
    }
}
