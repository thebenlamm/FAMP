//! Every host packaging's `plugin.json` must declare the workspace version.
//!
//! The three manifests are hand-maintained -- `scripts/gen-plugin.sh`
//! regenerates only `commands/` and `hooks/`, so nothing was keeping their
//! `version` in step with `[workspace.package].version`. They drifted to
//! `1.0.0` against a `1.1.0-rc.1` workspace, which is not cosmetic: the
//! manifest version is what the host's plugin list displays, so a user
//! reporting a bug names a version that does not identify the code they are
//! running, and `/plugin update` cannot express that anything changed.
//!
//! This is a drift gate, not a style check -- it fails the build rather than
//! letting the next release repeat it.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::path::PathBuf;

/// Repo-relative paths of every host packaging manifest.
const MANIFESTS: &[&str] = &[
    "plugins/claude-code/.claude-plugin/plugin.json",
    "plugins/codex/.codex-plugin/plugin.json",
    "plugins/grok/plugin.json",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Parse `[workspace.package] version = "..."` out of the root `Cargo.toml`.
///
/// Deliberately hand-rolled rather than pulling in a TOML dependency for one
/// field: this gate must not add a dep to the test graph.
fn workspace_version(root: &std::path::Path) -> String {
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let mut in_section = false;
    for line in cargo.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[workspace.package]";
            continue;
        }
        if in_section {
            if let Some(rest) = t.strip_prefix("version") {
                if let Some(v) = rest.split('"').nth(1) {
                    return v.to_string();
                }
            }
        }
    }
    panic!("could not find [workspace.package].version in Cargo.toml");
}

#[test]
fn plugin_manifests_declare_the_workspace_version() {
    let root = repo_root();
    let expected = workspace_version(&root);

    for rel in MANIFESTS {
        let path = root.join(rel);
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        let json: serde_json::Value = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));

        let got = json
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{} has no string `version` field", path.display()));

        assert_eq!(
            got, expected,
            "{rel} declares version {got}, but [workspace.package].version is \
             {expected} -- bump the manifest with the workspace so the version \
             a host displays identifies the code it is running"
        );
    }
}

/// Guard against the gate going vacuous: if the manifest list is ever emptied
/// or a path is renamed, the loop above would pass by iterating nothing.
#[test]
fn every_manifest_on_disk_is_covered_by_the_gate() {
    // Emptying MANIFESTS is caught by the length comparison at the end: the
    // manifests still exist on disk, so `found` would be non-empty against a
    // zero-length list. No separate is_empty assert -- MANIFESTS is a const,
    // so clippy resolves that at compile time (const_is_empty).
    let root = repo_root();

    let mut found = Vec::new();
    for host in ["claude-code", "codex", "grok"] {
        let dir = root.join("plugins").join(host);
        if !dir.exists() {
            continue;
        }
        for candidate in [
            dir.join("plugin.json"),
            dir.join(".claude-plugin").join("plugin.json"),
            dir.join(".codex-plugin").join("plugin.json"),
            dir.join(".grok-plugin").join("plugin.json"),
        ] {
            if candidate.exists() {
                found.push(candidate);
            }
        }
    }

    let covered: Vec<PathBuf> = MANIFESTS.iter().map(|r| root.join(r)).collect();
    for f in &found {
        assert!(
            covered.contains(f),
            "{} exists on disk but is not listed in MANIFESTS -- the version \
             gate would silently skip it",
            f.display()
        );
    }
    assert_eq!(
        found.len(),
        MANIFESTS.len(),
        "MANIFESTS lists {} paths but {} exist on disk",
        MANIFESTS.len(),
        found.len()
    );
}
