---
phase: 260711-g1t
plan: 01
subsystem: docs
tags: [spec-version, mcp, docs, anti-drift-gate, justfile]

requires: []
provides:
  - README/CLAUDE.md/spec file reconciled to spec v0.5.2 in non-historical prose
  - all shipping crate `description` fields at v0.5.2
  - extended `check-spec-version-coherence` Justfile gate (crate-description drift)
  - v0.11-current-runtime milestone framing in CLAUDE.md Architecture + README
  - drift-proof MCP `Mcp` subcommand help text + twelve-descriptor unit test gate
  - README ~/.famp vs ~/.famp-local two-directory documentation
  - README toolchain-components (rustfmt/clippy) prerequisite note
  - obsolete v0.8 redeploy-listeners.sh retired to docs/history/
  - clarified install-claude-code restart messaging (slash commands live vs MCP restart)
  - GitHub issue #22 tracking eventual ~/.famp / ~/.famp-local unification
affects: [docs, mcp-surface, dev-loop-scripts]

tech-stack:
  added: []
  patterns:
    - "Anti-drift gate pattern: extend an existing Justfile coherence recipe with a grep-based crate-description check rather than adding a new recipe"
    - "Runtime-enumerated help text: point doc-comments at the authoritative runtime source (tools/list) instead of hardcoding a count that can drift"

key-files:
  created:
    - docs/history/redeploy-listeners.sh (renamed from scripts/redeploy-listeners.sh)
  modified:
    - README.md
    - CLAUDE.md
    - FAMP-v0.5.1-spec.md
    - crates/famp-crypto/Cargo.toml
    - crates/famp-inbox/Cargo.toml
    - crates/famp-taskdir/Cargo.toml
    - crates/famp-transport-http/Cargo.toml
    - crates/famp/Cargo.toml
    - Justfile
    - crates/famp/src/cli/mcp/server.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/install/claude_code.rs

key-decisions:
  - "README 'Current Milestones' changelog list still said v0.9 'shipping now' with no v0.10/v0.11 rows at all — not explicitly named as an edit site in the plan text, but directly violated the plan's must_haves truth ('README describes v0.11 as the current runtime') and the plan's own verify command (grep -c 'shipping now' README.md must be 0). Fixed as part of Task 3 rather than treating it as a separate deviation, since it is the same milestone-currency drift the task targets."
  - "Left the historical 'do not port scripts/redeploy-listeners.sh' reference in docs/superpowers/specs/2026-04-26-windows-port-brief.md untouched per plan instruction, even though the path is now stale post-move — that doc is itself a historical record."

requirements-completed: [DRIFT-01-spec-version, DRIFT-02-milestone-currency, DRIFT-03-mcp-tool-count, DRIFT-04-two-dir-doc, DRIFT-05-obsolete-script, DRIFT-06-toolchain-note, DRIFT-07-install-restart-msg]

coverage:
  - id: D1
    description: "Spec version 0.5.2 stated in non-historical README/CLAUDE.md/spec-file prose; historical rows untouched"
    requirement: "DRIFT-01-spec-version"
    verification:
      - kind: unit
        ref: "grep -c 'v0.5.2-spec-conformant' README.md; grep -c 'v0.5.2' CLAUDE.md"
        status: pass
      - kind: unit
        ref: "cargo test -p famp --lib cli::tests::version_strings_unified"
        status: pass
    human_judgment: false
  - id: D2
    description: "All five stale crate descriptions bumped to v0.5.2; check-spec-version-coherence extended to fail on any v0.5.1 description"
    requirement: "DRIFT-01-spec-version"
    verification:
      - kind: unit
        ref: "just check-spec-version-coherence"
        status: pass
      - kind: other
        ref: "falsification: reverted crates/famp/Cargo.toml description to v0.5.1, gate failed (exit 1); restored, gate passed"
        status: pass
    human_judgment: false
  - id: D3
    description: "CLAUDE.md Architecture + README frame v0.11 as current runtime; shipped v0.9 bus items moved out of Not Shipped Yet"
    requirement: "DRIFT-02-milestone-currency"
    verification:
      - kind: unit
        ref: "grep -c 'FAMP today is local-first (v0.9)' CLAUDE.md; grep -c 'shipping now' README.md"
        status: pass
    human_judgment: false
  - id: D4
    description: "MCP Mcp subcommand doc-comment no longer hardcodes a tool count; twelve-descriptor anti-drift unit test added"
    requirement: "DRIFT-03-mcp-tool-count"
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib cli::mcp::server::tests::tool_descriptors_has_exactly_twelve_named_tools"
        status: pass
      - kind: other
        ref: "falsification: renamed famp_leave to famp_leave_XXX in server.rs, test failed with named diff; restored, test passed"
        status: pass
    human_judgment: false
  - id: D5
    description: "README documents ~/.famp vs ~/.famp-local split (no references removed) + toolchain prerequisite note; GitHub issue #22 filed for eventual unification"
    requirement: "DRIFT-04-two-dir-doc"
    verification:
      - kind: unit
        ref: "grep -c '.famp-local' README.md; grep -Eic 'rustfmt|clippy' README.md"
        status: pass
      - kind: other
        ref: "gh issue create --repo thebenlamm/FAMP -> https://github.com/thebenlamm/FAMP/issues/22"
        status: pass
    human_judgment: false
  - id: D6
    description: "scripts/redeploy-listeners.sh retired to docs/history/, no live README/Justfile reference"
    requirement: "DRIFT-05-obsolete-script"
    verification:
      - kind: unit
        ref: "test -f docs/history/redeploy-listeners.sh && test ! -f scripts/redeploy-listeners.sh && grep -c redeploy-listeners README.md Justfile"
        status: pass
    human_judgment: false
  - id: D7
    description: "install-claude-code completion message distinguishes live slash commands from MCP-registration-needs-restart"
    requirement: "DRIFT-07-install-restart-msg"
    verification:
      - kind: unit
        ref: "grep -c 'MCP server' crates/famp/src/cli/install/claude_code.rs; cargo test -p famp --lib install::claude_code (11 tests)"
        status: pass
    human_judgment: false

