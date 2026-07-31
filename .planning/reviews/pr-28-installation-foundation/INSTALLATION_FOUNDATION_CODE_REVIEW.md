# Adversarial Code Review: FAMP install-time executable resolver

Review of uncommitted working-tree changes in `thebenlamm/FAMP` (local checkout).  
Reviewer did not modify product files, create branches, commit, push, or open a PR.

---

## Initial verification

| Check | Result |
|---|---|
| Remote | `thebenlamm/FAMP` (`git@github.com:thebenlamm/FAMP.git`) |
| Branch | `main...origin/main`, uncommitted changes only |
| Untracked resolver | `?? crates/famp/src/cli/executable.rs` (will **not** appear in ordinary `git diff`) |
| Extra untracked | `?? INSTALLATION_FOUNDATION_REPORT.md` (implementation report; not code) |
| Diff stat (tracked) | 12 files, **+334 / −184** |
| With untracked resolver (~337 lines) | ≈ **+671** — matches the report’s insertion claim |
| `git diff --check` | clean |
| Unrelated pre-existing edits | **None** — every tracked change is on the install/daemon/error surface |

Commands:

```bash
git remote -v
git status --short --branch
git diff --stat
git diff --check
```

---

## Intended PR scope (review baseline)

> Add one shared, tested abstraction for resolving the FAMP executable path embedded into generated daemon and AI-tool configuration. Remove hard-coded and fallback assumptions about `~/.cargo/bin/famp`. Make resolution failures occur before configuration or service mutation. Render Claude and Grok hook scripts with the same resolved absolute executable path.

Expected production precedence:

1. `FAMP_INSTALL_FAMP_BIN`
2. `current_exe()`, only when the executable stem is `famp`
3. `which famp`
4. actionable failure

---

## Findings (severity order)

### HIGH

#### H1. Stale operator docs still describe the removed cargo fallback

1. **Severity:** HIGH  
2. **Where:** `docs/CONFIGURATION.md:269` (and illustrative path at `:292`)  
3. **Behavior:** Documents  
   `FAMP_INSTALL_FAMP_BIN` as skipping `current_exe` / `PATH` / **`~/.cargo/bin/famp`**.  
   That third fallback is **gone**. Missing override + no `famp` stem + no PATH hit now **fails**, it does not invent `~/.cargo/bin/famp`.  
4. **Why it matters:** Operators and tests will follow wrong recovery advice after install failure.  
5. **Repro:** Unset `FAMP_INSTALL_FAMP_BIN`, remove `famp` from PATH, run a non-`famp`-named binary → `NotFound`, not a cargo path.  
6. **Fix:** Rewrite the env table to the four-step precedence; note fail-closed validation; keep cargo only as an example path.  
7. **This PR?** **Yes** — documents the contract this PR changes.

#### H2. macOS `daemon install` rewrites the plist but does not reload an already-registered job

1. **Severity:** HIGH (pre-existing, **made more important** by path repair goals)  
2. **Where:** `load_macos` in `crates/famp/src/cli/daemon/install.rs` (~262–266)  
3. **Behavior:** If `com.famp.broker` is already registered, bootstrap is skipped. Plist on disk is updated; **live launchd ProgramArguments are not**.  
4. **Why it matters:** Constraint: reinstall should repair generated paths. On macOS the file repairs; the running service can keep the old binary until bootout/bootstrap. Linux gets `daemon-reload` + `enable --now` (still may not hard-restart an active unit — milder).  
5. **Repro:** Install daemon → move/replace resolved `famp` → re-run `famp daemon install` with new `FAMP_INSTALL_FAMP_BIN` → inspect loaded job vs plist.  
6. **Fix:** On already-registered + plist content change: bootout + bootstrap (or document “file updated; run restart/uninstall+install”). Prefer explicit reload when `ProgramArguments[0]` changed.  
7. **This PR?** **Should be addressed or explicitly deferred** in this PR’s notes; full reload policy can be a follow-up if documented.

