---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 10
subsystem: scripts/famp-local
tags: [hook, declarative-wiring, sofer-leverage, phase-2-registration-only]
requires:
  - scripts/famp-local existing TSV helpers (wires_remove_row, awk + tempfile + mv)
  - 02-00 stub: crates/famp/tests/hook_subcommand.rs (3 #[ignore] tests)
provides:
  - cmd_hook_add / cmd_hook_list / cmd_hook_remove / cmd_hook (bash subcommand surface)
  - hooks_file helper (resolves $STATE_ROOT/hooks.tsv)
  - hooks.tsv on-disk registry (4-field TSV: id, event:glob, to, added_at)
  - hook dispatch entry in main case
affects:
  - scripts/famp-local (1230 → 1316 lines, +86)
  - crates/famp/tests/hook_subcommand.rs (stub replaced; 3 tests now run)
tech-stack:
  added: []
  patterns:
    - awk + tempfile + mv (atomic TSV row removal — mirrors wires_remove_row)
    - "h<unix-time-hex><6-hex-random>" id format (head -c3 /dev/urandom | xxd -p)
    - ISO-8601 UTC timestamp via `date -u +%Y-%m-%dT%H:%M:%SZ`
    - dispatcher pattern: `cmd_hook` switches on subcommand to add|list|remove
key-files:
  created: []
  modified:
    - scripts/famp-local
    - crates/famp/tests/hook_subcommand.rs
decisions:
  - "cmd_hook_list emits TSV verbatim (no header) — pipe-friendly; users awk/cut on it directly"
  - "Test env var: FAMP_LOCAL_ROOT (not STATE_ROOT, which is internal). Plan text was wrong; corrected as Rule 1 deviation."
  - "Empty hooks.tsv after remove is acceptable (file persists with 0 bytes); list prints empty string, not 'no hooks registered'. The 'no hooks registered' branch fires only when the file does not exist."
  - "HOOK-04 split (D-12) reflected inline: HOOK-04a (registration) closed here; HOOK-04b (execution runner) deferred to Phase 3 where `famp install-claude-code` will install the ~/.claude/hooks.json fragment."
metrics:
  duration: 12min
  completed: 2026-04-28
---

# Phase 02 Plan 10: famp-local hook add|list|remove Summary

`scripts/famp-local` now has a declarative `hook` subcommand family that registers Edit-event hooks to a TSV registry — replacing Sofer's hand-written bash hook scripts with three round-trippable commands. Phase 2 ships the registration surface only (HOOK-04a); the execution runner is Phase 3 (HOOK-04b) per the D-12 split.

## What Shipped

- **`cmd_hook_add --on Edit:<glob> --to <peer-or-#channel>`** — validates `--on` starts with `Edit:`, generates a unique id (`h<unix-time-hex><6-hex-random>`), appends a TSV row to `~/.famp-local/hooks.tsv`. Echoes `hook added: id=… on=… to=…` on stdout.
- **`cmd_hook_list`** — cats `hooks.tsv` verbatim. Absent file → `no hooks registered` to stdout, exit 0.
- **`cmd_hook_remove <id>`** — atomic awk-rewrite via tempfile + mv. Absent id → `die "hook id '<id>' not found"`.
- **`cmd_hook`** dispatcher — switches on subcommand; unknown subcommand dies with help.
- **Main dispatch** — `hook) cmd_hook "$@" ;;` slots in immediately after `wire)`.
- **Help text** — three lines documenting the subcommand surface and the D-12 split.
- **Inline comments** — document HOOK-04a vs HOOK-04b boundary at both `hooks_file` and `cmd_hook_add`.

## TSV Format (Finalized)

```
<id>\t<event>:<glob>\t<to>\t<added_at>
```

Exactly 4 tab-separated fields. `<id>` starts with `h`. `<added_at>` is ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`).

Example row: `h69f13d0a3f579b<TAB>Edit:*.md<TAB>alice<TAB>2026-04-28T23:04:42Z`

## LoC Budget

`scripts/famp-local`: **1316 lines** (target ≤ 1500, plan estimate ~1340 — landed under).

## Decisions

- **cmd_hook_list emits no header** — TSV verbatim is pipe-friendly. The plan PATTERNS file mentioned a header line but the simpler verbatim form ships, matching `wires.tsv` precedent and leaving the file directly `cut`/`awk`-able.
- **Empty hooks.tsv after final remove is acceptable** — `cmd_hook_remove` does not unlink the file when it goes empty. `cmd_hook_list` then prints an empty string rather than the "no hooks registered" message. Tests assert either form.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Test env var corrected from `STATE_ROOT` to `FAMP_LOCAL_ROOT`**
- **Found during:** Task 2
- **Issue:** Plan task 2 sample test code set `STATE_ROOT` env, but `scripts/famp-local` reads `FAMP_LOCAL_ROOT` and computes `STATE_ROOT="${FAMP_LOCAL_ROOT:-$HOME/.famp-local}"` internally (line 19). Setting `STATE_ROOT` from the outside is shadowed by line 19 and the script falls back to the user's real `~/.famp-local`. Same bug repeats in the plan task 1 manual-test acceptance criteria.
- **Fix:** Tests use `.env("FAMP_LOCAL_ROOT", state_root)`, matching the existing `famp_local_wire_migration.rs` convention. A note in the test module-level doc records the correction.
- **Files modified:** `crates/famp/tests/hook_subcommand.rs`
- **Commit:** `1ea8419`

## D-12 Split Confirmation

This plan ships **HOOK-04a only** — the registration surface. The execution runner (HOOK-04b) is deferred to Phase 3 alongside `famp install-claude-code`, which will install the `~/.claude/hooks.json` fragment whose body is sketched in the inline comment near `cmd_hook_add`:

```
famp send --to "$to" --new-task "Edit hook: $glob matched $file"
```

REQUIREMENTS.md and ROADMAP.md edits for the split (HOOK-04 → HOOK-04a in Phase 2 / HOOK-04b in Phase 3) land in plan 02-12 alongside CARRY-02.

## Verification

- `bash -n scripts/famp-local` → exit 0
- `wc -l scripts/famp-local` → 1316 (≤ 1500)
- `cargo nextest run -p famp test_hook_add test_hook_list test_hook_remove` → 3 passed, 0 failed (in 0.082 s)
- Manual round-trip in temp `FAMP_LOCAL_ROOT`:
  - `hook add --on Edit:*.md --to alice` → `hook added: id=h69f13d0a3f579b on=Edit:*.md to=alice`
  - `hook list` → prints the TSV row
  - `hook remove h69f13d0a3f579b` → `hook removed: h69f13d0a3f579b`
  - `hook list` (post-remove) → empty
  - `hook remove h-not-real` → `famp-local: hook id 'h-not-real' not found`, exit 1

## Commits

| Task | Commit | Files |
|------|--------|-------|
| 1 — cmd_hook_add/list/remove + dispatcher + help + inline doc | `1d667d5` | scripts/famp-local |
| 2 — hook_subcommand.rs integration tests (Rule 1 deviation) | `1ea8419` | crates/famp/tests/hook_subcommand.rs |

## Self-Check: PASSED

- scripts/famp-local exists ✓
- crates/famp/tests/hook_subcommand.rs exists ✓
- Commit 1d667d5 in git log ✓
- Commit 1ea8419 in git log ✓
- All 3 integration tests pass ✓
- bash syntax check passes ✓
- LoC budget respected (1316 ≤ 1500) ✓
