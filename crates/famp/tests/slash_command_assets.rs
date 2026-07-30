#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 05 CC-07 regression gate, extended (quick task 260729-ur8) into a
//! general MCP-tool-schema gate for every `crates/famp/assets/slash_commands/*.md`
//! asset.
//!
//! Each asset is checked against the MCP tool registry, which this module
//! parses as TEXT out of `src/cli/mcp/server.rs` — `tool_descriptors()` is
//! private and this is an integration-test crate, so slicing the `json!`
//! literal out of the source is the only way to reach it from here. The
//! registry size is pinned in code by `slash_command_asset_harness_is_not_vacuous`
//! rather than restated in this prose comment, so a registry change cannot
//! silently drift this doc block. See the paired unit test
//! `tool_descriptors_has_exactly_twelve_named_tools` inside `server.rs` — a
//! deliberate registry change should update both gates together.
//!
//! The original CC-07 `famp-who.md` tests below
//! (`test_famp_who_does_not_reference_unregistered_tool`,
//! `test_famp_who_allowed_tools_lists_only_famp_peers`,
//! `test_famp_who_argument_hint_present`) are retained unmodified. See
//! `.planning/v0.9-MILESTONE-AUDIT.md` for the originating CC-07 evidence.

use std::collections::BTreeSet;

const FAMP_WHO_MD: &str = include_str!("../assets/slash_commands/famp-who.md");

#[test]
fn test_famp_who_does_not_reference_unregistered_tool() {
    assert!(
        !FAMP_WHO_MD.contains("famp_sessions"),
        "famp-who.md must not reference famp_sessions \
         (not a registered MCP tool — see CC-07 / v0.9-MILESTONE-AUDIT.md)"
    );
}

#[test]
fn test_famp_who_allowed_tools_lists_only_famp_peers() {
    let line = FAMP_WHO_MD
        .lines()
        .find(|l| l.starts_with("allowed-tools:"))
        .expect("allowed-tools frontmatter line missing");
    let tools: std::collections::BTreeSet<&str> = line
        .trim_start_matches("allowed-tools:")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let expected: std::collections::BTreeSet<&str> =
        std::iter::once("mcp__famp__famp_peers").collect();
    assert_eq!(tools, expected, "CC-07: allowed-tools surface drift");
}

#[test]
fn test_famp_who_argument_hint_present() {
    assert!(
        FAMP_WHO_MD.contains("argument-hint: [#channel?]"),
        "argument-hint contract changed — CC-07 expects [#channel?]"
    );
}

// ── registry parser (quick task 260729-ur8) ────────────────────────────────

/// The whole `server.rs` source, embedded as text. `tool_descriptors()` is
/// private, so this integration-test crate cannot call it directly — parsing
/// the `json!` literal back out of the source text is the only reachable
/// source of truth for the tool registry.
const SERVER_RS: &str = include_str!("../src/cli/mcp/server.rs");

/// Slice the `json!([...])` array literal out of `fn tool_descriptors()` and
/// parse it as strict JSON. Every anchor lookup panics loudly, naming the
/// anchor that went missing, if `server.rs` is reshaped — a silent partial
/// match here would make every downstream test pass vacuously.
fn registry() -> serde_json::Value {
    const FN_ANCHOR: &str = "fn tool_descriptors() -> serde_json::Value {";
    const JSON_ANCHOR: &str = "serde_json::json!(";
    const TERMINATOR: &str = "\n    ])\n}";

    let fn_start = SERVER_RS.find(FN_ANCHOR).expect(
        "server.rs no longer contains `fn tool_descriptors() -> serde_json::Value {` — the \
         registry parser anchor moved; update registry() in slash_command_assets.rs",
    );
    let after_fn = &SERVER_RS[fn_start..];

    let json_offset = after_fn.find(JSON_ANCHOR).expect(
        "tool_descriptors()'s body no longer contains `serde_json::json!(` — the registry \
         parser anchor moved; update registry() in slash_command_assets.rs",
    );
    let slice_start = fn_start + json_offset + JSON_ANCHOR.len();

    let terminator_offset = SERVER_RS[slice_start..].find(TERMINATOR).expect(
        "no `\\n    ])\\n}` terminator found after tool_descriptors()'s json! literal — the \
         registry parser anchor moved; update registry() in slash_command_assets.rs",
    );
    // The terminator's first 5 bytes are the newline + four indent spaces, so
    // index+5 lands on the closing `]` and index+6 is one-past it.
    let slice_end = slice_start + terminator_offset + 6;

    let body = &SERVER_RS[slice_start..slice_end];
    assert!(
        body.starts_with('[') && body.ends_with(']'),
        "sliced tool_descriptors() body does not start with `[` and end with `]` — the \
         registry parser anchor moved; update registry() in slash_command_assets.rs"
    );

    serde_json::from_str(body).expect(
        "failed to parse tool_descriptors()'s json! literal as strict JSON — check for a \
         non-JSON Rust expression (interpolation, comment, trailing comma) inside the literal",
    )
}

