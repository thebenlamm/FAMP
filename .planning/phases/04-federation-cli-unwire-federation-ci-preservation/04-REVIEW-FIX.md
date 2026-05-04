---
phase: 04-federation-cli-unwire-federation-ci-preservation
source: 04-REVIEW.md
status: resolved
fixed: 4
intentional: 1
created: 2026-05-04
---

# Phase 04 Code Review Fixes

## Fixed

- **CR-02:** Updated the `famp_peers` MCP tool descriptor to match v0.9 behavior: list currently online registered identities with an empty input schema. The tool no longer advertises peer-add or `peers.toml` inputs.
- **WR-01:** Replaced README onboarding that pointed users at archived `scripts/famp-local` commands with live v0.9 local-bus CLI commands.
- **WR-02:** Removed listener-daemon redeploy guidance from the current README and replaced it with broker lifecycle guidance.
- **WR-03:** Corrected the migration guide: `famp install-claude-code` writes user-scope Claude Code config; project-scope `.mcp.json` cleanup is manual.

## Intentional

- **CR-01:** No code change. `famp info` is intentionally retained by Plan 04-08: its must-haves require `info` to stay live and self-contained by inlining `PeerCard` and `load_identity`. The stale federation verbs were deleted; `info` is kept as the surviving diagnostic/identity surface.

## Verification

- `cargo clippy -p famp --all-targets -- -D warnings`: PASS
- `cargo nextest run -p famp -E 'test(mcp)' --no-fail-fast`: PASS, 10 passed
- Stale-doc grep for archived-wrapper onboarding, listener redeploy, and stale `famp_peers` descriptor strings: PASS, no matches
