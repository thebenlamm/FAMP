# Phase 3: Claude Code integration polish — Context

**Gathered:** 2026-05-02
**Status:** Ready for planning (with one parked precondition — see "Preconditions" below)

<domain>
## Phase Boundary

Hit the v0.9 milestone exit gate: a fresh-macOS user runs **≤12 lines of README and ≤30 seconds of wall-clock** to get two Claude Code windows exchanging a message. Net-new code in this phase:

1. `famp install-claude-code` subcommand — writes user-scope MCP entry to `~/.claude.json`, drops 7 slash-command markdown files into `~/.claude/commands/`, writes a hooks.json fragment to `~/.claude/hooks.json`, and installs a hook-runner shim to `~/.famp/hook-runner.sh`.
2. `famp uninstall-claude-code` subcommand — clean reversal of the above.
3. `famp install-codex` + `famp uninstall-codex` subcommands — same direct-JSON-edit pattern against Codex's MCP config; MCP-only (no slash commands, no hooks for Codex).
4. 7 slash-command markdown templates (`/famp-register`, `/famp-send`, `/famp-channel`, `/famp-join`, `/famp-leave`, `/famp-who`, `/famp-inbox`).
5. `~/.famp/hook-runner.sh` bash shim — reads `~/.famp-local/hooks.tsv` and dispatches `famp send` (HOOK-04b runner).
6. README Quick Start rewrite passing the 12-line gate.
7. `docs/ONBOARDING.md` — minimal scope (≤80 lines).

**Scope-locked from ROADMAP.md:** `CC-01..10 + HOOK-04b` — **11 requirements total**.

**Not in this phase:**
- Federation CLI removals, `e2e_two_daemons` library-API refactor, `v0.8.1-federation-preserved` tag, migration doc — Phase 4.
- v0.9.0 tag — cuts at end of Phase 4, not 3.
- GitHub release binaries, Homebrew tap, brew formula — punted to v0.9.1 if user demand emerges (D-09).
- Cursor / Continue / other MCP-aware client installers — pattern generalizes from `install-codex`, but ship those when a real user hits them.
- Channels deep-dive in ONBOARDING, hook recipes deep-dive, troubleshooting section — D-13 keeps ONBOARDING minimal.
- Any changes to `famp-bus`, the broker, the MCP server's tool surface, or the hook *registration* surface (`famp-local hook add/list/remove`) — all locked in Phases 1 and 2.

**Carrying forward from Phase 2 (already locked, do NOT re-decide):**
- 8-tool MCP surface stable: `famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami` (Phase 2 D-04).
- Hook *registration* is `famp-local hook add/list/remove` writing TSV rows `<id>\t<event>:<glob>\t<to>\t<added_at>` to `~/.famp-local/hooks.tsv` (Phase 2 D-12 / HOOK-04a — shipped). Phase 3 only adds the *runner*.
- Identity binding: `famp_register` is gating-required as the first MCP tool call; subsequent tools return `BusErrorKind::NotRegistered` until register succeeds (Phase 2 D-05). Slash commands inherit this — the user must `/famp-register alice` before any other slash command works in that window.
- `bind_as` connection-property model (Phase 2 D-10) means slash commands pass through a single long-lived MCP `bus: BusClient` per CC window, no per-message identity field.
- `MCP-01` audit is source-import grep, not `cargo tree` reachability (Phase 2 D-11). Phase 3 adds no new federation deps.

</domain>

<decisions>
## Implementation Decisions

### `famp install-claude-code` mechanics (CC-01)

- **D-01: Direct `~/.claude.json` JSON edit only — no shellout to `claude mcp add`.** Reasons: (a) CC's CLI flags are a moving target (auth prompts, scope semantics, version skew); a shellout failure surfaces as "famp install hangs" with the user blaming us; (b) the JSON mutation is snapshot-testable with `insta` — a shellout is not; (c) many users install Claude Code via the macOS `.app` bundle and never put `claude` on PATH, making the "fallback" the *primary* path in practice; (d) we need direct-JSON code for `uninstall` regardless, and maintaining two implementations of the same mutation is the smell. The `mcpServers.<name>` schema in `~/.claude.json` is the most stable surface in CC — drift risk is lower than CLI flag churn.

- **D-02: Atomic write + backup + idempotency.** Write strategy: read `~/.claude.json` → mutate in memory → write to `~/.claude.json.tmp` → rename to `~/.claude.json` (atomic on macOS APFS / ext4). Before mutate, copy to `~/.claude.json.bak.<timestamp>` so a corrupt write is recoverable. Idempotent: detect existing `mcpServers.famp` entry by structural match (command + args); if present and matching, no-op; if present and stale, in-place update; if absent, insert. Never duplicate.

