# Phase 3: Claude Code integration polish - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-02
**Phase:** 03-claude-code-integration-polish
**Areas discussed:** install-claude-code mechanics, slash commands + HOOK-04b runner, README install method + 30s gate, Codex parity + ONBOARDING.md scope

**Mode:** Single-pass — Ben asked for honest opinions on all four areas, then asked for steelmans of the opposite, then locked the revised positions. No multi-turn AskUserQuestion drilldown per area.

---

## install-claude-code mechanics

| Option | Description | Selected |
|---|---|---|
| Shellout to `claude mcp add` first; fall back to direct JSON edit | Future-proof against `~/.claude.json` schema drift; documented; idempotent at `claude` level. | |
| Direct JSON edit only (atomic write + .bak backup) | Fully under our control; snapshot-testable with `insta`; survives `claude` CLI flag drift; same code path as uninstall. | ✓ |

**User's choice:** Direct JSON edit only.
**Notes:** Steelman of the direct-edit-only position landed: (a) `claude` CLI is a moving target — auth prompts, scope semantics, version skew can each break the install non-deterministically; (b) direct-edit is snapshot-testable, shellout is not; (c) many users install Claude Code via the macOS `.app` bundle and never put `claude` on PATH, making the "fallback" the *primary* path in practice; (d) we need direct-JSON code for uninstall regardless. Locked as D-01.

Idempotency + atomic-write + .bak backup + user-scope-only + ship `uninstall` in same plan all locked alongside (D-02, D-03, D-04). Snapshot-tested install→uninstall round-trip mandatory.

---

## Slash commands + HOOK-04b runner

### Slash command naming

| Option | Description | Selected |
|---|---|---|
| Keep ROADMAP wording — `/famp-msg` + `/famp-channel` | Don't re-litigate; ROADMAP locks them; `famp-` prefix already disambiguates from system `/msg`. | |
| Mirror CLI verbs — `/famp-send` (renamed from `/famp-msg`) | One mental model: `famp send --to bob` ↔ `/famp-send bob`. ROADMAP explicitly deferred the bikeshed to Phase 3 — this *is* the time. | ✓ |

**User's choice:** Mirror CLI verbs — `/famp-send` (rename from `/famp-msg`).
**Notes:** Steelman flipped initial recommendation. ROADMAP literally says "Naming bikeshed (msg vs send vs dm) deferred to Phase 3" — that's a deferral instruction, not a lock. CLI-mirroring removes a synonym tax across 7 commands. Requires REQUIREMENTS.md CC-05 amendment + ROADMAP §"Phase 3" success criterion 2 amendment (D-05).

Final 7 names locked in CONTEXT.md D-05 with MCP-tool-mapping table.

### HOOK-04b hook event

| Option | Description | Selected |
|---|---|---|
| `PostToolUse` matcher `Edit\|Write\|MultiEdit` | Per-file granularity; standard CC pattern for file-edit reactions. | |
| `Stop` (once per turn) | Coalesces "Claude finished a turn touching these files" into one event; per-file fan-out via shim against turn's modified-files list. Avoids find-replace-flood. | ✓ |

**User's choice:** `Stop` hook.
**Notes:** Steelman flipped initial recommendation. PostToolUse fires per file — a single find-replace across 50 files dispatches 50 hooks. With concurrent CC windows it's worse. Stop coalesces into per-turn dispatch. Locked as D-07. Shim reads turn's modified-files list (researcher confirms exact source — `$CLAUDE_TRANSCRIPT_PATH` or equivalent) and applies hooks.tsv globs to the full list.

### HOOK-04b runner implementation

| Option | Description | Selected |
|---|---|---|
| Native Rust `famp hook run` subcommand | Survives Phase 4's `scripts/famp-local` deprecation; testable in Rust; hooks.json fragment is one tiny command line. | |
| Bash shim at `~/.famp/hook-runner.sh` | ~30 lines bash; decouples shim lifecycle from `famp` binary version (old `famp` still works with fresh shim); shellcheck-only test surface. | ✓ |

**User's choice:** Bash shim at `~/.famp/hook-runner.sh`.
**Notes:** Steelman convinced — the binary-coupling argument is real (an old `famp` binary without `hook run` would silently break the hook). Bash shim installed by `famp install-claude-code`, removed by `famp uninstall-claude-code`. Lives in `~/.famp/` data dir, not `scripts/famp-local/` (Phase 4 deprecates that). Locked as D-08.

hooks.json merge strategy locked alongside (D-09): unique-marker entry under Stop key, command-prefix idempotency, atomic write with .bak backup.

---

## README install method + 30s gate

| Option | Description | Selected |
|---|---|---|
| GitHub release binary + Homebrew tap | ~5s install, fits literal 30s budget, but requires GH Actions + macOS notarization + second repo for tap; permanent maintenance burden at v0.1.0 pre-release. | |
| `cargo install --path crates/famp` | Works today, no new pipeline; but 60–120s compile blows the literal 30s gate. | |
| `cargo install famp` from crates.io (after publishing) | One command per release, no separate pipeline, no signing, no second repo; second-window install <30s; first-window includes one compile (acknowledged in README). | ✓ |