fn tool_names(registry: &serde_json::Value) -> BTreeSet<String> {
    registry
        .as_array()
        .expect("registry() must return a JSON array")
        .iter()
        .filter_map(|entry| entry.get("name").and_then(serde_json::Value::as_str))
        .map(String::from)
        .collect()
}

fn properties_of(registry: &serde_json::Value, tool: &str) -> BTreeSet<String> {
    registry
        .as_array()
        .expect("registry() must return a JSON array")
        .iter()
        .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(tool))
        .and_then(|entry| entry.pointer("/inputSchema/properties"))
        .and_then(serde_json::Value::as_object)
        .map(|props| props.keys().cloned().collect())
        .unwrap_or_default()
}

fn required_of(registry: &serde_json::Value, tool: &str) -> BTreeSet<String> {
    registry
        .as_array()
        .expect("registry() must return a JSON array")
        .iter()
        .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(tool))
        .and_then(|entry| entry.pointer("/inputSchema/required"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// One entry per `assets/slash_commands/*.md` asset: file name paired with
/// its embedded content. `slash_command_asset_harness_is_not_vacuous` checks
/// this list's length against the directory's actual `.md` count, so a newly
/// added asset that is not registered here fails loudly instead of silently
/// escaping every gate below.
const ASSETS: &[(&str, &str)] = &[
    (
        "famp-channel.md",
        include_str!("../assets/slash_commands/famp-channel.md"),
    ),
    (
        "famp-inbox.md",
        include_str!("../assets/slash_commands/famp-inbox.md"),
    ),
    (
        "famp-join.md",
        include_str!("../assets/slash_commands/famp-join.md"),
    ),
    (
        "famp-leave.md",
        include_str!("../assets/slash_commands/famp-leave.md"),
    ),
    (
        "famp-register.md",
        include_str!("../assets/slash_commands/famp-register.md"),
    ),
    (
        "famp-send.md",
        include_str!("../assets/slash_commands/famp-send.md"),
    ),
    (
        "famp-who.md",
        include_str!("../assets/slash_commands/famp-who.md"),
    ),
];

/// Every distinct MCP tool name (`mcp__famp__<tool>`) referenced anywhere in
/// `text`.
fn referenced_tools(text: &str) -> BTreeSet<String> {
    const MARKER: &str = "mcp__famp__";
    let mut tools = BTreeSet::new();
    for (idx, _) in text.match_indices(MARKER) {
        let after = &text[idx + MARKER.len()..];
        let end = after
            .char_indices()
            .find(|(_, c)| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_'))
            .map_or(after.len(), |(i, _)| i);
        if end > 0 {
            tools.insert(after[..end].to_string());
        }
    }
    tools
}

/// Number of `.md` files physically present in `assets/slash_commands/`,
/// computed at test time via `Path::extension()` (never `.ends_with(".md")`
/// — `just lint` denies `case_sensitive_file_extension_comparisons`).
fn asset_dir_md_count() -> usize {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/slash_commands");
    std::fs::read_dir(dir)
        .expect("assets/slash_commands directory must exist")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("md")))
        .count()
}

#[test]
fn slash_command_asset_harness_is_not_vacuous() {
    let dir_count = asset_dir_md_count();
    let listed_count = ASSETS.len();
    assert_eq!(
        dir_count, listed_count,
        "assets/slash_commands/ holds {dir_count} .md files but the ASSETS const in \
         slash_command_assets.rs lists {listed_count}; register the new/removed asset in \
         ASSETS or this harness silently stops covering it"
    );

    let registry = registry();
    let names = tool_names(&registry);
    let name_count = names.len();
    assert_eq!(
        name_count, 12,
        "parsed registry has {name_count} tool names, expected 12 — cross-check against \
         server.rs's own tool_descriptors_has_exactly_twelve_named_tools unit test; a \
         deliberate registry change must update both"
    );

    for (file, text) in ASSETS {
        let refs = referenced_tools(text);
        let ref_count = refs.len();
        assert_eq!(
            ref_count, 1,
            "{file} references {ref_count} distinct MCP tools ({refs:?}); this harness \
             assumes one tool per asset for unambiguous per-key validation — add an explicit \
             asset-to-tool mapping before shipping a multi-tool asset"
        );
    }
}

/// The gates below read two Markdown idioms: `- \`key\`: \`value\`` bullets and
/// inline `` `key: "value"` `` spans, with tools named as `mcp__famp__<tool>`.
/// Anything else is INVISIBLE to them — which means the original bug could
/// re-ship verbatim in different formatting and CI would stay green:
///
/// ```text
///     ```json
///     {"to": {"kind": "agent", "name": "$1"}}     <- no backtick spans, unseen
///     ```
///     call the `famp_sessions` tool                <- no mcp__famp__, unseen
/// ```
///
/// Rather than grow the extractors to chase every phrasing (each widening adds
/// false-positive surface, and a gate that red-lights CORRECT assets is a gate
/// that gets deleted), this pins the assets to the readable idiom. Writing an
/// unparseable form fails here with instructions, instead of passing silently.
#[test]
fn slash_command_assets_stay_in_the_gate_readable_idiom() {
    for (file, text) in ASSETS {
        assert!(
            !text.contains("```"),
            "{file} contains a fenced code block. The argument-key gates only read \
             `- \\`key\\`: \\`value\\`` bullets and inline \\`key: \"value\"\\` spans, so a call \
             shape shown in a fence is unvalidated — express the arguments as bullets instead."
        );

        for (idx, _) in text.match_indices("famp_") {
            let before = &text[..idx];
            assert!(
                before.ends_with("mcp__") || before.ends_with("mcp__famp__"),
                "{file} names a tool as a bare `famp_…` at byte {idx}; the tool gates only \
                 match the `mcp__famp__<tool>` form, so a bare name escapes the \
                 registry/dispatch checks — write mcp__famp__<tool>."
            );
        }
    }
}

#[test]
fn slash_command_assets_reference_only_dispatchable_mcp_tools() {
    let registry = registry();
    let names = tool_names(&registry);

    for (file, text) in ASSETS {
        for tool in referenced_tools(text) {
            assert!(
                names.contains(&tool),
                "{file} references mcp__famp__{tool}, which is not a descriptor in the \
                 registry parsed from server.rs's tool_descriptors()"
            );
            let arm = format!("\"{tool}\" =>");
            assert!(
                SERVER_RS.contains(&arm),
                "{file} references mcp__famp__{tool}, which has a registry descriptor but no \
                 `\"{tool}\" =>` arm in dispatch_tool — the asset would prescribe a call the \
                 server cannot dispatch"
            );
        }
    }
}

// ── argument-key + count-claim gates (quick task 260729-ur8, Task 2) ───────

fn is_key_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    first_ok && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Every argument key an asset prescribes for its single referenced tool.
///
/// Rule A (bullet form, e.g. famp-send.md's `- \`peer\`: \`$1\`` bullets): if
/// the trimmed line starts with `- ` and contains `": "`, take the portion
/// before the FIRST `": "` and collect every odd-index backtick span of that
/// portion that is a valid identifier. Taking the head before the first
/// `": "` is what makes a broken `to`-with-JSON-blob bullet yield `to` rather
/// than the keys nested inside its JSON value.
///
/// Rule B (inline form, e.g. famp-join.md's `` `channel: "$ARGUMENTS"` ``):
/// for every odd-index backtick span on any line, if the span contains `:`
/// and the portion before the first `:` is a valid identifier, that portion
/// is a key. This deliberately skips spans like famp-who.md's
/// `{ online: [...] }` (head is not an identifier) and bare spans with no
/// colon such as tool names or `include_terminal` in prose.
fn prescribed_keys(text: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();

    for line in text.lines() {
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(colon_idx) = rest.find(": ") {
                let head = &rest[..colon_idx];
                for (i, span) in head.split('`').enumerate() {
                    if i % 2 == 1 && is_key_ident(span) {
                        keys.insert(span.to_string());
                    }
                }
            }
        }

        for (i, span) in line.split('`').enumerate() {
            if i % 2 == 1 {
                if let Some(colon_idx) = span.find(':') {
                    let head = &span[..colon_idx];
                    if is_key_ident(head) {
                        keys.insert(head.to_string());
                    }
                }
            }
        }
    }

    keys
}

