//! CC-09: README Quick Start fence is <=12 user-visible lines.
//! The literal gate is bytes-of-source within the fence body.
//! Renderer-stable across GitHub + crates.io.
//!
//! The content assertions also pin the *recommended wiring path*. That path is
//! the `famp@famp` plugin, not `famp install-claude-code`: the two register an
//! MCP server under the same name, and a reader who ran both got every FAMP
//! tool twice. Quick Start must name the plugin and warn against the pair.

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

    // Find the first "## Quick Start" header, and bound everything below to
    // that section. Bounding matters: an unbounded fence search silently
    // measures a fence from a LATER section if Quick Start's own fence is ever
    // removed, so the gate would pass while measuring the wrong thing.
    let qs_idx = body
        .find("## Quick Start")
        .expect("README missing '## Quick Start' section");
    let after_qs = &body[qs_idx..];
    let section_end = after_qs[1..]
        .find("\n## ")
        .map_or(after_qs.len(), |i| i + 1);
    let section = &after_qs[..section_end];

    // Extract the body of the first fence with the given tag, searching only
    // within the Quick Start section.
    let fence_body_of = |tag: &str| -> String {
        let open = section.find(tag).unwrap_or_else(|| {
            panic!("Quick Start is missing a {tag} fence\n--- section ---\n{section}\n--- end ---")
        });
        let after_open = &section[open + tag.len()..];
        let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
        let close = after_open
            .find("\n```")
            .unwrap_or_else(|| panic!("Quick Start {tag} fence is not closed"));
        after_open[..close].to_string()
    };

    // CC-09 caps what a Quick Start reader actually sees. BOTH fences count:
    // the ```text one is the recommended (plugin) path and the ```bash one is
    // the CLI fallback. Capping only ```bash — as this gate once did — let the
    // recommended path grow without limit, which is the path most readers use.
    for tag in ["```text", "```bash"] {
        let fence_body = fence_body_of(tag);
        let line_count = fence_body.lines().count();
        assert!(
            line_count <= 12,
            "README Quick Start {tag} fence has {line_count} lines (CC-09 cap: 12)\n--- fence body ---\n{fence_body}\n--- end ---"
        );
    }

    // Sanity: the CLI fence must mention the prebuilt-binary installer
    // (D-01/16-04 amendment from `cargo install famp`, which resolves against
    // crates.io where this project's crate was never published — see D-11
    // below for the earlier `brew install famp` -> `cargo install famp`
    // amendment this one supersedes).
    let bash_fence = fence_body_of("```bash");
    assert!(
        bash_fence.contains("releases/latest/download/famp-installer.sh"),
        "Quick Start must include the prebuilt-binary curl installer \
         (D-01/16-04; `cargo install famp` is broken — famp is not on \
         crates.io)\nactual:\n{bash_fence}"
    );

    // The recommended path's steps live in the ```text fence, not ```bash:
    // they are typed into a Claude Code window, not a shell. Assert the fence
    // itself, so a passing mention in a prose table cannot satisfy the gate.
    let text_fence = fence_body_of("```text");
    for required in ["/plugin install famp@famp", "/famp:setup", "/famp:register"] {
        assert!(
            text_fence.contains(required),
            "Quick Start's recommended-path fence must include `{required}` — \
             the plugin path is only runnable if all of marketplace-add, \
             install, setup, and register are shown\nactual:\n{text_fence}"
        );
    }

    // `/famp:register` is an MCP tool call, so a window open at install time
    // has not loaded the server. Quick Start omitting the restart sends the
    // reader straight into a failing step-4.
    assert!(
        text_fence.to_lowercase().contains("restart"),
        "Quick Start's recommended-path fence must tell the reader to restart \
         Claude Code before /famp:register (the MCP server only loads at \
         window start)\nactual:\n{text_fence}"
    );

    // The regression this gate exists to prevent: the plugin and
    // `famp install-claude-code` register the same MCP server name, so running
    // both yields 24 tools instead of 12 and four Stop hooks instead of two.
    // Quick Start must say so — a reader who follows it verbatim after
    // installing the plugin would otherwise land in exactly that broken state.
    assert!(
        section.contains("Do not also run `famp install-claude-code`"),
        "Quick Start must warn against running the plugin and \
         `famp install-claude-code` together (duplicate MCP server: 24 tools \
         instead of 12, 4 Stop hooks instead of 2)\nactual:\n{section}"
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
