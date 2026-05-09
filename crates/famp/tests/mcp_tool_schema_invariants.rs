//! MCP tool schema invariants — Anthropic API compatibility gate.
//!
//! The Anthropic API rejects tools whose `inputSchema` contains `oneOf`,
//! `allOf`, or `anyOf` at the top level (400 error). This test catches
//! regressions before they reach agent sessions.

#![allow(unused_crate_dependencies, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::mcp_harness::Harness;

#[test]
fn tool_schemas_have_no_forbidden_top_level_keywords() {
    let mut h = Harness::with_agents(&["probe"]);

    let resp = h.call("tools/list", &serde_json::json!({}));
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools/list must return a tools array");

    assert!(!tools.is_empty(), "tools/list returned no tools");

    for tool in tools {
        let name = tool["name"].as_str().unwrap_or("<unknown>");
        let schema = &tool["inputSchema"];
        for kw in ["oneOf", "allOf", "anyOf"] {
            assert!(
                schema.get(kw).is_none(),
                "tool '{name}' has forbidden top-level '{kw}' in inputSchema \
                 — Anthropic API will reject this with HTTP 400"
            );
        }
    }
}