---

### MEDIUM

#### M1. `current_exe` uses `file_stem() == "famp"`, not exact filename

1. **Severity:** MEDIUM  
2. **Where:** `resolve_with` in `executable.rs:93–96`  
3. **Behavior:** Accepts `famp`, and also `famp.exe`, `famp.bak`, `famp.1` (stem `famp`). Old Codex path used `file_name() == "famp"`.  
4. **Why it matters:** A backup/binary named `famp.bak` running as current process would be pinned into configs.  
5. **Repro:** Rename binary to `famp.bak`, run it as install entrypoint with empty PATH.  
6. **Fix:** Prefer `file_name() == "famp"`, or Windows-only `stem == "famp" && ext in {None, Some("exe")}`.  
7. **This PR?** Yes if you care about stem strictness; low probability in practice.

#### M2. Negative “no mutation” coverage mostly bypasses the public resolver

1. **Severity:** MEDIUM  
2. **Where:** Claude/Grok/Codex unit tests inject `FampExecutable::validate(...)` into `*_with_executable`; Codex probe test similarly.  
3. **Behavior:** Live check shows public path is correct (`install-claude-code` with bad/empty override creates zero files). **Tests do not lock that.**  
4. **Why it matters:** Future reordering can reintroduce mutation-before-resolve without red tests.  
5. **Repro:** None today; gap only.  
6. **Fix:** One `temp_env::with_var("FAMP_INSTALL_FAMP_BIN", ...)` test per public `run_at` / `run_at_project` / daemon `run_at` asserting empty target tree on `NotFound` / `EmptyExplicit` / `Metadata`.  
7. **This PR?** Yes for the critical guarantee this PR claims.

#### M3. Shared await-hook **source**, separate destinations — correct, but untested ownership story

1. **Severity:** MEDIUM (risk area; implementation is OK)  
2. **Where:** Claude → `~/.claude/hooks/famp-await.sh` + `~/.famp/hook-runner.sh`; Grok → `~/.grok/hooks/famp-await.sh`  
3. **Behavior:** Not one shared installed file. Different pins are last-installer-wins **per tool**, not cross-tool corruption. Uninstall Claude does not remove Grok’s shim (and vice versa).  
4. **Why it matters:** Easy to get wrong; worth an explicit dual-install/uninstall test.  
5. **Repro (desired test):** Install Claude with bin A, Grok with bin B; assert both pins; uninstall one; other remains.  
6. **Fix:** Add that integration/unit fixture.  
7. **This PR?** Recommended; not a functional bug now.

#### M4. Codex capability probe executes whatever the resolver selected (including override)

1. **Severity:** MEDIUM (largely pre-existing contract, **amplified** by validated arbitrary override)  
2. **Where:** `probe_codex_stop_support` in `codex.rs:226–255`  
3. **Behavior:** Spawns `<bin> hook codex-stop --help` with 5s kill timeout. Override → install-time code execution of that path.  
4. **Why it matters:** Expected for “install with this binary”; poisoned `FAMP_INSTALL_FAMP_BIN` in a shared env is install-time RCE-as-user. Same class as running any installer.  
5. **Repro:** Point override at a script that touches a canary; run `install-codex`.  
6. **Fix:** Document as intentional; optional stricter “must look like famp --help” checks.  
7. **This PR?** Docs/comment; not a redesign blocker.

#### M5. `mcp_error_kind` gains `famp_executable_error` without exhaustive fixture update

1. **Severity:** MEDIUM (low runtime impact)  
2. **Where:** `cli/mcp/error_kind.rs:49`; `tests/mcp_error_kind_exhaustive.rs` has **no** `FampExecutable` fixture  
3. **Behavior:** Compile-time match is exhaustive (good). Runtime uniqueness test does not construct the new variant.  
4. **Why it matters:** Kind-string uniqueness is only partially enforced by the test harness.  
5. **Repro:** Add a second variant with the same kind string later — fixture list still green.  
6. **Fix:** Add `CliError::FampExecutable(NotFound)` (or similar) to `variants_*`.  
7. **This PR?** Yes, small.

