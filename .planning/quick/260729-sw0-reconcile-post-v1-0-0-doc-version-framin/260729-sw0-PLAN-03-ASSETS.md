---
phase: quick-260729-sw0
plan: 03
type: execute
wave: 3
depends_on: [01, 02]
files_modified:
  - crates/famp/assets/slash_commands/famp-send.md
  - crates/famp/assets/slash_commands/famp-channel.md
  - crates/famp/assets/slash_commands/famp-who.md
  - crates/famp/assets/slash_commands/famp-inbox.md
  - crates/famp-crypto/README.md
  - crates/famp-gateway/Cargo.toml
  - CONTRIBUTING.md
autonomous: true
---

# Plan 03 — installed assets + crate manifests (wave 3)

Wave 1 fixed README/ARCHITECTURE/CLAUDE.md framing. Wave 2 fixed the managed-block
sources and the docs/ guides. This wave fixes the **installed artifacts** and crate
metadata — including two genuine BLOCKERs that emit a runtime error if followed.

All findings verified by the orchestrator against source. Verification commands recorded.

## Deployment consequence (do NOT skip)

`crates/famp/assets/slash_commands/*.md` are COPIED into `~/.claude/commands/` by
`famp install-claude-code`. Editing them in the repo does NOT update an existing
install. After this wave: run `just install`, then the user must re-run
`famp install-claude-code` to propagate. No broker restart needed (no wire/FSM change).

## The `check-spec-version-coherence` contract (settles the famp-gateway edit)

`Justfile:205-215`. Gated on `FAMP_SPEC_VERSION == "0.5.2"` in
`crates/famp-envelope/src/version.rs` (confirmed true). It then requires
`MessageClass::AuditLog` and `AuditLogBody` to exist, and loops `crates/*/Cargo.toml`
failing **only if** a `^description` line contains the literal substring `v0.5.1`.
It does NOT require `v0.5.2` to be present. So a description may carry `v0.5.2`
(the PROTOCOL SPEC version) freely — that is distinct from `1.0.0`, the RELEASE
version. Keeping `v0.5.2` in the gateway description is correct and gate-safe.

## FROZEN — must remain byte-identical

`crates/famp/tests/slash_command_assets.rs` asserts on `famp-who.md`:

| Assertion | Detail |
|---|---|
| must NOT contain `famp_sessions` | test line 20 |
| frontmatter `allowed-tools:` line == exactly `mcp__famp__famp_peers` | test line 30-31 |
| must contain literal `argument-hint: [#channel?]` | test |

`crates/famp/tests/install_claude_code.rs` asserts all 7 `famp-*.md` EXIST with mode
`0o644` (existence + permission only, no content bytes).
`crates/famp/tests/install_grok.rs` asserts `SKILL.md` contains `famp_register`.

Every edit below touches BODY PROSE only. Do not alter frontmatter, do not change
file modes, do not add `famp_sessions`.

---

## Task 1 — BLOCKERS: slash commands prescribe a nonexistent JSON shape

Verified: `crates/famp/src/cli/mcp/tools/send.rs:113` reads `mode` via
`.ok_or_else(...)` — it is REQUIRED. Lines 121/125/136/141/145 read `peer`,
`channel`, `task_id`, `title`, `body`. `grep -rn '"to"\|"kind"'
crates/famp/src/cli/mcp/tools/send.rs` → ZERO hits: the nested `to` object is
never read. `crates/famp/tests/mcp_bus_e2e.rs:31-33` documents the flat surface
explicitly as "NOT nested `to: {kind, name}`". Following these assets literally
yields `EnvelopeInvalid: missing required field: mode`.

### E1 · `crates/famp/assets/slash_commands/famp-send.md:10`
OLD line: ``- `to`: `{"kind": "agent", "name": "$1"}` ``
Replace the argument-construction block so it reads:
```
- `peer`: `$1`
- `mode`: `"open"`
- `title` or `body`: the message text (everything after the recipient)
```
Keep surrounding prose. If the block also names `new_task` as the mode, change it
to `"open"` — `new_task` is a valid mode but opens a task FSM, which a plain
`/famp-send` should not do implicitly. Preserve the existing "if recipient starts
with `#`, redirect to /famp-channel" guidance if present.

### E2 · `crates/famp/assets/slash_commands/famp-channel.md:10`
OLD line: ``- `to`: `{"kind": "channel", "name": "$1"}` ``
Replace with:
```
- `channel`: `$1`
- `mode`: `"open"`
- `title` or `body`: the body text
```
Keep the existing guidance that `$1` must start with `#`.

**Both edits must be verified by actually exercising the documented shape** — see
Verification step 4. Do not mark this task done on inspection alone.

---

## Task 2 — stale counts and version framing in installed assets

### E3 · `crates/famp/assets/slash_commands/famp-who.md:18` and `:21`
Verified: the MCP surface is 12 tools, enforced by
`tool_descriptors_has_exactly_twelve_named_tools` (`server.rs:489`).

OLD (:18): `8-tool MCP surface in v0.9".`
NEW (:18): `12-tool MCP surface".`

OLD (:21): `above. The v0.9 MCP surface is exactly 8 tools and the project tests`
NEW (:21): `above. The MCP surface is exactly 12 tools and the project tests`

### E4 · `crates/famp/assets/slash_commands/famp-inbox.md:6`
v1.0 has shipped and `include_terminal` is STILL a broker-side no-op, so "deferred
to v1" is now false framing; and the next milestone is undefined, so do not name a
replacement version.