fn single_referenced_tool(file: &str, text: &str) -> String {
    referenced_tools(text)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{file} references no MCP tool"))
}

#[test]
fn slash_command_assets_prescribe_only_real_argument_keys() {
    let registry = registry();

    for (file, text) in ASSETS {
        let tool = single_referenced_tool(file, text);
        let allowed = properties_of(&registry, &tool);
        for key in prescribed_keys(text) {
            assert!(
                allowed.contains(&key),
                "{file} prescribes key `{key}` for {tool}, but {tool}'s \
                 inputSchema.properties only accepts {allowed:?}"
            );
        }
    }
}

#[test]
fn slash_command_assets_prescribe_every_required_argument_key() {
    let registry = registry();

    for (file, text) in ASSETS {
        let tool = single_referenced_tool(file, text);
        let prescribed = prescribed_keys(text);
        // famp-register.md expresses `identity` as prose rather than a
        // prescribed key (GT-4): an unconditional required-key check would
        // false-fail on it, so this gate only applies once an asset
        // prescribes at least one key.
        if prescribed.is_empty() {
            continue;
        }
        let required = required_of(&registry, &tool);
        for key in required {
            assert!(
                prescribed.contains(&key),
                "{file} prescribes keys {prescribed:?} for {tool} but omits required key \
                 `{key}`"
            );
        }
    }
}

