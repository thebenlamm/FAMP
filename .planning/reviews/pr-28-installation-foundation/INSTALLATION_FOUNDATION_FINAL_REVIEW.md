# Final pre-commit review — installation foundation

Review of the corrected uncommitted installation-foundation change in the local checkout of `thebenlamm/FAMP`.

Reviewer did not modify files, format, stage, commit, push, create branches, create issues, or open a pull request.

---

## Initial verification

| Check | Result |
|---|---|
| Remote | `thebenlamm/FAMP` |
| Branch | `main...origin/main` (original) |
| Staged | **nothing** (`git diff --cached` empty) |
| Product code | uncommitted (modified + required untracked) |
| Required untracked | `crates/famp/src/cli/executable.rs` (650 lines), `crates/famp/tests/claude_grok_hook_coexistence.rs` (168 lines) |
| Scratch (exclude) | `INSTALLATION_FOUNDATION_{REPORT,CODE_REVIEW,CORRECTIONS}.md` |
| `git diff --check` | clean |

Tracked: **24 files, +1303 / −311**. Effective with required untracked: **~+2121 / −311**.

Commands:

```bash
git remote -v
git status --short --branch
git diff --check
git diff --stat
```

---

## Prior HIGH / MEDIUM resolution table

| Prior finding | Status | Evidence |
|---|---|---|
| **H1** Stale Cargo-fallback docs | **RESOLVED** | `docs/CONFIGURATION.md` new section “Resolution precedence…” — steps 1–4, explicit “no `~/.cargo/bin/famp` fallback”, cargo as example only; install flags table updated; Claude MCP example annotated. |
| **H2** launchd loaded-job repair | **RESOLVED** | `service_file_outcome` / `needs_reload_advisory` (`daemon/install.rs`); macOS path classifies before write, prints single advisory naming `famp daemon restart`; pure unit tests; CONFIGURATION “Reinstalling with a different famp binary (macOS)” + accurate Linux “milder but not exempt” note. `daemon/restart.rs` still does bootout+bootstrap+kickstart. |
| **M1** exact current-exe filename | **RESOLVED** | `is_famp_executable_name_for` — Unix exact `famp`, Windows case-insensitive `famp.exe`; rejects `famp.bak` / `famp.1`; tests for convention matrix + current harness not accepted. |
| **M2** public-path no-mutation | **RESOLVED** | `public_run_at_fails_before_any_mutation_*` in claude/codex/grok/daemon; all call public `run_at` / `run_at_project`; force fail via `FAMP_INSTALL_FAMP_BIN` (`MISSING_FAMP_BIN`, whitespace, directory); `temp_env::with_var` (WR-06); full-tree snapshot under home. |
| **M3** Claude/Grok ownership | **RESOLVED** | `tests/claude_grok_hook_coexistence.rs` — dual pin A/B, uninstall each side, reinstall Claude → Grok unchanged. |
| **M4** Codex override execution intentional | **RESOLVED** | `probe_codex_stop_support` docs: never skipped; override trusted as install invocation; 5s timeout retained. |
| **M5** MCP exhaustive fixture | **RESOLVED** | `mcp_error_kind_exhaustive.rs` constructs `CliError::FampExecutable(NotFound)` + spot-check `famp_executable_error`. |
| **M6** expanded resolver matrix | **RESOLVED** | 12 resolver tests (was 6): filename convention, stem skip, leading/trailing spaces, non-UTF-8 env, cwd failure, cargo-shaped path only when selected. |

No prior HIGH/MEDIUM remains open.

---

## New findings (correction-pass residue only)

### LOW — `posix_shell_literal` is crate-public library API

1. **Where:** `cli/mod.rs` → `pub mod executable`; `executable.rs` `pub fn posix_shell_literal`  
2. **Behavior:** Reachable as `famp::cli::executable::posix_shell_literal` from any dependent (and from integration tests).  
3. **Scenario:** External callers may treat a quoting helper as stable API; crate’s intentional public surface is still protocol re-exports + already-public `cli`.  
4. **Smallest fix (optional):** keep as-is (consistent with `pub mod cli` test surface) **or** reimplement the one-liner in the coexistence test and use `pub(crate)`.  
5. **Before commit?** **No** — justified by existing `pub mod cli` integration-test architecture; not a behavioral risk.

