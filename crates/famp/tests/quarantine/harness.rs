//! Shared corpus runner (Phase 14 plan 14-04).
//!
//! `run_all_surfaces` drives the exact production entry points every one
//! of the seven mechanical rendering surfaces reaches —
//! `render::render_envelope_body` (used directly by `famp_inbox`, CLI
//! `inbox list`, `famp_channel_log`, and indirectly by `famp_await`/CLI
//! `await`/CLI `wait-reply` via `render_stamped_envelopes`, which itself
//! only calls `render_envelope_body`) and `render::render_body_text`
//! (used directly by CLI `register --tail`'s `emit_tail_line`) — never a
//! test-local reimplementation of the wrapping logic (D-07).
//!
//! Each of the seven surfaces is a thin wrapper with no logic beyond
//! calling one of these two functions and shaping the result into its
//! own output envelope; plan 14-02's `quarantine_surfaces.rs` already
//! proved that routing end-to-end for all seven (13 tests). This corpus
//! drives the shared helper those surfaces reach directly, rather than
//! re-spinning a live broker + subprocess per surface for every one of
//! the corpus's 24 cases (168 process spawns for zero incremental
//! routing signal) — see 14-04-SUMMARY.md's Decisions Made for the full
//! rationale.

#![allow(dead_code)]

use crate::fixtures::CorpusCase;
use famp::cli::render::{render_body_text, render_envelope_body, MARKER_STEM};
use famp_bus::Origin;
use serde_json::Value;

/// Normalize a rendered `Value` to plain text regardless of whether it
/// stayed a structured `Value` (local origin, verbatim) or became a
/// wrapped `String` (gateway/unknown origin).
fn rendered_text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

fn nonce_from_marker_line(line: &str) -> String {
    let key = "nonce=";
    let start = line
        .find(key)
        .unwrap_or_else(|| panic!("no nonce= found in marker line: {line}"))
        + key.len();
    let rest = &line[start..];
    let end = rest.find(' ').unwrap_or(rest.len());
    rest[..end].to_string()
}

/// The three universal behaviors every corpus case must satisfy at
/// gateway origin:
///   - exactly 2 occurrences of the real marker stem (the two genuine
///     markers) — any attacker-forged marker, however encoded or
///     positioned, must never add a 3rd occurrence;
///   - the rendered text opens with the real opening marker (nothing
///     attacker-controlled appears before it);
///   - the rendered text closes with the real closing marker, and its
///     nonce matches the opening marker's nonce (nothing
///     attacker-controlled appears after it).
fn assert_enclosed_and_matching_nonce(case_id: &str, surface: &str, text: &str) {
    let occurrences = text.matches(MARKER_STEM).count();
    assert_eq!(
        occurrences, 2,
        "case {case_id} ({surface}): expected exactly 2 real marker occurrences \
         (open+close); a forged or unneutralized stem escaped the wrap. \
         Got {occurrences}. Text:\n{text}"
    );

    let open_prefix = format!("--- {MARKER_STEM} origin=");
    assert!(
        text.starts_with(&open_prefix),
        "case {case_id} ({surface}): attacker-controlled text appeared before \
         the real opening marker. Text:\n{text}"
    );

    let first_line = text.lines().next().unwrap_or(text);
    let last_line = text.lines().last().unwrap_or(text);
    let close_prefix = format!("--- END {MARKER_STEM} nonce=");
    assert!(
        last_line.starts_with(&close_prefix) && last_line.ends_with(" ---"),
        "case {case_id} ({surface}): rendered output must end with the real \
         closing marker; attacker-controlled text may have appeared after it. \
         Text:\n{text}"
    );

    let open_nonce = nonce_from_marker_line(first_line);
    let close_nonce = nonce_from_marker_line(last_line);
    assert_eq!(
        open_nonce, close_nonce,
        "case {case_id} ({surface}): open/close nonce mismatch. Text:\n{text}"
    );
    assert!(
        !open_nonce.is_empty(),
        "case {case_id} ({surface}): nonce must be non-empty"
    );
}

/// Drive `case.body` through the production render helper at both
/// `Origin::Gateway` (must be enclosed) and `Origin::Local` (must be
/// byte-identical passthrough), for both the `Value`-typed entry point
/// (`render_envelope_body`, reached by 6 of 7 surfaces) and the
/// text-typed entry point (`render_body_text`, reached by CLI `register
/// --tail`).
pub fn run_all_surfaces(case: &CorpusCase) {
    let gateway_value = render_envelope_body(Origin::Gateway, &case.body);
    let gateway_value_text = rendered_text(&gateway_value);
    assert_enclosed_and_matching_nonce(
        case.id,
        "render_envelope_body/gateway",
        &gateway_value_text,
    );

    let local_value = render_envelope_body(Origin::Local, &case.body);
    assert_eq!(
        local_value, case.body,
        "case {}: local-origin Value must render byte-identical",
        case.id
    );

    let body_as_text = rendered_text(&case.body);
    let gateway_text = render_body_text(Origin::Gateway, &body_as_text);
    assert_enclosed_and_matching_nonce(case.id, "render_body_text/gateway", &gateway_text);

    let local_text = render_body_text(Origin::Local, &body_as_text);
    assert_eq!(
        local_text, body_as_text,
        "case {}: local-origin text must render byte-identical",
        case.id
    );
}

/// QUAR-06 widened to the full corpus: build an `AwaitOutcome` carrying
/// `case.body` and confirm the wake notification payload
/// (`hook::emit::emit_block_decision_at`, the real path behind
/// `famp-await.sh`) never contains it.
///
/// Trivially-short/empty bodies (empty string, `null`, `""`) are skipped
/// — asserting their absence would be vacuous (a 4-char-or-shorter
/// substring is likely to "not be found" regardless of whether the
/// mechanism works, producing a false sense of coverage).
pub fn assert_wake_payload_excludes_case_body(case: &CorpusCase) {
    let Some(signature) = wake_signature(case) else {
        return;
    };

    let outcome = famp::cli::await_cmd::AwaitOutcome {
        envelopes: vec![serde_json::json!({
            "from": "agent:example.test/attacker",
            "body": case.body.clone(),
        })],
        mailbox: Some(famp_bus::MailboxName::Agent("bob".into())),
        next_offset: Some(1),
        timed_out: false,
        diagnostic: None,
        aborted: false,
    };
    let dead_sock =
        std::env::temp_dir().join(format!("famp-quarantine-corpus-wake-{}.sock", case.id));
    let mut buf = Vec::new();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _ = rt.block_on(famp::cli::hook::emit::emit_block_decision_at(
        &dead_sock, &outcome, "bob", &mut buf,
    ));
    let payload = String::from_utf8(buf).expect("wake payload is valid UTF-8");

    assert!(
        !payload.contains(&signature),
        "case {}: wake payload must never carry the case body text; \
         signature {signature:?} found in: {payload}",
        case.id
    );
}

/// The substring to search for in the wake payload — the case body's
/// text if it's a string of meaningful length, otherwise its compact
/// JSON serialization (also only if of meaningful length). Returns
/// `None` for trivially short bodies where containment is not a
/// meaningful signal.
fn wake_signature(case: &CorpusCase) -> Option<String> {
    let signature = match &case.body {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    if signature.len() >= 4 {
        Some(signature)
    } else {
        None
    }
}