OLD fragment: `broker-side terminal-FSM filtering is deferred to v1.`
NEW fragment: `broker-side terminal-FSM filtering has not shipped as of v1.0, so every list returns all unread envelopes.`

NOTE: `crates/famp/src/cli/mcp/server.rs` lines ~68 and ~73 carry the same stale
"deferred to v1" phrase in the live tool description. That is SOURCE, not an asset.
Report it; do NOT edit it in this wave (it changes the MCP tool surface, which has
its own `just install` + review contract).

---

## Task 3 — crate metadata and CONTRIBUTING

### E5 · `crates/famp-crypto/README.md:3` — stale spec version
Verified: spec authority is v0.5.2; this crate's own `Cargo.toml` description
already says v0.5.2, so the README contradicts its own manifest.

OLD: ``FAMP v0.5.1 Ed25519 sign/verify with domain separation (`FAMP-sig-v1\0`),``
NEW: ``FAMP v0.5.2 Ed25519 sign/verify with domain separation (`FAMP-sig-v1\0`),``

DO NOT touch this file's other `v0.5.1` mentions — lines ~18 and ~32 cite
`FAMP-v0.5.1-spec.md` §7.1a / §7.1c by its real filename. That file exists at repo
root; those are correct citations, not drift. Verify with `test -f FAMP-v0.5.1-spec.md`.

### E6 · `crates/famp-gateway/Cargo.toml:9` — "skeleton" is false
OLD: `description = "FAMP v0.5.2 — famp-gateway crate (Layer 2 skeleton)"`
NEW: `description = "FAMP v0.5.2 — famp-gateway crate (Layer 2 cross-host federation gateway, shipped v1.0; INV-10)"`
Keeps `v0.5.2` so `check-spec-version-coherence` still passes (see contract above).

### E7 · `CONTRIBUTING.md:4-6` — PR gate reads as pending
v1.0 shipped, so by CONTRIBUTING's own rule external PRs are now open; and
`docs/DEVELOPMENT.md:281` was already updated in wave 2 to say so. Do not let the
two disagree.

OLD:
```
Protocol. The `v0.8` Personal Runtime (with MCP integration) is maintained by
a single developer. External PRs are welcome from `v1.0` onward; until then,
please file issues rather than PRs.
```
NEW:
```
Protocol. The Personal Runtime (with MCP integration) is maintained by a single
developer under adversarial review. External PRs are welcome — `v1.0` has shipped.
```
CAVEAT: the "single developer" headcount is retained as-is because it is the
maintainer's own statement about the project and is not ours to change. Only the
version gate is being reconciled.

### E8 · `CONTRIBUTING.md:71` — stale version anchor
OLD: ``For `v0.8`: single maintainer plus adversarial review agent.``
NEW: `Single maintainer plus adversarial review agent, applied to every non-trivial change.`

### E9 · `CONTRIBUTING.md` Repo Layout — lists 11 of 16 crates
Verified `ls crates/` = 16. Add the five missing entries, matching the existing
bullet style and placement in the list:
```
- `crates/famp-bus` — pure-actor local UDS bus (Layer 1)
- `crates/famp-gateway` — cross-host Ed25519-signed proxy for remote principals (Layer 2, INV-10)
- `crates/famp-inspect-proto` — inspector RPC types (no I/O)
- `crates/famp-inspect-server` — inspector RPC handlers, mounted by the broker
- `crates/famp-inspect-client` — inspector RPC client behind `famp inspect`
```

---

## Verification — run ALL; paste literal output

Traps: `cargo nextest` HANGS here; `just ci` / `just test` unusable locally — use
plain `cargo test`. `cargo test <filter>` EXITS 0 ON ZERO MATCHES, so confirm the
passed-count for every run; an empty run is a FAILURE, not a pass.

1. `cargo test -p famp --test slash_command_assets` → confirm non-zero passed count.
2. `cargo test -p famp --test install_claude_code --test install_grok` → non-zero passed.
   (If these flake on `target/debug/famp` relinking, re-run in isolation — a known
   repo flake, not a regression. Say so explicitly if it happens.)
3. `just check-spec-version-coherence` → exit 0. This is the gate E6 could break.
4. **Prove E1/E2 actually work** (do not skip — this is the whole point of Task 1):
   `cargo run -q -p famp -- mcp` is a stdio server, so instead assert against the
   parser: confirm `mode` is required and `peer`/`channel` are the real keys via
   `grep -n 'get("mode")\|get("peer")\|get("channel")' crates/famp/src/cli/mcp/tools/send.rs`,
   AND confirm no `to`/`kind` key is read:
   `grep -c '"to"\|"kind"' crates/famp/src/cli/mcp/tools/send.rs` → must be `0`.
   Then confirm the assets no longer instruct the nonexistent shape:
   `grep -c '"kind"' crates/famp/assets/slash_commands/famp-send.md crates/famp/assets/slash_commands/famp-channel.md` → must be `0` for both.
5. `grep -rn '8-tool\|8 tools\|deferred to v1\.' crates/famp/assets/` → empty.
6. `grep -c 'v0.5.1' crates/famp-gateway/Cargo.toml` → 0.
7. `grep -n 'from `v1.0` onward\|For `v0.8`:' CONTRIBUTING.md` → empty.
8. `grep -c '^- `crates/' CONTRIBUTING.md` → 16.
9. `cargo build -q -p famp-gateway` → succeeds (proves the Cargo.toml edit is valid TOML).
10. `git diff --name-only` → exactly the 7 files in `files_modified`.