- **D-03: User-scope only — no project-scope provision in v0.9.** `~/.claude.json` writes happen at user level; Phase 3 makes no provision for project-scoped MCP entries (`.mcp.json`). Single-host single-user is the v0.9 acceptance condition; multi-tenant / shared-dev-environment scopes belong to v1.0+ if ever.

- **D-04: Ship `famp uninstall-claude-code` in the same plan as `famp install-claude-code`.** Reverses every mutation: removes the `mcpServers.famp` entry, deletes the 7 slash-command files in `~/.claude/commands/famp-*.md`, drops only the famp-tagged entry from `~/.claude/hooks.json` (D-08), removes `~/.famp/hook-runner.sh`. Snapshot-test the install→uninstall round-trip — pre-state byte-equal to post-state when run on a clean install. No partial uninstall states.

### Slash commands (CC-02..08)

- **D-05: Slash command names mirror CLI verbs exactly.** `/famp-msg` is RENAMED to `/famp-send` to match the underlying `famp send` CLI verb. The user has one mental model: `famp send --to bob "hi"` ↔ `/famp-send bob "hi"`. Final 7 names locked:

  | Slash command | MCP tool invocation |
  |---|---|
  | `/famp-register <name>` | `famp_register(name)` |
  | `/famp-send <to> <body>` | `famp_send(to={kind:"agent", name}, new_task=body)` |
  | `/famp-channel <#name> <body>` | `famp_send(to={kind:"channel", name}, new_task=body)` |
  | `/famp-join <#name>` | `famp_join(channel)` |
  | `/famp-leave <#name>` | `famp_leave(channel)` |
  | `/famp-who [#name?]` | `famp_peers()` if no arg, else channel members via `famp_sessions` filter |
  | `/famp-inbox` | `famp_inbox(include_terminal=false)` |

  > **Required upstream amendments (researcher MUST surface in RESEARCH.md):**
  > - `.planning/REQUIREMENTS.md` CC-05: replace `/famp-msg` with `/famp-send`.
  > - `.planning/ROADMAP.md` §"v0.9 Phase 3" success criterion 2: replace `/famp-msg` with `/famp-send`.
  > Both edits land in the same plan that ships the slash-command files (atomic per AUDIT-05-style invariant).

- **D-06: Slash command file format — markdown with frontmatter.** Each `~/.claude/commands/famp-<verb>.md` carries YAML frontmatter declaring `allowed-tools: [mcp__famp__<tool>]` (specific per command, not wildcard) and a body that is a prompt template using `$ARGUMENTS` to invoke the right MCP tool with the right argument shape. Standard CC pattern; researcher confirms exact frontmatter keys via Context7/CC docs.

### HOOK-04b runner (HOOK-04b)

- **D-07: Hook event = `Stop`, not `PostToolUse:Edit|Write|MultiEdit`.** PostToolUse fires per-file: a single find-replace across 50 files would dispatch 50 `famp send` calls and flood the recipient. Stop fires once per turn and coalesces the "Claude finished a turn touching these files" signal into a single dispatch event. The shim reads the turn's modified-files list (researcher confirms exact CC env var or transcript-parsing source — likely `$CLAUDE_TRANSCRIPT_PATH` or equivalent), applies each `~/.famp-local/hooks.tsv` row's glob across the full list, and dispatches **one `famp send`** per matching glob row (not per file). Less spam, more meaningful, no later dedup/throttling debt.

- **D-08: Runner is a bash shim at `~/.famp/hook-runner.sh`, NOT a native Rust `famp hook run` subcommand.** Reasons: (a) ~30 lines of bash (read TSV, glob-match, shell to `famp send`); (b) decouples shim lifecycle from `famp` binary version — an old `famp` binary still works with a fresh shim; (c) avoids growing `famp --help` surface with a hook-internals subcommand; (d) shellcheck + a 5-line bash harness is cheaper than Rust integration tests for this. The shim is installed by `famp install-claude-code` to `~/.famp/hook-runner.sh` (data dir, NOT `scripts/famp-local/` which Phase 4 deprecates) and removed by `famp uninstall-claude-code`. Researcher MUST shellcheck it and decide on `set -euo pipefail` discipline.

