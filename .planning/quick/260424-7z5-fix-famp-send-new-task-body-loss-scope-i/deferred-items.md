# Deferred items discovered during quick task 260424-7z5

These issues exist on `main` prior to this task and are outside the surgical
scope of the 7z5 fix. Logged for follow-up.

## Pre-existing `cargo fmt --all -- --check` failures

`cargo fmt --all -- --check` on `main` at `526ac2c` (before any 7z5 edits
were applied — verified via `git stash && cargo fmt --all -- --check`)
reports formatting drift in these files:

- `crates/famp/src/cli/inbox/list.rs` (lines 126, 143)
- `crates/famp/src/cli/mcp/tools/inbox.rs` (line 43)
- `crates/famp/tests/e2e_two_daemons.rs` (line 127)
- `crates/famp/tests/inbox_list_filters_terminal.rs` (lines 16, 39, 300, 314, 346)
- `crates/famp/tests/mcp_stdio_tool_calls.rs` (line 415)

None of these files are touched by 260424-7z5. Fixing them here would be a
drive-by refactor and violate the surgical-changes constraint in CLAUDE.md /
the 7z5 PLAN. Recommended follow-up: a separate `chore(fmt): ...` commit
running `cargo fmt --all` across the repo, so the fmt CI gate goes green
again without coupling it to behavioural changes.

The four files 7z5 *did* touch are individually fmt-clean (verified via
`rustfmt --check <file>`):
- `crates/famp-envelope/src/body/request.rs`
- `crates/famp/src/cli/send/mod.rs`
- `crates/famp/src/cli/mcp/tools/send.rs`
- `crates/famp/tests/send_new_task_scope_instructions.rs`

## Rerun clippy + fmt after fmt chore

Once the fmt chore above lands, re-run:

    just ci

to confirm the full pipeline is green again. The 7z5-specific gates are
already green independently (see `260424-7z5-SUMMARY.md`).