#### M6. Resolver unit matrix is only **six** tests; several requested cases missing

1. **Severity:** MEDIUM  
2. **Where:** `executable.rs` tests  
3. **Present:** precedence, invalid explicit no-fallback, relative+spaces, symlink preserve + not-file + not-exec, non-UTF-8 path, basic posix quoting  
4. **Absent (notable):**  
   - override with **leading/trailing spaces that are real path characters** (live OK, no test)  
   - non-UTF-8 **env value** (path non-UTF-8 covered)  
   - newline-containing path end-to-end into hooks  
   - cwd failure (`Absolute`)  
   - `file_name`/`file_stem` edge cases  
   - public ProcessLocator / no global env mutation proof beyond FakeLocator  
   - systemd tab/newline (whitespace helper covers via `is_whitespace`, thin coverage)  
   - concurrent replace after validate (TOCTOU — acceptable omit)  
5. **Fix:** Expand matrix for env semantics + public no-mutation + one newline shell render test.  
6. **This PR?** Strongly preferred for the stated quality bar.

---

### LOW

#### L1. Lexically absolute relative paths keep `./` noise

- **Where:** `absolute_and_validate`  
- **Example (live):** `./rel-famp` → `/tmp/..././rel-famp` embedded in config  
- **Impact:** Works; slightly ugly; no canonicalize (symlink-preserving — intentional)  
- **Fix:** Optional `components().collect()` normalize without resolving symlinks  
- **This PR?** Optional

#### L2. `CliError::FampExecutable` double-prints via source walk

- **Live:**  
  `FAMP executable resolution failed: …` then `caused by: …` same text  
- **Where:** `error.rs` `#[error("... {0}")]` + `#[from]` + `famp.rs` source loop  
- **Fix:** `#[error(transparent)]` or omit `{0}` and rely on source  
- **This PR?** Nice polish

#### L3. `utf8()` uses `unwrap_or_default()` after construction-time UTF-8 check

- **Where:** `FampExecutable::utf8`  
- **Impact:** Broken invariant → empty command string silently  
- **Fix:** Store `String` at validate time, or `expect`  
- **This PR?** Optional

#### L4. `DaemonError::FampExecutable(String)` stringifies and loses typed source

- **Where:** `daemon/install.rs` map_err  
- **Impact:** Weaker diagnostics only  
- **Fix:** `FampExecutable(#[from] FampExecutableError)` if cycles allow  
- **This PR?** Optional

#### L5. Stale comment on Codex probe “skipped when binary missing”

- **Where:** `codex.rs` probe docs (~221–225)  
- **Reality:** Resolver always validates existence; probe always runs  
- **This PR?** Yes, comment fix

#### L6. Uninstall Claude still mentions `~/.cargo/bin` for binary removal

- **Where:** `uninstall/claude_code.rs` note  
- **This PR?** Optional consistency

#### L7. Remaining `~/.cargo/bin/famp` mentions

- **Production install path:** removed from Claude/Grok/Codex/daemon generation  
- **Still present:** tests/fixtures, daemon restart cmdline tests, docs, historical plans — mostly fine  
- **Bus auto-spawn:** `bus_client/spawn.rs` still uses `current_exe()` — **correctly out of scope**

#### L8. `INSTALLATION_FOUNDATION_REPORT.md` untracked

- Implementation scratch; should not land as product docs without intent  
- **This PR?** Exclude from commit unless wanted

---

### NIT