- **D-09: hooks.json merge strategy — unique-marker entry under the Stop key.** Marker is the `command` string starting with `~/.famp/hook-runner.sh`. Idempotent: detect any existing famp entry by command-prefix match; replace in-place if present, insert if absent. Never overwrite the whole file. Read → mutate-in-memory → atomic temp-rename, with `.bak` backup (same pattern as D-02). On uninstall, drop only the matching entry. If `~/.claude/hooks.json` doesn't exist, create it with our entry as the sole contents (file format authoritative from CC docs; researcher fetches).

> **AMENDED 2026-05-02 (planner Task 03-02-1):** D-09's reference to `~/.claude/hooks.json` is corrected to `~/.claude/settings.json`. Per official Claude Code docs, the canonical hooks file is `~/.claude/settings.json` under the top-level `"hooks"` key (with sub-keys per event: `"Stop"`, `"PostToolUse"`, etc.). There is no `~/.claude/hooks.json` in the official schema. The merge logic from D-09 stands as written — touch ONLY `settings["hooks"]["Stop"]`, leaving every other settings key (permissions, env vars, model defaults, etc.) untouched. The unique-marker entry (command string starting with `~/.famp/hook-runner.sh`) and atomic .bak + temp-rename pattern are preserved. [Source: 03-RESEARCH.md Pitfall 3 + §"State of the Art" row 2.]

### Install method + acceptance gate (CC-09)

- **D-10: Install path is `cargo install famp` from crates.io.** Phase 3 includes publishing `famp` and the transitive workspace dependencies to crates.io (in correct dependency order). No GitHub release binaries, no Homebrew tap, no `curl | sh`, no notarization apparatus. Reasons: (a) `cargo publish` per release is one command, no separate Action, no signing pipeline, no second repo; (b) crates.io is the standard Rust-distribution path — Rust users expect first-install compile time and second-run-fast experience (`~/.cargo/bin/famp` is fast forever after); (c) macOS binary distribution requires notarization or users get the "unidentified developer" popup — a real product to maintain at v0.1.0 pre-release; (d) brew tap means a second repo + Formula DSL + version bumps in two places, and the user *still* types `brew install thebenlamm/famp/famp` (not `brew install famp`) — small payoff, permanent maintenance burden.

- **D-11: ROADMAP and gate-language amendments (atomic with D-10).** Researcher MUST surface and the planning step MUST land:
  - `.planning/ROADMAP.md` §"v0.9 Phase 3" success criterion 3: replace `brew install famp` with `cargo install famp`.
  - Amend the 30-second gate spirit: the literal stopwatch reading is **second-window install** (after `cargo install famp` has populated `~/.cargo/bin/`). First-time install includes one `cargo install` compile (~60–120s) which is acknowledged as out-of-budget and called out in the README as "first install: a few minutes; subsequent windows: <30s." The 12-line gate stays literal — README content is ≤12 user-visible lines. ROADMAP wording adjustment is researcher's exact draft, both options preserve the spirit ("crisp, copy-paste, no manual edits").

  > Punt brew/binary-installer to v0.9.1 if user demand emerges. Don't pre-build for hypothetical demand.

### Codex parity (out-of-band scope addition)

- **D-12: Ship `famp install-codex` + `famp uninstall-codex` in this phase.** Same direct-JSON-edit pattern as `install-claude-code` (researcher confirms Codex's MCP config target — likely `~/.codex/config.toml` or `~/.codex/mcp.json`; format may be TOML or JSON; spec verification required). ~50 LoC. Codex gets **MCP-only** — no slash commands, no hooks (Codex doesn't expose those primitives the same way). The pattern generalizes: future `famp install-cursor` / `famp install-continue` follow the same template, shipped on demand.

  > **Note:** README Quick Start (D-11) gates on Claude Code only — Codex parity is an additive surface for symmetry, not part of the 12-line gate. Codex-specific install is one line in `docs/ONBOARDING.md`.

### `docs/ONBOARDING.md` scope (CC-10)

- **D-13: Minimal scope, ≤80 lines, three sections.** Final outline:
  1. **Install** (`cargo install famp` + the 12-line Quick Start verbatim from README).
  2. **Other clients** (Codex: one line — `cargo install famp && famp install-codex`; Cursor/Continue: "open an issue if you need this").
  3. **Uninstall** (`famp uninstall-claude-code` / `famp uninstall-codex` — clean removal).
  Plus one closing pointer: "for the protocol, see [`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](...). For commands, run `famp --help` and `famp-local --help`."

  > **Explicitly OUT of v0.9.0 ONBOARDING.md:**
  > - Channels deep-dive (ChannelEvent broadcast already deferred to v0.9.1; semantics may shift).
  > - Hooks deep-dive (covered by `famp-local hook --help`).
  > - Troubleshooting section (rots fastest; grow from real reports in a separate `TROUBLESHOOTING.md` if needed).
  > - Multi-agent workflow examples (premature; wait for Sofer-equivalent usage to surface real patterns).

