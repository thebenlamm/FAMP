//! CC-09: README Quick Start fence is <=12 user-visible lines.
//! The literal gate is bytes-of-source within the fence body.
//! Renderer-stable across GitHub + crates.io.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::path::PathBuf;

#[test]
fn readme_quick_start_fence_is_at_most_12_lines() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap()
        .to_path_buf(); // repo root
    let readme = root.join("README.md");
    let body = std::fs::read_to_string(&readme)
        .unwrap_or_else(|e| panic!("could not read README.md at {}: {e}", readme.display()));

    // Find the first "## Quick Start" header.
    let qs_idx = body
        .find("## Quick Start")
        .expect("README missing '## Quick Start' section");
    let after_qs = &body[qs_idx..];

    // Find the first ```bash fence opening after that header.
    let fence_open = after_qs
        .find("```bash")
        .expect("Quick Start missing ```bash fence");
    let after_open = &after_qs[fence_open + 7..]; // skip ```bash
                                                  // Skip the trailing newline after ```bash.
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    // Find the closing ```.
    let fence_close = after_open
        .find("\n```")
        .expect("Quick Start fence not closed");
    let fence_body = &after_open[..fence_close];

    let line_count = fence_body.lines().count();
    assert!(
        line_count <= 12,
        "README Quick Start fence has {line_count} lines (CC-09 cap: 12)\n--- fence body ---\n{fence_body}\n--- end ---"
    );

    // Sanity: the block must mention `cargo install famp` (D-11 amendment from
    // `brew install famp`) and `famp install-claude-code`.
    assert!(
        fence_body.contains("cargo install famp"),
        "Quick Start must include `cargo install famp` (D-11)\nactual:\n{fence_body}"
    );
    assert!(
        fence_body.contains("famp install-claude-code"),
        "Quick Start must include `famp install-claude-code`\nactual:\n{fence_body}"
    );
    assert!(
        fence_body.contains("/famp-register"),
        "Quick Start must demonstrate /famp-register\nactual:\n{fence_body}"
    );
}

#[test]
fn readme_quick_start_does_not_reference_brew_install_famp() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    // D-11: brew install famp was replaced by cargo install famp.
    assert!(
        !readme.contains("brew install famp"),
        "README still references `brew install famp` (D-11 amendment requires `cargo install famp`)"
    );
}

#[test]
fn readme_quick_start_does_not_reference_famp_msg() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    // D-05: /famp-msg was renamed to /famp-send.
    assert!(
        !readme.contains("/famp-msg"),
        "README still references `/famp-msg` (D-05 amendment requires `/famp-send`)"
    );
}