- Duplicated “make temp executable” helpers across modules (fine for now).  
- No assert that `@FAMP_BIN@` appears exactly once before replace.  
- `posix_shell_literal` only one unit assertion (live `/bin/sh` round-trip for spaces, quotes, `$`, backticks, `\`, newlines, globs, Unicode **passed** in review).  
- Hook assets remain **bash** scripts; quoting is POSIX-correct; `hook-runner` uses bashisms (`<<<`) — pre-existing, shebang is bash.

---

## Areas that appear correct

| Area | Assessment |
|---|---|
| **Precedence** | Override → stem-`famp` `current_exe` → `which famp` → `NotFound` |
| **Empty / whitespace-only override** | Fail closed (`EmptyExplicit`); no fallback |
| **Path not trimmed** | Leading/trailing spaces preserved (live-verified trailing space filename) |
| **Relative paths** | Joined with `current_dir`; no accidental `canonicalize` on the binary |
| **Symlinks** | `metadata` follows for validation; stored path is the link path |
| **Relative symlink targets** | Kernel resolves vs link parent — safe to keep absolute link path |
| **UTF-8 / non-file / non-exec** | Enforced; wrapper scripts allowed (intentional) |
| **No unchecked constructor** | Only `validate_candidate` builds `FampExecutable` |
| **POSIX quoting** | Correct `'` / `'\''` strategy; safe for `/bin/sh` assignment |
| **Template injection** | Single `@FAMP_BIN@` replace; path cannot re-open token pass |
| **Mutation-before-fail (Claude/Grok/Codex resolve)** | Resolve first; live no-mutation on bad override |
| **Linux daemon preflight** | `generate_systemd_unit` before sandbox / unit write; whitespace fails with **no** unit file |
| **Codex probe-before-mutate** | Preserved; timeout 5s |
| **Codex uninstall path** | Reads installed MCP `command` before remove — **better** than re-resolving; justified scope |
| **`error_kind` arm** | Required for exhaustive match; install rarely hits MCP |
| **Claude vs Grok hooks** | Separate paths; uninstall isolation OK |
| **Runtime broker self-spawn** | Untouched |
| **No Homebrew / managed installer state** | Clean |

---

## File classification

| File | Class |
|---|---|
| `cli/executable.rs` | **Essential** (untracked — must be `git add`ed) |
| `assets/famp-await.sh`, `hook-runner.sh` | **Essential** |
| `install/{claude_code,codex,grok,await_hook,hook_runner}.rs` | **Essential** |
| `daemon/install.rs` | **Essential** |
| `cli/mod.rs`, `cli/error.rs` | **Essential** |
| `uninstall/codex.rs` | **Justified supporting** (old `resolve_famp_bin` removed) |
| `mcp/error_kind.rs` | **Justified supporting** (exhaustive match) |
| `INSTALLATION_FOUNDATION_REPORT.md` | **Out of scope** unless intentionally published |
| `docs/CONFIGURATION.md` | **Missing essential doc update** |

Scope/size (~671 insertions with resolver) is **proportionate**. Coherent single PR; not a split candidate unless launchd reload policy becomes a large redesign.

---

## Review methodology notes

Inspected:

- Full `git diff` for all tracked changes  
- Untracked `crates/famp/src/cli/executable.rs` in full  
- Surrounding call sites for Claude, Codex, Grok, daemon install/load, uninstall codex  
- Assets `famp-await.sh` / `hook-runner.sh`  
- Repository search for remaining `~/.cargo/bin/famp`, `command -v famp`, `which::which("famp")`, `FAMP_INSTALL_FAMP_BIN`, hook assets, `current_exe`  
- Confirmed bus self-spawn path unchanged  

Live checks included:

- Bad / whitespace-only `FAMP_INSTALL_FAMP_BIN` → Claude install creates no files  
- Space-containing executable path correctly quoted into hooks  
- systemd unit generation rejects whitespace binary before unit write  
- Codex install with custom bin + uninstall cleans hooks/trust via MCP `command` field  
- PATH-less install of `target/debug/famp` uses `current_exe`  
- Relative path, symlink path, trailing-space filename overrides  

---

## Final verdict

# APPROVE WITH MINOR FIXES

### 1. Minimum required before commit

1. **`git add crates/famp/src/cli/executable.rs`** — without it the tree does not build after commit of `mod executable`.  
2. **Update `docs/CONFIGURATION.md`** for real precedence (no cargo fallback; fail-closed validation; applies to Claude/Grok/daemon, not Codex-only).  
3. **Decide on H2 (launchd reload):** either implement path-change reload, or document in the same doc/PR description that macOS reinstall updates the plist but does not rebootstrap an already-loaded job (and name the recovery command).  
4. **Do not commit** `INSTALLATION_FOUNDATION_REPORT.md` unless you want it.  
5. Prefer adding **at least one public-path no-mutation test** (M2) before merge if the PR claims that guarantee.

### 2. Optional follow-ups

- Tighten `file_stem` → exact `file_name` (M1).  
- Transparent error display (L2).  
- Dual Claude/Grok pin + uninstall matrix (M3).  
- Expand resolver test matrix (M6).  
- Normalize `//.` in absolute paths (L1).  
- Fix stale probe comment (L5).

### 3. Test gaps that should close

- Public entry no-mutation on resolution failure (all installers + daemon).  
- Override with significant leading/trailing path spaces.  
- Newline path → rendered hook `bash -n` + `"$FAMP_BIN"` invocation.  
- `current_dir` failure.  
- `FampExecutable` in `mcp_error_kind_exhaustive` fixtures.  
- Claude+Grok different pins / uninstall isolation.  
- (Documented) launchd already-loaded path repair behavior.

### 4. Coherent first PR?

**Yes.** One abstraction, consistent wire-through, assets rendered, cargo fallback removed, fail-before-mutate for resolution, systemd pure preflight improved.

### 5. Safe to commit after required changes?

**Yes**, after adding `executable.rs`, fixing docs (+ launchd note or fix), and excluding the scratch report. Optional tests strongly recommended but live validation already supports the main guarantees.

### 6. Commands run and results

| Command | Result |
|---|---|
| `git remote -v` / `status` / `diff --stat` / `diff --check` | Repo OK; uncommitted; check clean |
| Inspect full diffs + `executable.rs` + surrounding install/daemon/uninstall | Reviewed |
| Repo-wide search for cargo/`which`/hooks/`FAMP_INSTALL` | Install production paths cleaned; docs/tests remain |
| `cargo fmt --all -- --check` | **pass** |
| `cargo clippy -p famp --all-targets -- -D warnings` | **pass** |
| `cargo test -p famp --lib cli::executable::tests` | **6 passed** |
| `cli::install::claude_code::tests` | **7 passed** |
| `cli::install::codex::tests` | **7 passed** |
| `cli::install::grok::tests` | **7 passed** |
| `cli::daemon::install::tests` | **5 passed** |
| `cargo test -p famp --test mcp_error_kind_exhaustive` | **5 passed** (fixture list still omits new variant) |
| `/bin/sh` quoting round-trip (spaces, quotes, `$`, `` ` ``, `\`, NL, CR, globs, Unicode) | **all OK** |
| `bash -n` on both assets | **OK** |
| `shellcheck` | **not installed** |
| Live: bad/empty override → Claude | **no files created** |
| Live: space path → hooks | **correct `FAMP_BIN='…'`** |
| Live: systemd whitespace bin → daemon | **error, no unit file** |
| Live: Codex install custom bin + uninstall | **hooks/trust cleaned via MCP command field** |
| Live: PATH-less install of `target/debug/famp` | **uses `current_exe`** |

### 7. Review-time tree (unchanged by review)

At review completion the product working tree was still:

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
 M crates/famp/src/cli/uninstall/codex.rs
?? INSTALLATION_FOUNDATION_REPORT.md
?? crates/famp/src/cli/executable.rs
```

No product source files were modified by the review process itself. This review document is a new write-only artifact.