### Claude's Discretion

- Exact Rust crate (or hand-rolled JSON walker) for `~/.claude.json` mutation. Recommend `serde_json::Value` walking with `Map`-key idempotency check; researcher may pick a JSON-Patch crate (`json-patch`) if it produces cleaner code.
- Exact `~/.claude/hooks.json` schema (objects vs arrays of hook configs under Stop key) — researcher fetches from CC docs and locks the merge logic.
- Bash idioms in `~/.famp/hook-runner.sh` — `set -euo pipefail`, error-on-malformed-TSV-row vs skip-and-warn, glob matcher (bash `case`/`extglob` vs `globstar`); shellcheck-clean is the only hard requirement.
- crates.io publish ordering — researcher determines the dependency graph (likely `famp-canonical → famp-crypto → famp-core → famp-fsm → famp-envelope → famp-bus → famp-keyring → famp-transport → famp-transport-http → famp`) and validates each crate's Cargo.toml is publishable (description, license, repository, no `path = ".."` deps without versions).
- crates.io name conflict check — surface BLOCKING discovery if `famp` or any workspace crate name is taken; pick a fallback (`famp-cli`, `famp-rs`, etc.) before any plan locks.
- Codex config target — `~/.codex/config.toml` vs `~/.codex/mcp.json` vs `$XDG_CONFIG_HOME/codex/...`; researcher fetches Codex docs and locks.
- Slash-command frontmatter exact keys (`allowed-tools` vs `tools` vs other) — researcher fetches CC docs.
- README rewrite line-by-line content — must hit the 12-line literal count; researcher drafts and counts.
- Whether `famp install-claude-code` automatically suggests / runs `cargo install famp` if `famp` itself isn't on PATH — recommend NO (don't recursively self-install), but worth flagging in the install summary printout.
- Whether the slash commands need scoped argument validation in their markdown body, or rely on the MCP tool's schema. Recommend rely-on-tool-schema; surface tool errors directly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Design authority
- `docs/superpowers/specs/2026-04-17-local-first-bus-design.md` — the v0.9 local-first bus design spec (506 lines). **§"CLI surface"** (313–329) and **§"MCP surface"** (331–354) are the v0.9 user-facing contract; the slash commands and `install-claude-code` mutation pattern derive from them. **§"Phasing / Phase 3"** (412–423) is the exit criteria for this phase.

### Spec & invariants
- `FAMP-v0.5.1-spec.md` (project root, amended in-place to v0.5.2 by commit `f44f3ee`) — wire format authority. Phase 3 does NOT amend the spec; slash commands and the install subcommand are pure user-facing wrappers around the existing 8-tool MCP surface.

### Requirements
- `.planning/REQUIREMENTS.md` §CC, HOOK-04b — the 11 active requirements for this phase. **CC-05 wording amendment required** (D-05): `/famp-msg` → `/famp-send`.
- `.planning/ROADMAP.md` §"v0.9 Phase 3: Claude Code integration polish" — five-bullet success-criteria block. **Two amendments required:**
  - Success criterion 2 (D-05): `/famp-msg` → `/famp-send`.
  - Success criterion 3 (D-11): `brew install famp` → `cargo install famp`; gate spirit pinned to second-window install.

### Phase 2 substrate (the artifact this phase wraps)
- `.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-CONTEXT.md` — Phase 2 decisions. **D-04 (MCP rewire architecture), D-05 (`famp_register` gating), D-06 (`BusErrorKind` exhaustive match), D-08 (`famp register` foreground UX), D-10 (`bind_as` connection property), D-11 (MCP-01 audit is source-import grep), D-12 (HOOK-04 split into 04a/04b)** are hard constraints on Phase 3.
- `.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-VERIFICATION.md` — Phase 2 PASS evidence; the 8-tool MCP surface and `famp-local hook add/list/remove` registration are verified shipped.

### Phase 1 substrate (carry-forward)
- `crates/famp-bus/src/proto.rs` — `BusMessage`, `BusReply`, `Target`, `BusErrorKind` definitions. Phase 3 changes nothing here.

