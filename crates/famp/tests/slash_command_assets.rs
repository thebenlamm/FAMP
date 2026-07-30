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

#[test]
fn slash_command_asset_tool_count_claims_match_registry() {
    let registry = registry();
    let expected = tool_names(&registry).len();
    let mut claims_found = 0usize;

    for (file, text) in ASSETS {
        for marker in ["-tool", " tools"] {
            for (idx, _) in text.match_indices(marker) {
                let before = &text[..idx];
                let digits_start = before
                    .rfind(|c: char| !c.is_ascii_digit())
                    .map_or(0, |i| i + 1);
                let digits = &before[digits_start..];
                if digits.is_empty() {
                    // No trailing digit run — inert phrases like
                    // `allowed-tools` and `tool listed` land here.
                    continue;
                }
                let claim: usize = digits.parse().unwrap_or_else(|_| {
                    panic!("{file}: `{digits}` before `{marker}` is not a valid number")
                });
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
