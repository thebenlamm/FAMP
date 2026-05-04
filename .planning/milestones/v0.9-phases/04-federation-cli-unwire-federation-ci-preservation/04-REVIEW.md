---
phase: 04-federation-cli-unwire-federation-ci-preservation
reviewed: 2026-05-04T02:16:08Z
depth: standard
files_reviewed: 36
files_reviewed_list:
  - ARCHITECTURE.md
  - CLAUDE.md
  - Cargo.lock
  - Cargo.toml
  - README.md
  - crates/famp/Cargo.toml
  - crates/famp/examples/cross_machine_two_agents.rs
  - crates/famp/examples/personal_two_agents.rs
  - crates/famp/src/bin/famp.rs
  - crates/famp/src/cli/broker/mod.rs
  - crates/famp/src/cli/error.rs
  - crates/famp/src/cli/info.rs
  - crates/famp/src/cli/install/claude_code.rs
  - crates/famp/src/cli/mcp/error_kind.rs
  - crates/famp/src/cli/mod.rs
  - crates/famp/src/cli/register.rs
  - crates/famp/src/cli/send/mod.rs
  - crates/famp/src/cli/uninstall/claude_code.rs
  - crates/famp/src/cli/util.rs
  - crates/famp/src/lib.rs
  - crates/famp/tests/adversarial/harness.rs
  - crates/famp/tests/adversarial/http.rs
  - crates/famp/tests/adversarial/memory.rs
  - crates/famp/tests/cli_help_invariant.rs
  - crates/famp/tests/clierror_fsm_transition_display.rs
  - crates/famp/tests/common/cycle_driver.rs
  - crates/famp/tests/common/listen_harness.rs
  - crates/famp/tests/common/mcp_harness.rs
  - crates/famp/tests/common/mod.rs
  - crates/famp/tests/e2e_two_daemons.rs
  - crates/famp/tests/e2e_two_daemons_adversarial.rs
  - crates/famp/tests/famp_local_wire_migration.rs
  - crates/famp/tests/hook_subcommand.rs
  - crates/famp/tests/mcp_error_kind_exhaustive.rs
  - crates/famp/tests/mcp_malformed_input.rs
  - crates/famp/tests/runtime_unit.rs
findings:
  critical: 2
  warning: 3
  info: 0
  total: 5
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-05-04T02:16:08Z
**Depth:** standard
**Files Reviewed:** 36
**Status:** issues_found

## Summary

Reviewed the Phase 04 federation CLI deletion and preservation changes. The core deleted verbs are gone from top-level help, and `cargo test -p famp --no-run` compiles. Targeted MCP tests initially failed inside the sandbox because broker `setsid()` was denied; rerunning `mcp_malformed_input` and `mcp_register_whoami` outside the sandbox passed. The remaining issues are real surface-contract regressions: stale federation peer-card and peer-add affordances are still advertised, and user-facing docs still direct users to archived or non-existent local wrapper flows.

## Critical Issues

### CR-01: Deleted Federation Peer-Card Command Still Ships

**File:** `crates/famp/src/cli/mod.rs:59`
**Issue:** **BLOCKER** - Phase 04 removes `init/setup/listen/peer` and says peer cards are v1.0 federation-internal, but the top-level CLI still advertises and dispatches `famp info` as "Output this agent's peer card". The implementation still resolves `FAMP_HOME`, requires the old six-file identity layout, and emits an HTTPS endpoint from `listen_addr` (`crates/famp/src/cli/info.rs:39`, `crates/famp/src/cli/info.rs:86`, `crates/famp/src/cli/info.rs:119`). In v0.9 there is no shipped `famp init/setup/listen` path to create that identity, so this is a stale federation surface left in production help.
**Fix:**
```rust
// Remove Info from Commands and the dispatch arm, or hide it behind a v1-only
// feature until the federation gateway reintroduces peer cards.
// Also remove/relocate cli/info.rs with the deferred v1 test/doc surface.
```

### CR-02: MCP Advertises `famp_peers` Add/Peers-TOML API That No Longer Exists

**File:** `crates/famp/src/cli/mcp/server.rs:78`
**Issue:** **BLOCKER** - `tools/list` still describes `famp_peers` as "List or add peers in peers.toml" and exposes an `action` enum with `"add"` plus `alias/endpoint/pubkey/principal` fields. The actual implementation ignores all input and always sends `BusMessage::Sessions`, returning only `{ "online": [...] }` (`crates/famp/src/cli/mcp/tools/peers.rs:22`, `crates/famp/src/cli/mcp/tools/peers.rs:43`). An MCP client can follow the advertised schema, call `famp_peers` with `action=add`, receive a success response, and no peer is added.
**Fix:**
```rust
// server.rs descriptor should match the v0.9 behavior:
{
    "name": "famp_peers",
    "description": "List currently online registered identities.",
    "inputSchema": { "type": "object", "properties": {} }
}

// Or explicitly reject unsupported actions in tools/peers.rs before returning SessionsOk.
```

## Warnings

### WR-01: README Still Instructs Users To Run Archived `scripts/famp-local`

**File:** `README.md:160`
**Issue:** **WARNING** - The v0.9 README first presents the new local bus path, then immediately tells users to run `scripts/famp-local wire` and lists `famp-local wire/unwire/send/inbox/status/stop/...` as the "Full CLI" (`README.md:163`, `README.md:170`). The same section then says the wrapper moved to history (`README.md:182`). This makes the published onboarding self-contradictory and points users at a path that is no longer the live v0.9 surface.
**Fix:** Replace this block with the actual v0.9 commands (`famp register`, `famp send`, `famp inbox`, `famp await`, `famp join`, `famp leave`, `famp sessions`, `famp whoami`) and move any archived `famp-local` table under a clearly labeled historical appendix.

### WR-02: README Still Documents Listener Redeploy For Deleted Daemons

**File:** `README.md:186`
**Issue:** **WARNING** - The "Redeploying after daemon code changes" section still documents listener daemons, `scripts/redeploy-listeners.sh`, per-agent `daemon.pid`, and a `listening on https://127.0.0.1:<port>` log line (`README.md:191`, `README.md:201`, `README.md:209`). Phase 04 deleted the `famp listen` surface for v0.9; leaving this in the primary README sends operators to a dead deployment model.
**Fix:** Delete this section from current README or move it into the v0.8/v1 federation history docs. Replace it with broker lifecycle guidance if v0.9 needs an operational section.

### WR-03: Migration Guide Claims `install-claude-code` Rewrites Project `.mcp.json`

**File:** `docs/MIGRATION-v0.8-to-v0.9.md:24`
**Issue:** **WARNING** - The migration guide says `famp install-claude-code` "auto-rewrites your `.mcp.json`" and repeats that it handles project-scope `.mcp.json` cleanup (`docs/MIGRATION-v0.8-to-v0.9.md:32`). The implementation only writes user-scope `~/.claude.json`, slash commands, settings hooks, and the hook runner (`crates/famp/src/cli/install/claude_code.rs:68`, `crates/famp/src/cli/install/claude_code.rs:82`, `crates/famp/src/cli/install/claude_code.rs:90`, `crates/famp/src/cli/install/claude_code.rs:98`). Users with legacy project `.mcp.json` files will believe the command migrated them when it did not.
**Fix:** Either implement project `.mcp.json` rewrite support, or update the migration guide to say user-scope install is automatic and project-scope `.mcp.json` cleanup is manual/handled only by the archived `famp-local` wrapper.

---

_Reviewed: 2026-05-04T02:16:08Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