/// English number words 1–20, so a count spelled out in prose ("exactly eight
/// tools") is gated exactly like a digit one ("exactly 8 tools").
///
/// The original shipped bug — `famp-who.md` claiming an "8-tool MCP surface"
/// after the registry reached 12 — was a digit, but nothing stops the next
/// rewrite from spelling it. A digit-only scanner would `continue` past a
/// spelled claim silently, which is the same class of hole this whole file
/// exists to close. Verified: with a digit-only scanner, an asset reading
/// "exactly eight tools" passed the suite 8/8.
fn spelled_number(word: &str) -> Option<usize> {
    const WORDS: [(&str, usize); 20] = [
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        ("twenty", 20),
    ];
    let lower = word.to_ascii_lowercase();
    WORDS.iter().find(|(w, _)| *w == lower).map(|(_, n)| *n)
}

/// The tool count `text` claims immediately before a `-tool` / ` tools`
/// marker, as a digit run (`12`) or an English number word (`twelve`).
///
/// `None` means no countable claim, which is how inert phrases like
/// `allowed-tools`, `tool listed`, and `mcp__famp__famp_peers` tool` stay out
/// of the assertion below.
fn claimed_count(before: &str) -> Option<usize> {
    // `trim_end_matches` returns a prefix of `before`, so its `len()` is always
    // a valid char boundary. An earlier version used
    // `rfind(|c| !pred(c)).map_or(0, |i| i + 1)`, which PANICS whenever the
    // first non-matching char is multi-byte: `rfind` yields that char's START
    // byte, so `+ 1` lands inside it. Markdown prose is full of em-dashes and
    // curly quotes, and `The surface has grown—12 tools are live.` reproduced
    // it as `byte index 488 is not a char boundary; it is inside '—'`. Pinned
    // by `claimed_count_survives_multibyte_punctuation_before_a_count`.
    let digit_run = trailing_run(before, |c| c.is_ascii_digit());
    if !digit_run.is_empty() {
        // Reject a digit run that is part of a larger token rather than a
        // standalone count: a version (`v0.9 tools`), a date
        // (`2026-07-29 tools`), or an identifier (`phase05 tools`). Without
        // this, ordinary prose red-lights the gate claiming a "9-tool
        // surface", which is a false failure pointing at a count nobody wrote.
        let head = &before[..before.len() - digit_run.len()];
        if head
            .chars()
            .next_back()
            .is_some_and(|c| c == '.' || c == '-' || c.is_ascii_alphanumeric())
        {
            return None;
        }
        return Some(
            digit_run
                .parse()
                .unwrap_or_else(|_| panic!("`{digit_run}` overflows usize as a tool count")),
        );
    }

    // No digit run — fall back to a trailing English word.
    spelled_number(trailing_run(before, |c| c.is_ascii_alphabetic()))
}