**User's choice:** `cargo install famp` from crates.io.
**Notes:** Steelman flipped initial recommendation. Binary-release pipeline is a whole new product to maintain pre-1.0 (notarization, signing, version-skew). Brew tap delivers `brew install thebenlamm/famp/famp` (not `brew install famp`) — small payoff, permanent burden. crates.io is the standard Rust path; first-install compile is expected; second-window experience (`~/.cargo/bin/famp`) is fast forever.

ROADMAP §"Phase 3" success criterion 3 amendment required (D-11): `brew install famp` → `cargo install famp`. 30-second gate language amended to second-window-install spirit; 12-line gate stays literal. Locked as D-10 + D-11.

Punt brew + GitHub release binaries + curl|sh installer to v0.9.1 if user demand emerges.

---

## Codex parity + ONBOARDING.md scope

### Codex parity

| Option | Description | Selected |
|---|---|---|
| Docs-only (mention in ONBOARDING.md, no subcommand) | Codex needs one MCP entry; "subcommand for one entry" feels like over-engineering. | |
| Ship `famp install-codex` subcommand | ~50 LoC, symmetric with `install-claude-code`, self-discoverable from `famp --help`, pattern generalizes to Cursor/Continue. | ✓ |

**User's choice:** Ship `famp install-codex` (+ `famp uninstall-codex`).
**Notes:** Steelman flipped initial recommendation. 50 LoC vs every Codex user copying a snippet (30% will get escaping/paths wrong) — net cost lower with subcommand. Pattern generalizes — same template un-blocks Cursor (`~/.cursor/mcp.json`) and Continue when they appear. Locked as D-12. Codex gets MCP-only — no slash commands, no hooks (Codex doesn't expose those primitives the same way).

### ONBOARDING.md scope

| Option | Description | Selected |
|---|---|---|
| Comprehensive (Install + Quick Start + Channels + Hooks + Other clients + Troubleshooting, ~150 lines) | Real users need depth beyond the 12-line gate. | |
| Minimal (Install + Quick Start verbatim + Other clients + Uninstall, ≤80 lines) | The headline gate is "two windows, one message." Channels/hooks/troubleshooting are not the gate; rot fastest; `famp-local hook --help` covers registration. Grow from real reports. | ✓ |

**User's choice:** Minimal scope, ≤80 lines, three sections + closing pointer.
**Notes:** Channels semantics may shift in v0.9.1 (ChannelEvent broadcast deferred); writing extensive docs before Sofer-equivalent uses them is rework risk. Troubleshooting sections rot fastest. Locked as D-13. Channels/hooks/troubleshooting/multi-agent workflow examples all explicitly deferred.

---

## Claude's Discretion

Captured in CONTEXT.md `<decisions>` "Claude's Discretion" subsection. Highlights:
- Exact JSON-mutation crate vs hand-rolled walker for `~/.claude.json`.
- Bash idioms in `~/.famp/hook-runner.sh` (shellcheck-clean is the only hard requirement).
- crates.io publish ordering for the workspace dependency graph.
- Codex config target file path + format (TOML vs JSON) — researcher fetches Codex docs.
- Slash-command frontmatter exact keys (`allowed-tools` vs `tools`) — researcher fetches CC docs.
- Whether `famp install-claude-code` suggests `cargo install famp` if `famp` itself isn't on PATH (recommend NO — don't recursively self-install).

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section:
- Homebrew tap (`thebenlamm/homebrew-famp`) — v0.9.1 if demand.
- GitHub release binaries + `curl | sh` — v0.9.1 if demand.
- Homebrew core formula — v1.0+ post-volume.
- `famp install-cursor` / `famp install-continue` — ship when a real user hits it.
- Channels / hooks deep-dive in ONBOARDING.md — v0.9.1 after stabilization.
- `docs/TROUBLESHOOTING.md` — grow from real reports.
- Multi-agent workflow examples — wait for Sofer-equivalent usage.
- Literal 30-second wall-clock including first install — D-11 amendment makes this second-window-only.
- Project-scope `.mcp.json` provision — v0.9 single-host single-user only.

## Preconditions / Blockers

- **PARKED:** Architect counsel on `famp send` audit_log wrapper from Phase 2 (`crates/famp/src/cli/send/mod.rs::build_envelope_value`). Slash commands invoke `famp_send` MCP tool, which routes through the wrapper. Plan-lock blocked until counsel resolves the option-1/2/3 question (lean option 1).
- **BLOCKING-RESEARCH:** crates.io name availability for `famp` and all workspace members. Researcher MUST run `cargo search` and surface conflicts.
- **BLOCKING-RESEARCH:** each workspace `Cargo.toml` is publishable (`description` / `license` / `repository` / `path` deps with version constraints).