### NIT

- `executable.rs` is 650 lines (~247 production / ~403 tests+support); size is mostly tests, not over-abstraction.  
- `DaemonError::FampExecutable(String)` still stringifies (flattened chain) — acceptable for lifecycle errors.  
- macOS-target `cargo check` not run (no Apple target installed).  
- `shellcheck` unavailable; `bash -n` on both source templates passes.  
- Raw templates contain `FAMP_BIN=@FAMP_BIN@` (valid assignment of a literal token); production always renders before install.

No BLOCKER or HIGH findings in the corrected tree.

---

## Spot checks (abbreviated)

### Resolver

- Precedence: override → exact name `current_exe` → `which famp` → `NotFound`.  
- Empty/whitespace override: `EmptyExplicit`, no fallback.  
- Spaces preserved; UTF-8 stored at validate time; symlink path preserved; no `canonicalize` on binary.  
- `FampExecutable` / locator / resolve: `pub(crate)`; error enum `pub` (needed for `CliError`).  
- No test-only APIs in non-`cfg(test)` production (except comments).

### Hooks

- Exactly one `@FAMP_BIN@` per asset; production `str::replace` non-recursive.  
- Hostile-path suite uses **real** `install_shim`, then independent `bash -n` + evaluation + canary.  
- No `command -v famp` / cargo fallback / bare `famp` in rendered hooks.

### No-mutation tests

- Force override → resolver never reaches `current_exe`/PATH.  
- `temp_env` serializes global env.  
- Snapshot includes pre-seeded configs/shims/service files.

### Launchd advisory

- Classification before write; advisory only if `Updated && already_loaded`.  
- `launchctl print` is read-only.  
- Docs + restart path match code.

### Codex

- Probe before mutation; missing-file skip removed; uninstall reads MCP `command` before remove.  
- ETXTBSY warm-up is **test-only**.

### Docs

- Precedence, validation, per-installer artifacts, macOS caveat, Linux non-overstatement: accurate vs code.

### Scope

All changed files are essential production, tests, docs, or justified supporting (uninstall codex/claude note, mcp error kind). No Homebrew/packaging/doctor/migration. Large but **one coherent PR** — no split recommended.

---

## Final verdict

# APPROVE

### 1. Resolution table

All eight prior HIGH/MEDIUM items: **RESOLVED** (see table above).

### 2. Minimum fixes before commit

**Product code: none.**

Staging hygiene only:

1. `git add` `crates/famp/src/cli/executable.rs` and `crates/famp/tests/claude_grok_hook_coexistence.rs`  
2. Do **not** add the three `INSTALLATION_FOUNDATION_*.md` scratch files  

### 3. Optional follow-ups

- Document `posix_shell_literal` as test-oriented or shrink to `pub(crate)` + local test helper.  
- macOS CI smoke for advisory print path.  
- `shellcheck` on **rendered** hooks in CI.

### 4. Accidental public API?

`pub mod executable` + `pub fn posix_shell_literal` + `pub enum FampExecutableError` are reachable outside the crate. Given existing **`pub mod cli`** (already used by integration tests), this is **intentional expansion of the test/CLI surface**, not a silent new protocol API. Residual: **LOW**, not blocking.

### 5. Coherent single PR?

**Yes.**

### 6. Commands and results

