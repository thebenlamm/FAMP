# Deferred Items — Phase 07

## 07-01: Pre-existing, out-of-scope test failures in `cargo test -p famp --lib`

**Found during:** Task 2 verification (`cargo test -p famp --lib`).

**Failures:**
- `cli::install::codex::tests::install_codex_is_idempotent`
- `cli::install::codex::tests::install_codex_preserves_unrelated_project_hooks`
- `cli::install::codex::tests::install_codex_preserves_unrelated_top_level_sections`
- `cli::install::codex::tests::install_codex_writes_mcp_and_stop_hook`
- `cli::uninstall::codex::tests::uninstall_after_install_removes_famp_table`

**Error:** `resolved famp binary target/debug/famp does not support 'hook codex-stop' (probe 'hook codex-stop --help' failed or timed out). Run 'just install' first...`

**Why out of scope:** None of Phase 07 Plan 01's tasks touch `crates/famp/src/cli/install/codex.rs`, `crates/famp/src/cli/hook/*`, or the Codex Stop-hook surface. These files were already modified in the shared working tree by concurrent, unrelated session activity at the start of this execution (visible in `git status` before any Plan 01 task ran). The failure is an environment/build-staleness issue (`target/debug/famp` predates the native `hook codex-stop` subcommand these files add), not a regression introduced by the additive `connect_no_spawn` constructor or the `famp-gateway` scaffold. `git diff` confirms Plan 01's commits touch only `crates/famp-gateway/`, `crates/famp/src/bus_client/mod.rs`, and `.planning/` docs.

**Action:** Not fixed. Logged per deviation-rules scope boundary (pre-existing/unrelated failures are out of scope for this plan). The relevant `cargo test -p famp --lib bus_client` subset (13/13) and full workspace `just lint` both pass clean, confirming Plan 01's own changes introduce zero regressions.
