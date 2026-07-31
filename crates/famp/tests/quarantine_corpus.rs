//! Phase 14 plan 14-04: the FAMP-native adversarial corpus (QUAR-03),
//! its non-vacuity falsification control (QUAR-04), and the one-hop
//! laundering pin (QUAR-11).
//!
//! Published benchmarks (AgentDojo, InjecAgent, WASP) are tool-calling-
//! agent shaped, not message-relay shaped, so every payload here is
//! authored fresh against this codebase's actual quarantine mechanism
//! (D-23). The corpus drives `famp::cli::render::render_envelope_body`/
//! `render_body_text` directly — the same one shared helper every one of
//! the seven mechanical rendering surfaces (`famp_inbox`, `famp_await`,
//! CLI `await`, CLI `inbox list`, `famp_channel_log`, CLI `register
//! --tail`, CLI `wait-reply`) routes through (D-07). Those seven surfaces'
//! own routing correctness was already proven end-to-end by plan 14-02's
//! `quarantine_surfaces.rs`; this corpus's job is payload coverage against
//! the shared helper those surfaces all reach, not re-proving routing.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

#[path = "quarantine/fixtures.rs"]
mod fixtures;
#[path = "quarantine/harness.rs"]
mod harness;

use fixtures::{cases, Family};
use harness::run_all_surfaces;

/// `fixtures::cases()` must cover at least 20 cases with every `Family`
/// variant represented at least twice — the acceptance criteria's
/// non-vacuity check on the corpus itself.
#[test]
fn corpus_case_count_and_family_coverage() {
    let all = cases();
    assert!(
        all.len() >= 20,
        "corpus must contain at least 20 cases, got {}",
        all.len()
    );

    for family in Family::ALL {
        let count = all.iter().filter(|c| c.family == family).count();
        assert!(
            count >= 2,
            "family {family:?} must have at least 2 cases, got {count}"
        );
    }
}

/// D-23 (mandatory): payloads whose body text itself contains the
/// tagging delimiter — a guessed nonce, no nonce, and fabricated
/// instruction prose after a fake close. The real closing marker's nonce
/// must never appear inside the attacker-controlled region, and no
/// fabricated marker in the body may match the real stem.
#[test]
fn corpus_delimiter_emission_cannot_forge_a_block_close() {
    for case in cases()
        .iter()
        .filter(|c| c.family == Family::DelimiterEmission)
    {
        run_all_surfaces(case);
    }
}

/// Many candidate closing markers with sequential/guessed/replayed
/// nonces — none may match the real per-render nonce.
#[test]
fn corpus_nonce_guessing_never_matches() {
    for case in cases().iter().filter(|c| c.family == Family::NonceGuessing) {
        run_all_surfaces(case);
    }
}

/// The marker stem expressed with a zero-width joiner, combining marks,
/// base64, and a JSON string escape split — neutralization must be
/// idempotent and none may produce a matching marker.
#[test]
fn corpus_encoding_evasion_is_neutralized() {
    for case in cases()
        .iter()
        .filter(|c| c.family == Family::EncodingEvasion)
    {
        run_all_surfaces(case);
    }
}

/// A JSON object, a JSON array, a deeply nested body, an empty string,
/// and JSON null — every one still renders inside the marker pair for
/// gateway origin, and verbatim for local origin.
#[test]
fn corpus_structural_bodies_stay_enclosed() {
    for case in cases()
        .iter()
        .filter(|c| c.family == Family::StructuralEvasion)
    {
        run_all_surfaces(case);
    }
}

/// Bodies shaped like a system instruction, a tool-call request, and an
/// operator note — no special handling; marked exactly like any other
/// remote text. Pins that the mechanism is provenance, not content
/// classification.
#[test]
fn corpus_role_confusion_prose_is_marked_not_classified() {
    for case in cases().iter().filter(|c| c.family == Family::RoleConfusion) {
        run_all_surfaces(case);
    }
}

/// A body near a size boundary, a body containing raw newlines, and a
/// body containing terminal control sequences — still enclosed, no
/// panic, no non-terminating render.
#[test]
fn corpus_size_and_control_characters_stay_enclosed() {
    for case in cases()
        .iter()
        .filter(|c| c.family == Family::SizeAndControl)
    {
        run_all_surfaces(case);
    }
}

/// QUAR-04's must-still-pass control: every case at `Origin::Local`
/// renders byte-identical passthrough, regardless of family.
#[test]
fn corpus_local_origin_renders_verbatim() {
    for case in cases() {
        let rendered = famp::cli::render::render_envelope_body(famp_bus::Origin::Local, &case.body);
        assert_eq!(
            rendered, case.body,
            "case {}: local-origin body must render byte-identical",
            case.id
        );
    }
}

/// QUAR-06 widened to the full corpus: the wake notification payload
/// never carries any case's body text, structural or string-shaped.
#[test]
fn corpus_wake_payload_carries_no_case_body() {
    for case in cases() {
        harness::assert_wake_payload_excludes_case_body(&case);
    }
}