| Command | Result |
|---|---|
| `git remote/status/diff --check/--stat` | FAMP main; uncommitted; check clean; +1303/−311 tracked |
| `cargo fmt --all -- --check` | pass |
| `cargo check -p famp --all-targets` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cli::executable::tests` | **12** passed |
| `cli::install::claude_code::tests` | **8** passed |
| `cli::install::codex::tests` | **8** passed |
| `cli::install::grok::tests` | **8** passed |
| `cli::install::await_hook::tests` | **19** passed |
| `cli::daemon::install::tests` | **8** passed |
| `mcp_error_kind_exhaustive` | **5** passed |
| `claude_grok_hook_coexistence` | **3** passed |
| install/roundtrip integration binaries | **15** passed (claude 3, grok 3, roundtrip 3, codex 4, grok-roundtrip 2) |
| `bash -n` both assets | pass |
| `shellcheck` | not installed |
| `git diff --check` | pass |

### 7. Effective diff size

Tracked **+1303 / −311** plus required untracked **~818 lines** → roughly **+2121 / −311** effective.

### 8. Files that should be committed

```
crates/famp/assets/famp-await.sh
crates/famp/assets/hook-runner.sh
crates/famp/src/cli/daemon/install.rs
crates/famp/src/cli/error.rs
crates/famp/src/cli/executable.rs          # untracked — must add
crates/famp/src/cli/install/await_hook.rs
crates/famp/src/cli/install/claude_code.rs
crates/famp/src/cli/install/codex.rs
crates/famp/src/cli/install/grok.rs
crates/famp/src/cli/install/hook_runner.rs
crates/famp/src/cli/mcp/error_kind.rs
crates/famp/src/cli/mod.rs
crates/famp/src/cli/uninstall/claude_code.rs
crates/famp/src/cli/uninstall/codex.rs
crates/famp/tests/claude_grok_hook_coexistence.rs  # untracked — must add
crates/famp/tests/codex_install_uninstall_roundtrip.rs
crates/famp/tests/grok_install_uninstall_roundtrip.rs
crates/famp/tests/hook_runner_await.rs
crates/famp/tests/hook_runner_dispatch.rs
crates/famp/tests/hook_runner_failure_modes.rs
crates/famp/tests/hook_runner_path_parity.rs
crates/famp/tests/install_claude_code.rs
crates/famp/tests/install_grok.rs
crates/famp/tests/install_uninstall_roundtrip.rs
crates/famp/tests/mcp_error_kind_exhaustive.rs
docs/CONFIGURATION.md
```

### 9. Scratch files to exclude

```
INSTALLATION_FOUNDATION_REPORT.md
INSTALLATION_FOUNDATION_CODE_REVIEW.md
INSTALLATION_FOUNDATION_CORRECTIONS.md
```

(Also exclude this final review file unless intentionally published.)

### 10. `git status --short` (at review end)

```text
 M crates/famp/assets/famp-await.sh
 M crates/famp/assets/hook-runner.sh
 M crates/famp/src/cli/daemon/install.rs
 M crates/famp/src/cli/error.rs
 M crates/famp/src/cli/install/await_hook.rs
 M crates/famp/src/cli/install/claude_code.rs
 M crates/famp/src/cli/install/codex.rs
 M crates/famp/src/cli/install/grok.rs
 M crates/famp/src/cli/install/hook_runner.rs
 M crates/famp/src/cli/mcp/error_kind.rs
 M crates/famp/src/cli/mod.rs
 M crates/famp/src/cli/uninstall/claude_code.rs
 M crates/famp/src/cli/uninstall/codex.rs
 M crates/famp/tests/codex_install_uninstall_roundtrip.rs
 M crates/famp/tests/grok_install_uninstall_roundtrip.rs
 M crates/famp/tests/hook_runner_await.rs
 M crates/famp/tests/hook_runner_dispatch.rs
 M crates/famp/tests/hook_runner_failure_modes.rs
 M crates/famp/tests/hook_runner_path_parity.rs
 M crates/famp/tests/install_claude_code.rs
 M crates/famp/tests/install_grok.rs
 M crates/famp/tests/install_uninstall_roundtrip.rs
 M crates/famp/tests/mcp_error_kind_exhaustive.rs
 M docs/CONFIGURATION.md
?? INSTALLATION_FOUNDATION_CODE_REVIEW.md
?? INSTALLATION_FOUNDATION_CORRECTIONS.md
?? INSTALLATION_FOUNDATION_REPORT.md
?? crates/famp/src/cli/executable.rs
?? crates/famp/tests/claude_grok_hook_coexistence.rs
```

### 11. Review modifications

**None.** This review did not edit, format, stage, or commit any product files. This document is a write-only review artifact.