### Claude Code documentation (researcher MUST fetch + add to research)
- Claude Code MCP config at `~/.claude.json` — schema for `mcpServers.<name>` entries (command, args, env). Use Context7 `mcp__context7__resolve-library-id` + `query-docs` against "Claude Code" or fetch from official Anthropic docs.
- Claude Code slash commands — markdown file format in `~/.claude/commands/`, frontmatter keys (`allowed-tools`?), `$ARGUMENTS` substitution semantics.
- Claude Code hooks — `~/.claude/hooks.json` schema, `Stop` event semantics, what env vars / transcript paths the shim has access to.

### Codex documentation (researcher MUST fetch + add to research)
- Codex MCP config target file path + format (TOML vs JSON).

### crates.io publishability
- Each workspace member's `Cargo.toml` — description, license (`Apache-2.0 OR MIT` already set), repository field, version pin compatibility. Researcher MUST verify all `path = ".."` deps also have published `version = "x.y.z"` constraints, or add them.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`famp uninstall-claude-code` mirrors `famp install-claude-code`** — write the install code as a pure `mutate(state) -> state'` function and let uninstall be `inverse_mutate`. Keeps install↔uninstall symmetry compiler-checked.
- **Phase 2 `bus_client::spawn` module** — already centralizes `posix_spawn`-style broker spawning. The hook-runner shim's `famp send` invocations ride on it transparently; no Phase 3 work needed.
- **Phase 2 MCP server in `crates/famp/src/cli/mcp/`** — 8-tool surface unchanged. Slash commands are pure markdown templates; Phase 3 does not modify any `.rs` file in `cli/mcp/`.
- **`scripts/famp-local hook add/list/remove`** (Phase 2 D-12 / HOOK-04a) — hook *registration* surface ships and stable. Phase 3 only adds the *runner* shim consuming `~/.famp-local/hooks.tsv`.

### Established Patterns
- **Atomic-write + .bak backup + idempotent merge** is the repeated pattern for all three target files (`~/.claude.json`, `~/.claude/hooks.json`, future Codex config). Researcher should factor this into a small reusable helper module (`crates/famp/src/install/json_merge.rs` or similar) — used by `install-claude-code`, `install-codex`, `uninstall-*`.
- **Snapshot tests via `insta`** — already in use across workspace. The install/uninstall round-trip and each JSON-mutation test should ship as `insta` snapshots, not handcrafted assertions.
- **Direct + override env var** mirrors Phase 2 D-07 (`$FAMP_BUS_SOCKET` overrides `~/.famp/bus.sock`). For test isolation in Phase 3: `$HOME` redirection (or `$FAMP_INSTALL_TARGET_HOME`) lets integration tests run install→uninstall against a tempdir without touching the real `~/.claude.json`.
- **`feedback_identity_naming`** (memory) — default + override path is non-optional. Applied here: install subcommand uses sensible defaults (user-scope, standard paths) but every path is overridable via env var or flag.

### Integration Points
- `crates/famp/src/cli/mod.rs` — adds 4 new subcommands (`install-claude-code`, `uninstall-claude-code`, `install-codex`, `uninstall-codex`). Stay clippy-clean.
- `~/.claude.json` (existing user file or freshly created) — read, mutate `mcpServers.famp`, atomic write.
- `~/.claude/commands/famp-*.md` (7 new files) — write fresh; uninstall removes them.
- `~/.claude/hooks.json` (existing or freshly created) — merge sentinel-tagged Stop entry; atomic write.
- `~/.famp/hook-runner.sh` (new file) — bash shim, mode 0755, owned by user.
- `~/.famp-local/hooks.tsv` — read-only from this phase's perspective; Phase 2 owns the write side.
- `Cargo.toml` (all workspace members) — researcher audits `description` / `repository` / `path` deps for crates.io publishability.

</code_context>

<specifics>
## Specific Ideas

- **The 30-second gate is *spiritual*, the 12-line gate is *literal*.** D-11 splits these: the line count is enforced byte-for-byte; the wall-clock is enforced on second-window install (after `cargo install famp` populated `~/.cargo/bin/`). First-install includes a `cargo install` compile, called out in README.
- **No drive-by README polish.** Per the user's surgical-changes preference (CLAUDE.md): every README change traces to the 12-line gate or the install-method amendment. No reformatting of unrelated sections.
- **Codex parity is symmetric, not minimal.** `famp install-codex` ships even though the Quick Start gate is Claude-Code-only — symmetry is self-discoverable from `famp --help`, and the same 50-line direct-JSON-edit pattern un-blocks Cursor/Continue when they show up.
- **The hook-runner shim must NEVER fail the Stop hook.** A malformed TSV row, a missing `famp` binary on PATH, or a broken pipe to the broker should log + exit 0 (Stop hook completes; CC doesn't get blocked). Recipient may miss the message; Claude's session is not interrupted. Researcher confirms with shellcheck-clean error handling.
- **Install snapshot tests run against `$HOME=$TMPDIR/install-test/`.** No real `~/.claude.json` modification in CI.
- **No partial uninstall states.** Uninstall is all-or-nothing; if any step fails, log the error and exit non-zero with the system left in either fully-installed or fully-uninstalled state (no half-removed slash commands).

</specifics>

<deferred>
## Deferred Ideas

- **Homebrew tap (`homebrew-thebenlamm-famp`) + `brew install thebenlamm/famp/famp`** — defer to v0.9.1 if user demand emerges. Adds a second repo + Formula DSL + version-bump-in-two-places ceremony for a marginal user-perceived gain (`brew install thebenlamm/famp/famp`, not `brew install famp`).
- **GitHub release binaries (macOS arm64 + x86_64) with `curl | sh`** — defer to v0.9.1 if user demand emerges. Requires a GH Action, macOS notarization, signing — a real product to maintain pre-1.0.
- **Homebrew core formula** (`brew install famp` no-tap) — Anthropic-style upstream submission. Wait until v1.0+ and a real install-volume signal.
- **`famp install-cursor` / `famp install-continue`** — same direct-JSON-edit pattern, ship when a real user hits it.
- **Channels deep-dive in ONBOARDING.md** — defer to v0.9.1 after ChannelEvent broadcast lands and channel semantics stabilize.
- **Hooks deep-dive in ONBOARDING.md** — `famp-local hook --help` covers the registration surface; deeper docs grow from real recipe requests.
- **`docs/TROUBLESHOOTING.md`** — grow from real user-report incidents, don't speculate.
- **Multi-agent workflow examples** — wait for Sofer-equivalent usage to surface real patterns. Premature documentation locks an architecture the implementation hasn't validated.
- **CC-09 30-second gate as literal wall-clock including first install** — D-11 amendment makes this second-window-only. If the user later wants the literal first-install measurement, the binary-release path (deferred above) is the route.
- **Project-scope `.mcp.json` provision** — v0.9 is single-host single-user; multi-tenant scope belongs to v1.0+ if ever.

</deferred>

---

## Preconditions / Blockers (read before plan-locks)

- **🟡 PARKED: Architect counsel on `famp send` audit_log wrapper.** Per `.planning/STATE.md` "Open question" entry (2026-04-30): `crates/famp/src/cli/send/mod.rs::build_envelope_value` wraps every local DM/deliver/channel-post payload as an unsigned `audit_log` `BusEnvelope`. Three options on the table: (1) accept as v0.9 convention; (2) add `MessageClass::BusDm`/`LocalRequest` (v0.5.3 amendment, AUDIT-05 atomic-bump); (3) loosen Phase 1 D-09 to accept untyped local payloads. Phase 3 doesn't touch `build_envelope_value` directly, but the slash commands invoke `famp_send` MCP tool — which routes to the same wrapper. **Researcher MUST surface this in `03-RESEARCH.md` if unresolved at plan-lock; planner MUST NOT lock the slash-command argument shape (D-05) without architect counsel resolved**, since option (2) might add a `class` argument to `famp_send` that the slash commands need to pass through. Lean is option (1) per STATE.md, in which case Phase 3 is unblocked and the wrapper stays as-is.

- **🟡 BLOCKING-RESEARCH: crates.io name availability for `famp` and all workspace members.** D-10 requires publishing to crates.io. If `famp` (or `famp-bus`, `famp-canonical`, etc.) is taken, that's a forced rename across the entire workspace before any plan locks. Researcher MUST run `cargo search` (or hit crates.io API directly) for every workspace crate name and surface conflicts at the top of `03-RESEARCH.md`.

- **🟡 BLOCKING-RESEARCH: each workspace `Cargo.toml` is publishable.** Audit `description` / `license` / `repository` / `path = ".."` deps with version constraints / no unpublished dev-deps in normal-deps. Surface gaps as remediation-tasks for the planner.

---

*Phase: 03-claude-code-integration-polish*
*Context gathered: 2026-05-02*