/// The longest suffix of `s` whose chars all satisfy `class`.
///
/// Boundary-safe by construction: `trim_end_matches` returns a prefix of `s`,
/// so slicing at its `len()` can never split a multi-byte char.
fn trailing_run(s: &str, class: impl Fn(char) -> bool) -> &str {
    &s[s.trim_end_matches(|c: char| class(c)).len()..]
}

#[test]
fn slash_command_asset_tool_count_claims_match_registry() {
    let registry = registry();
    let expected = tool_names(&registry).len();
    let mut claims_found = 0usize;

    for (file, text) in ASSETS {
        for marker in ["-tool", " tools"] {
            for (idx, _) in text.match_indices(marker) {
                let Some(claim) = claimed_count(&text[..idx]) else {
                    continue;
                };
                claims_found += 1;
                assert_eq!(
                    claim, expected,
                    "{file} claims a {claim}-tool surface (near `{marker}`) but the registry \
                     parsed from server.rs has {expected} tools"
                );
            }
        }
    }

    assert!(
        claims_found >= 1,
        "no numeric tool-count claim found in any asset — this assertion exists so a future \
         rewrite that removes every countable claim doesn't silently turn this test into a \
         no-op; if that's intentional, delete this test explicitly instead"
    );
}

/// Guards `claimed_count`'s own logic, so the scanner cannot regress into
/// silently returning `None` for real claims (which would make the test above
/// pass vacuously) or into flagging inert `-tool` substrings.
#[test]
fn claimed_count_reads_digits_and_words_but_ignores_inert_phrases() {
    assert_eq!(claimed_count("The MCP surface is exactly 12"), Some(12));
    assert_eq!(claimed_count("The MCP surface is exactly twelve"), Some(12));
    assert_eq!(claimed_count("claimed an 8"), Some(8));
    assert_eq!(claimed_count("claimed an Eight"), Some(8));
    // Inert: these are the real substrings that precede `-tool` / ` tools`
    // in the shipped assets and must NOT be read as counts.
    assert_eq!(claimed_count("allowed"), None);
    assert_eq!(claimed_count("---\nallowed"), None);
    assert_eq!(claimed_count("use only the listed"), None);
    assert_eq!(claimed_count(""), None);
}

/// A digit run that is part of a larger token is not a surface-size claim.
/// Without this guard the gate red-lights correct prose, naming a count the
/// author never wrote — a false failure is how a gate loses its credibility.
#[test]
fn claimed_count_ignores_digits_inside_versions_dates_and_identifiers() {
    assert_eq!(claimed_count("on the v0.9"), None);
    assert_eq!(claimed_count("shipped 2026-07-29"), None);
    assert_eq!(claimed_count("see phase05"), None);
    // ...but a standalone number still counts, including at start-of-text.
    assert_eq!(claimed_count("exactly 12"), Some(12));
    assert_eq!(claimed_count("12"), Some(12));
    assert_eq!(claimed_count("(12"), Some(12));
}

/// Regression: `claimed_count` must not panic on multi-byte punctuation
/// adjacent to the scanned run. The previous `rfind(...) + 1` arithmetic
/// yielded a byte index inside the em-dash and paniced with
/// `byte index N is not a char boundary`, which named a UTF-8 offset instead
/// of the asset — found by adversarial review of this file's own diff.
#[test]
fn claimed_count_survives_multibyte_punctuation_before_a_count() {
    // Em-dash (3 bytes) immediately before a digit run.
    assert_eq!(claimed_count("The surface has grown—12"), Some(12));
    // Curly apostrophe / quote (3 bytes) immediately before a word run.
    assert_eq!(claimed_count("the server’s twelve"), Some(12));
    assert_eq!(claimed_count("the “twelve"), Some(12));
    // Multi-byte char with no countable run after it — must be None, not panic.
    assert_eq!(claimed_count("see the docs —"), None);
    assert_eq!(claimed_count("→"), None);
}
