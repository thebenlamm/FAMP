---
phase: quick-260729-ur8
plan: 01
status: complete
commits:
  - 6a57253 (test): extend slash_command_assets.rs with 5 new gate tests
  - 1b88492 (ci): add .github/workflows/asset-gate.yml
files_modified:
  - crates/famp/tests/slash_command_assets.rs
  - .github/workflows/asset-gate.yml
---

# Quick Task 260729-ur8 — CI-gated test validating slash-command asset schemas

## What shipped

`crates/famp/tests/slash_command_assets.rs` extended from 3 famp-who.md-only
tests into a 7-asset, registry-cross-checked gate (8 tests total, 3
pre-existing + 5 new). The registry itself is parsed as TEXT out of
`crates/famp/src/cli/mcp/server.rs`'s `tool_descriptors()` `json!` literal
(no `crates/famp/src/` file was touched — `tool_descriptors()` stayed
private).

New tests:
1. `slash_command_asset_harness_is_not_vacuous` — asset-dir count vs the
   registered `ASSETS` const, registry size == 12, one-tool-per-asset
   invariant.
2. `slash_command_assets_reference_only_dispatchable_mcp_tools` — every
   `mcp__famp__famp_X` an asset names resolves to both a registry descriptor
   and a `dispatch_tool` match arm.
3. `slash_command_assets_prescribe_only_real_argument_keys` — every key an
   asset prescribes is a real `inputSchema.properties` member of the tool it
   names. **This is the gate that would have caught the shipped `to` bug.**
4. `slash_command_assets_prescribe_every_required_argument_key` — every
   `required` key is covered, skipped for assets with an empty prescribed-key
   set (famp-register.md expresses `identity` as prose, not a key — GT-4).
5. `slash_command_asset_tool_count_claims_match_registry` — numeric
   tool-count claims in asset prose (e.g. famp-who.md's "12-tool" / "12
   tools") match the parsed registry length.

New `.github/workflows/asset-gate.yml` (purely additive, single ubuntu job,
plain `cargo test -p famp --test slash_command_assets`, no nextest install)
covers the previously dark commit path: ci.yml's `paths-ignore` excludes
`**/*.md`, and the slash-command assets are `.md`, so an assets-only edit
produced zero check-runs before this. `.github/workflows/ci.yml` is
byte-identical to HEAD — not touched, per GT-5a (Ben's 2026-07-29 decision).

## Falsification evidence (mandatory, both states recorded)

**Broken state** — `crates/famp/assets/slash_commands/famp-send.md`'s
`` - `peer`: `$1` `` bullet temporarily replaced with the historical broken
`` - `to`: `{"kind": "agent", "name": "$1"}` `` bullet:

```
test slash_command_assets_prescribe_only_real_argument_keys ... FAILED

---- slash_command_assets_prescribe_only_real_argument_keys stdout ----

thread 'slash_command_assets_prescribe_only_real_argument_keys' panicked at crates/famp/tests/slash_command_assets.rs:345:13:
famp-send.md prescribes key `to` for famp_send, but famp_send's inputSchema.properties only accepts {"body", "channel", "expect_reply", "mode", "more_coming", "peer", "task_id", "title"}

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Control test `test_famp_who_allowed_tools_lists_only_famp_peers` **PASSED**
in that same run (visible in the `7 passed` count) — proving the harness
itself executed rather than the target being skipped.

**Restored state** — `git checkout -- crates/famp/assets/slash_commands/famp-send.md`,
then re-ran:

```
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Literal clean-state pass count: **8 passed; 0 failed** (3 pre-existing + 5
new).

## Verification (all recorded)

- `cargo test -p famp --test slash_command_assets` — **8 passed; 0 failed**
  on the clean tree (confirmed twice: once after Task 1+2, once after Task
  3's workflow addition).
- `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) —
  **clean**. Run twice (once before, once after adding asset-gate.yml). Not
  plain `cargo clippy` — this repo denies clippy `pedantic` and warns
  `nursery`, and `just lint` is the recipe CI mirrors.
- `cargo fmt --all -- --check` — clean (ran `cargo fmt --all` once to apply
  a formatting fix to a multi-line `.parse().unwrap_or_else(...)` chain,
  then confirmed `--check` passes).
- `git diff --exit-code -- .github/workflows/ci.yml` — clean, `ci.yml`
  byte-identical to HEAD.
- `git diff --name-only 2d0adbe..HEAD` →
  `.github/workflows/asset-gate.yml`, `crates/famp/tests/slash_command_assets.rs`
  only. **No file under `crates/famp/src/` touched; no `pub` added to
  `tool_descriptors()`.**
- Asset file modes: all 7 `crates/famp/assets/slash_commands/*.md` files
  remain `-rw-r--r--` (0644) before and after, including famp-send.md after
  the falsification patch was reverted.
- `python3 -c 'import yaml; ...'` YAML-parser check (PyYAML `on:`→`True`
  gotcha handled via `d.get("on", d.get(True))`) — confirms both `push` and
  `pull_request` carry the identical 3-entry `paths` list, `concurrency.group`
  (`asset-gate-${{ github.ref }}`) does not start with `ci-`, and exactly one
  job is declared.

**`just install` was deliberately NOT run.** This is a test-only + CI-config
change — no MCP tool schema, tool descriptor, or CLI surface was touched, so
the installed `~/.cargo/bin/famp` binary is unaffected and irrelevant to this
plan's deliverable.

## Two-path CI wiring evidence

- **`.rs`-touching commits:** already covered by ci.yml's existing `test`
  job — `cargo nextest run --workspace --profile ci` (ci.yml:104-119,
  `needs: [test-canonical, test-crypto]`). `--workspace` auto-discovers
  `crates/famp/tests/slash_command_assets.rs` as an integration target; no
  registration needed. No change made to ci.yml.
- **Assets-only `.md` commits:** now covered by the new
  `.github/workflows/asset-gate.yml`, proven reachable by the YAML-parser
  check above (paths list includes `crates/famp/assets/slash_commands/**`).

## Known Stubs

None.

## Deviations from Plan

None — plan executed exactly as written. Tasks 1 and 2 were combined into a
single `Write` + single commit (the whole extended test file was authored in
one pass rather than incrementally in two edits), but both tasks' required
tests, behavior, and the mandatory falsification protocol were all completed
and verified exactly as specified.