duration: ~15min
completed: 2026-07-11
status: complete
---

# Phase 260711-g1t Plan 01: Reconcile v0.11 Doc Drift Summary

**Closed all seven pre-verified v0.11 doc/script drift items (stale 0.5.1 spec references, stale v0.9 milestone framing, wrong 8-vs-12 MCP tool count, undocumented two-directory layout, dead v0.8 listener script, missing toolchain note, misleading install-restart message) and wired two anti-drift gates so the version and tool-count drift cannot silently reopen.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-11T15:46Z
- **Tasks:** 7/7
- **Files modified:** 12 (+1 renamed)

## Accomplishments

- Spec version 0.5.2 now authoritative in all non-historical README/CLAUDE.md/spec-file prose; historical rows (v0.5.1 fork note, frozen crypto vectors) left untouched
- Five stale crate `description` fields (famp-crypto, famp-inbox, famp-taskdir, famp-transport-http, famp) bumped to v0.5.2; `check-spec-version-coherence` now fails the build if any crate description regresses to v0.5.1
- CLAUDE.md Architecture section and README now frame v0.11 (broker daemon, shipped 2026-06-06) as the current runtime; the shipped v0.9 local bus moved out of "Not Shipped Yet" into "What Works Today"; README's Current Milestones changelog no longer calls v0.9 "shipping now" and gained the missing v0.10/v0.11 rows
- MCP `Mcp` subcommand doc-comment no longer hardcodes a stale tool count ("eight tools", partial enumeration) — it now points at the runtime `tools/list` method; a new unit test in `server.rs` pins `tool_descriptors()` to exactly the twelve current tool names, so a future add/remove fails loud
- README documents the `~/.famp` (message runtime) vs `~/.famp-local` (non-MCP CLI identity backing store) split without removing any existing reference; Prerequisites notes the first toolchain install also pulls rustfmt+clippy
- `scripts/redeploy-listeners.sh` (dead v0.8 per-agent HTTPS listener script) moved to `docs/history/redeploy-listeners.sh`; no live doc/recipe pointed to it
- `install-claude-code` completion message now distinguishes the 7 slash commands (live immediately) from the MCP server registration (needs restart)
- Filed GitHub issue [#22](https://github.com/thebenlamm/FAMP/issues/22) proposing eventual unification of `~/.famp` and `~/.famp-local`, explicitly scoped out as a runtime change

## Task Commits

1. **Task 1: Spec version 0.5.2 in prose docs (README, CLAUDE.md, spec file)** - `9c8405d` (docs)
2. **Task 2: Crate descriptions to v0.5.2 + extend coherence gate** - `474036b` (chore)
3. **Task 3: Milestone currency — CLAUDE.md Architecture + README "Not Shipped Yet"** - `bfb1da6` (docs)
4. **Task 4: Fix MCP tool-count help + add twelve-descriptor anti-drift test** - `b04e328` (test)
5. **Task 5: Document ~/.famp vs ~/.famp-local split + toolchain note + file unification issue** - `2c57ca9` (docs)
6. **Task 6: Retire obsolete redeploy-listeners.sh to docs/history/** - `709e976` (chore)
7. **Task 7: Clarify install-claude-code restart messaging** - `b16925f` (fix)

_This SUMMARY and STATE/ROADMAP updates are committed separately by the orchestrator (docs commit), per the quick-task execution convention._

## Files Created/Modified

- `README.md` — v0.5.2 stack line + changelog row; v0.11 current-runtime milestone framing, Current Milestones v0.10/v0.11 rows, no more "shipping now"; ~/.famp vs ~/.famp-local subsection; rustfmt/clippy prerequisite note
- `CLAUDE.md` — spec-fidelity constraint names v0.5.2 as authority; Architecture section reframed with v0.11 as current runtime and v0.9 as a shipped prior milestone
- `FAMP-v0.5.1-spec.md` — reference-implementation note rewritten past-tense (gap closed); new Δ34 changelog row
- `crates/famp-crypto/Cargo.toml`, `crates/famp-inbox/Cargo.toml`, `crates/famp-taskdir/Cargo.toml`, `crates/famp-transport-http/Cargo.toml`, `crates/famp/Cargo.toml` — `description` version token 0.5.1 → 0.5.2
- `Justfile` — `check-spec-version-coherence` extended with a crate-description v0.5.1 grep check
- `crates/famp/src/cli/mcp/server.rs` — new `#[cfg(test)] mod tests` with `tool_descriptors_has_exactly_twelve_named_tools`
- `crates/famp/src/cli/mod.rs` — `Mcp` subcommand doc-comment rewritten to reference runtime `tools/list` instead of a hardcoded count
- `crates/famp/src/cli/install/claude_code.rs` — completion message distinguishes slash commands (live) from MCP registration (restart needed)
- `docs/history/redeploy-listeners.sh` — moved from `scripts/redeploy-listeners.sh` (git rename, byte-identical)

## Decisions Made

- Fixed the README "Current Milestones" list's stale `v0.9 ... shipping now` line and missing v0.10/v0.11 rows as part of Task 3, even though the plan text named only the "Not Shipped Yet" section explicitly — it is the same milestone-currency drift, and the plan's own verify grep required zero "shipping now" occurrences in README.md.
- Left the `docs/superpowers/specs/2026-04-26-windows-port-brief.md` reference to `scripts/redeploy-listeners.sh` untouched per plan instruction — it is itself a historical "do not port" note.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] README "Current Milestones" list still called v0.9 "shipping now" with no v0.10/v0.11 rows**
- **Found during:** Task 3
- **Issue:** Fixing only the "Not Shipped Yet" section (as literally scoped) would have left the plan's own verify command (`grep -c 'shipping now' README.md | grep -q '^0$'`) failing, since the same stale phrase also appeared in the separate "Current Milestones" changelog block, which was also missing v0.10 and v0.11 rows entirely.
- **Fix:** Changed the v0.9 row from "shipping now" to "shipped", and added v0.10 and v0.11 rows describing the inspector and the broker daemon respectively.
- **Files modified:** README.md
- **Verification:** `grep -c 'shipping now' README.md` returns 0
- **Committed in:** bfb1da6 (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix, in-scope for the same drift item)
**Impact on plan:** Necessary to satisfy the plan's own verification command and the "v0.11 is current runtime" truth; no scope creep beyond the milestone-currency drift already targeted by Task 3.

## Issues Encountered

None. `gh` was already authenticated (`thebenlamm` account); issue #22 filed without incident.

## User Setup Required

None — no external service configuration required. Per the plan's `<post_execution_notes>`: Tasks 4 and 7 changed binary strings in famp source (`crates/famp/src/cli/mod.rs`, `crates/famp/src/cli/mcp/server.rs`, `crates/famp/src/cli/install/claude_code.rs`); **the orchestrator must run `just install`** after merge so `~/.cargo/bin/famp` reflects the corrected MCP help text and install-claude-code message. No broker restart needed (no wire/protocol change).

## Next Phase Readiness

- Zero runtime behavior change; all 177 `-p famp --lib` tests pass (176 prior + 1 new), `just check-spec-version-coherence` passes, `just spec-lint` passes (21/21), `cargo build -p famp` clean.
- Diff scope confirmed clean: only README.md, CLAUDE.md, FAMP-v0.5.1-spec.md, 5 Cargo.toml description lines, Justfile, and 3 famp CLI source files changed — no envelope/wire/FSM/broker source touched.
- Open follow-up: GitHub issue #22 (directory unification) is unassigned and unscheduled — a future milestone decision, not a v0.11 blocker.

---
*Phase: 260711-g1t*
*Completed: 2026-07-11*

## Self-Check: PASSED

- FOUND: docs/history/redeploy-listeners.sh
- FOUND commits: 9c8405d, 474036b, bfb1da6, b04e328, 2c57ca9, 709e976, b16925f
