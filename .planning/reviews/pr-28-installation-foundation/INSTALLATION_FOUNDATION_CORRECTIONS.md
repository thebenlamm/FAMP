# Installation-Foundation Correction Pass

Response to the adversarial review (`INSTALLATION_FOUNDATION_CODE_REVIEW.md`,
verdict **APPROVE WITH MINOR FIXES**) on the uncommitted installation-foundation
implementation in `thebenlamm/FAMP`.

No branch was created or switched, nothing was committed, pushed, staged, or
opened as a PR. This document is itself a scratch artifact — see §12.

---

## 1. Corrections implemented

| # | Item | Status |
|---|---|---|
| 1 | Executable-resolution docs rewritten in `docs/CONFIGURATION.md` | Done |
| 2 | Exact-filename matching for the current executable (pure helper + tests) | Done |
| 3 | Public-path no-mutation tests (Claude, Codex, Grok, daemon) | Done |
| 4 | `CliError::FampExecutable` added to the exhaustive MCP kind fixtures | Done |
| 5 | Stale Codex probe comment fixed; error display de-duplicated | Done |
| 6 | Hook-template render tests (hostile paths, token, no-fallback asserts) | Done |
| 7 | Claude/Grok coexistence + uninstall-isolation tests (both directions) | Done |
| 8 | launchd repair semantics — **narrower documented outcome + advisory** | Done |
| 9 | `INSTALLATION_FOUNDATION_REPORT.md` excluded (still untracked, untouched) | Confirmed |
| 10 | `crates/famp/src/cli/executable.rs` identified as a required new file | See §10 |

Plus one thing the review missed, found while running the full suite: **the base
implementation broke 15 existing tests and made 12 more host-dependent.**
Details in §8 and §13.

---

## 2. Files modified or added during this pass

**New (untracked, must be committed):**

- `crates/famp/src/cli/executable.rs` — the resolver (pre-existing from the base
  work; extended here)
- `crates/famp/tests/claude_grok_hook_coexistence.rs` — new test file

**Modified this pass:**

- `crates/famp/src/cli/error.rs`
- `crates/famp/src/cli/daemon/install.rs`
- `crates/famp/src/cli/install/await_hook.rs`
- `crates/famp/src/cli/install/claude_code.rs`
- `crates/famp/src/cli/install/codex.rs`
- `crates/famp/src/cli/install/grok.rs`
- `crates/famp/src/cli/uninstall/claude_code.rs`
- `crates/famp/tests/mcp_error_kind_exhaustive.rs`
- `crates/famp/tests/install_claude_code.rs`
- `crates/famp/tests/install_grok.rs`
- `crates/famp/tests/install_uninstall_roundtrip.rs`
- `crates/famp/tests/grok_install_uninstall_roundtrip.rs`
- `crates/famp/tests/codex_install_uninstall_roundtrip.rs`
- `crates/famp/tests/hook_runner_await.rs`
- `crates/famp/tests/hook_runner_dispatch.rs`
- `crates/famp/tests/hook_runner_path_parity.rs`
- `crates/famp/tests/hook_runner_failure_modes.rs`
- `docs/CONFIGURATION.md`

**Untouched from the base work:** `crates/famp/assets/*.sh`,
`crates/famp/src/cli/install/hook_runner.rs`,
`crates/famp/src/cli/mcp/error_kind.rs`, `crates/famp/src/cli/mod.rs`,
`crates/famp/src/cli/uninstall/codex.rs`.

---

## 3. Resolution of every HIGH and MEDIUM finding

### H1 — stale cargo-fallback docs

New section “Resolution precedence for the installed `famp` binary” documents
the 4-step precedence, states there is no `~/.cargo/bin/famp` fallback, that
validation precedes mutation, that an explicitly-set-but-invalid override fails
closed, that Cargo installs remain supported (as a *result*, never a default),
and lists every artifact carrying the path. The env-flag table row no longer
says “Codex Stop hook”; the cargo path in the MCP example is explicitly labelled
an example.

### H2 — macOS plist rewritten but job not reloaded

Narrower outcome — see §4.

### M1 — `file_stem() == "famp"`

Now `file_name()` against a pure helper
`is_famp_executable_name_for(name, NameConvention)`: Unix = exact `famp`;
Windows = exact `famp.exe`, ASCII-case-insensitive. `resolve_with` uses the host
convention (`HOST_NAME_CONVENTION`). Both conventions are testable on any host,
so `famp.exe` acceptance is proved on Linux.

### M2 — negative coverage bypassed the resolver

Four new tests call the real public entry points (`claude_code::run_at`,
`codex::run_at_project`, `grok::run_at`, `daemon::install::run_at`) under
`temp_env::with_var("FAMP_INSTALL_FAMP_BIN", …)` with a missing path, a
whitespace-only value, and a directory. Each snapshots the whole target tree
(files + directories + bytes) and asserts it is unchanged; the daemon test
additionally asserts `~/.famp/` was never created. The dependency-injected unit
tests are untouched.

### M3 — shared source, separate destinations, untested

New `claude_grok_hook_coexistence.rs`: install Claude with A and Grok with B,
assert each installed hook pins only its own binary, uninstall Claude → Grok's
hook byte-identical and still pinned to B; reverse direction likewise; plus a
reinstall test showing repinning Claude leaves Grok's hook alone.

### M4 — Codex probe executes the resolved binary

Documented as intentional in the probe doc comment: the override is trusted
exactly as much as the installer invocation itself.

### M5 — missing exhaustive fixture

`CliError::FampExecutable(FampExecutableError::NotFound)` added to
`variants_c()` (so it participates in `every_variant_has_mcp_kind` and
`mcp_kinds_are_unique`) plus a spot check pinning the kind string
`famp_executable_error`.

### M6 — thin resolver matrix

6 → 12 tests: exact-name matching, `famp.bak` current-exe rejection falling
through to PATH, leading/trailing-space filenames, non-UTF-8 env value
(relative and absolute), `current_dir` failure on a relative explicit path (via
the existing injected locator, one added `cwd_fails` flag), and Cargo-shaped-path
preservation across all three sources plus its non-invention.

### LOW findings also addressed

- **L2** double print — `CliError::FampExecutable` is now `#[error(transparent)]`;
  `Metadata`/`Absolute` no longer embed `{error}` in Display (it is the
  `#[source]`).
- **L3** `utf8()` — `FampExecutable` stores the validated `String`; no
  `unwrap_or_default`.
- **L5** stale probe comment — rewritten.
- **L6** uninstall cargo note — generalized.

**Not addressed:** L1 (`./` noise in lexically-absolute relative paths —
cosmetic), L4 (`DaemonError::FampExecutable(String)` kept; see §13), L7/L8 (no
action needed).

---

## 4. Chosen launchd outcome and rationale

**Acceptable narrower outcome — documented, not implemented.** Rationale from
inspecting `daemon/{install,restart,uninstall,status}.rs`:

- `load_macos` deliberately no-ops when the label is registered: reloading is not
  free. `restart.rs` documents that a reload “drops all in-memory registrations
  and parked `famp await` waiters”. Making an *idempotent install* silently kill
  every listen-mode agent is a lifecycle change, not a small fix.
- A correct reload is already implemented and tested elsewhere:
  `famp daemon restart` does `bootout` + `bootstrap` + `kickstart` (full bootout
  is required to refresh launchd's Lightweight Code Requirement after a cdhash
  change, issue #20) and then waits for a Hello handshake.
- macOS code cannot be compiled or run on this Linux host (only
  `x86_64-unknown-linux-gnu` is installed), so implementing bootout/bootstrap
  here would ship untestable lifecycle code.

What was added instead (both permitted by the narrower outcome):

- **Pure, cross-platform-tested decision logic:**
  `service_file_outcome(existing, generated) -> Created | Unchanged | Updated`
  and `needs_reload_advisory(outcome, already_loaded)`, with two tests —
  including one proving that repointing the `famp` binary yields `Updated`. No
  `launchctl` execution in the tests.
- **Operator-visible advisory:** macOS `daemon install` prints a note *only* when
  the plist content changed under an already-loaded job, naming the verified
  recovery command `famp daemon restart` and explaining why install does not do
  it automatically. Idempotency is preserved when content is unchanged.
- **Docs:** `docs/CONFIGURATION.md` gained “Reinstalling with a different `famp`
  binary (macOS)” stating exactly that the plist is always updated; that an
  already-loaded job keeps its previous `ProgramArguments`; that
  `famp daemon restart` is the reload (fallback: `famp daemon uninstall` then
  `famp daemon install`). Linux is described accurately as milder but not exempt.

---

## 5. Documentation changes

`docs/CONFIGURATION.md` (+105 lines):

- New section **“Resolution precedence for the installed `famp` binary”** —
  precedence block, six behavioural rules, artifact table (Claude / Codex / Grok
  / daemon), and repin/rerun guidance.
- Corrected `FAMP_INSTALL_FAMP_BIN` row in the CLI-flags table: applies to
  **every** installer, not Codex only; an invalid value fails the install rather
  than falling back.
- New `FAMP_INSTALL_FAMP_BIN` row in the Environment Variables table.
- The cargo path in the Claude MCP example is re-labelled as an example, not a
  default.
- New **“Reinstalling with a different `famp` binary (macOS)”** subsection under
  Daemon Service Files.

---

## 6. New tests and what each proves

### Resolver (`cli::executable::tests`, 6 new → 12 total)

| Test | Proves |
|---|---|
| `only_exact_platform_file_names_identify_the_famp_executable` | `famp` accepted (Unix); `famp.exe` / `FAMP.EXE` accepted (Windows semantics); `famp.bak`, `famp.1`, `famp.exe.bak`, `famp-old`, `cargo-famp`, space-padded names rejected under both; the real running test binary rejected |
| `current_exe_with_extension_is_not_a_candidate` | A `famp.bak` current-exe falls through to PATH, and hard-fails when PATH is empty |
| `leading_and_trailing_space_file_names_are_preserved` | Leading/trailing spaces are path characters, not trimmed |
| `non_utf8_explicit_env_value_fails_without_fallback` | Relative and absolute non-UTF-8 overrides fail even with a valid PATH candidate available |
| `current_dir_failure_on_relative_explicit_path_is_fatal` | `Absolute` error, no PATH fallback; absolute paths never consult cwd |
| `cargo_shaped_path_is_preserved_when_actually_selected` | A cargo path works via all three sources and is never invented |

### Public-path no-mutation (one per installer)

`public_run_at_fails_before_any_mutation_when_executable_is_unresolvable` in
`claude_code`, `grok`, `daemon::install`, and
`public_run_at_project_fails_before_any_mutation_when_executable_is_unresolvable`
in `codex`. Each proves: expected resolver error; byte-identical pre-existing
files; no new file, directory, backup, hook asset or service file. Codex
additionally proves the legacy `famp-await.sh` shim is not pruned and that
`hooks.state` trust plus `mcp_servers.famp.command` are untouched. Daemon
additionally proves `~/.famp/` is not created.

### Hook templates (`cli::install::await_hook::tests`, 3 new, covering both assets)

| Test | Proves |
|---|---|
| `token_appears_exactly_once_in_each_template` | Exactly one `@FAMP_BIN@` per source template, and it is the `FAMP_BIN` assignment |
| `hostile_executable_paths_render_safely_into_both_hooks` | 14 real staged files (spaces, apostrophe, `$`/`${}`, backticks, `$(…)`, `;`, `&&`, `\|`, backslashes, globs, double quotes, tab, Unicode `ünïcode-日本語-🚀`, and a **newline** on Unix). For each × each template: rendered file passes `bash -n`; `$FAMP_BIN` round-trips byte-for-byte when evaluated and quoted; a canary file proves no second command ran |
| `rendered_hooks_never_fall_back_to_path_or_cargo` | Source *and* rendered contain no `command -v famp`, no `.cargo/bin/famp`, no bare `famp send` / `famp await` (regex self-checked against known-bad and known-good samples), and no surviving token |

### Daemon reload decision (2 new)

- `changed_plist_under_a_loaded_job_advises_an_explicit_reload`
- `repointing_the_famp_binary_is_a_plist_change`

### Coexistence (3 new integration tests)

- `uninstall_claude_leaves_grok_hook_pinned_to_its_own_executable`
- `uninstall_grok_leaves_claude_hook_pinned_to_its_own_executable`
- `reinstalling_one_integration_repins_only_its_own_hook`

### MCP kinds

`FampExecutable` fixture in `variants_c()` + `famp_executable_error` spot check.

---

## 7. Exact commands run (principal)

```bash
git remote -v; git status --short --branch; git diff --check

cargo fmt --all; cargo fmt --all -- --check
cargo check -p famp --all-targets
cargo clippy -p famp --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings

cargo test -p famp --lib cli::executable::tests
cargo test -p famp --lib cli::install::claude_code::tests
cargo test -p famp --lib cli::install::codex::tests
cargo test -p famp --lib cli::install::grok::tests
cargo test -p famp --lib cli::install::await_hook::tests
cargo test -p famp --lib cli::install::hook_runner::tests
cargo test -p famp --lib cli::daemon::install::tests
cargo test -p famp --test mcp_error_kind_exhaustive
cargo test -p famp --test claude_grok_hook_coexistence

cargo test -p famp --test install_claude_code --test install_grok --test install_codex \
  --test install_uninstall_roundtrip --test grok_install_uninstall_roundtrip \
  --test codex_install_uninstall_roundtrip
cargo test -p famp --test hook_runner_await --test hook_runner_dispatch \
  --test hook_runner_path_parity --test hook_runner_failure_modes

cargo test --workspace                                            # x3
PATH=<shim-without-famp>:/usr/bin:/bin cargo test --workspace     # CI-parity hermeticity

bash -n crates/famp/assets/hook-runner.sh
bash -n crates/famp/assets/famp-await.sh
git diff --check

# live checks against the debug binary
FAMP_INSTALL_FAMP_BIN=/definitely/not/here/famp ./target/debug/famp install-claude-code --home $TMP
FAMP_INSTALL_FAMP_BIN=/definitely/not/here/famp ./target/debug/famp daemon install --home $TMP
```

---

## 8. Test, lint, and shell-validation results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | **pass** |
| `cargo check -p famp --all-targets` | **pass** |
| `cargo clippy -p famp --all-targets -- -D warnings` | **pass** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **pass** |
| `cli::executable::tests` | **12 passed** |
| `cli::install::claude_code::tests` | **8 passed** |
| `cli::install::codex::tests` | **8 passed** |
| `cli::install::grok::tests` | **8 passed** |
| `cli::install::await_hook::tests` | **19 passed** |
| `cli::install::hook_runner::tests` | **7 passed** |
| `cli::daemon::install::tests` | **8 passed** |
| `--test mcp_error_kind_exhaustive` | **5 passed** |
| `--test claude_grok_hook_coexistence` | **3 passed** |
| **Full workspace suite** | **1063 passed, 0 failed, 3 ignored** (162 test binaries, exit 0) |
| **Full workspace suite, `famp` absent from `PATH`** | **same, exit 0** |
| `bash -n` on both assets | **OK** |
| `git diff --check` | **clean** |
| `shellcheck` / `just check-shellcheck` | **NOT AVAILABLE — shellcheck is not installed on this host.** `bash -n` is a syntax check only and is **not** equivalent to shellcheck; that CI gate remains unverified here |

### Failures caused by the base change, now fixed

Running the *suite* (not just unit tests) surfaced 15 pre-existing tests failing
because they executed the raw asset and hit the literal token
(`line 583: @FAMP_BIN@: command not found`):

- `hook_runner_await` — 11 failures
- `hook_runner_dispatch` — 2 failures
- `hook_runner_path_parity` — 2 failures

They now render the template exactly as `install_shim` does, pinned at each
test's mock `famp`.

Separately, 12 install/roundtrip tests silently depended on `famp` being on the
developer's `PATH` — verified failing with `FampExecutable(NotFound)` under a
PATH without famp, which is the CI condition (`cargo nextest run --workspace`
with no prior install). They now pin `FAMP_INSTALL_FAMP_BIN` via `temp_env`,
following the existing `install_codex.rs` convention.

Three `hook_runner_await` tests that previously *skipped* unless
`~/.claude/hooks/famp-await.sh` existed now run against the rendered asset. With
pinning in place they would otherwise have executed the user's real `famp`
against the live broker.

### One flake observed

In the first full-workspace run,
`cli::install::codex::tests::install_codex_writes_mcp_and_stop_hook` failed once
(the probe of the staged stub reported “unsupported”) — consistent with a
transient `ETXTBSY` when another test thread forks while the stub is being
written. Not reproduced in 3 isolated + 3 full-suite runs since. Mitigated
test-side by warming up the stub until it actually executes before handing it to
the installer.

No environment-blocked socket tests failed.

---

## 9. Remaining repository-search matches

| Pattern | Remaining matches |
|---|---|
| `~/.cargo/bin/famp`, `.cargo/bin/famp` | Resolver tests (deliberate: cargo-shaped path as a normal selection), `json_merge`/`toml_merge`/daemon fixtures, `restart.rs` cmdline tests, negative asserts (`!contains(".cargo/bin/famp")`), `docs/CONFIGURATION.md` (labelled example + explicit “no fallback” statement), `docs/DEVELOPMENT.md` (accurate: `just install` → `cargo install`), `docs/GATEWAY-SETUP.md`, historical plan/spec docs. **No production fallback remains** |
| `command -v famp` | Negative assertions in tests, and one historical plan document (`docs/superpowers/plans/2026-05-05-listen-mode.md`) recording the old design |
| `which::which("famp")` | One site: the resolver's `ProcessLocator` (precedence step 3) |
| `current_exe()` | Resolver + its tests; `await_cmd`, `listen_wake`, `bus_client/spawn` (runtime self-spawn, correctly out of scope); test helpers |
| `famp send`, `famp await` | Zero in either shipped asset (both use `"$FAMP_BIN"`); remaining hits are prose/docs |
| `FAMP_INSTALL_FAMP_BIN` | Resolver, docs, tests |
| `probe is skipped`, `binary is missing` | **Zero matches** — the comment was rewritten |

---

## 10. `git diff --stat`, including the untracked resolver file

```
 crates/famp/assets/famp-await.sh                   |  21 +-
 crates/famp/assets/hook-runner.sh                  |  15 +-
 crates/famp/src/cli/daemon/install.rs              | 299 ++++++++++++++++++---
 crates/famp/src/cli/error.rs                       |   8 +
 crates/famp/src/cli/install/await_hook.rs          | 211 ++++++++++++++-
 crates/famp/src/cli/install/claude_code.rs         | 174 +++++++++---
 crates/famp/src/cli/install/codex.rs               | 236 ++++++++++------
 crates/famp/src/cli/install/grok.rs                | 152 ++++++++---
 crates/famp/src/cli/install/hook_runner.rs         |  19 +-
 crates/famp/src/cli/mcp/error_kind.rs              |  17 +-
 crates/famp/src/cli/mod.rs                         |   1 +
 crates/famp/src/cli/uninstall/claude_code.rs       |   4 +-
 crates/famp/src/cli/uninstall/codex.rs             |  13 +-
 crates/famp/tests/codex_install_uninstall_roundtrip.rs |  25 +-
 crates/famp/tests/grok_install_uninstall_roundtrip.rs  |  19 +-
 crates/famp/tests/hook_runner_await.rs             | 136 ++++++----
 crates/famp/tests/hook_runner_dispatch.rs          |  36 ++-
 crates/famp/tests/hook_runner_failure_modes.rs     |  19 +-
 crates/famp/tests/hook_runner_path_parity.rs       |  18 +-
 crates/famp/tests/install_claude_code.rs           |  26 +-
 crates/famp/tests/install_grok.rs                  |  24 +-
 crates/famp/tests/install_uninstall_roundtrip.rs   |  24 +-
 crates/famp/tests/mcp_error_kind_exhaustive.rs     |  12 +
 docs/CONFIGURATION.md                              | 105 +++++++-
 24 files changed, 1303 insertions(+), 311 deletions(-)
```

`git diff --stat` cannot show untracked files. **Both of these are required in
the eventual commit set:**

```
crates/famp/src/cli/executable.rs                  650 lines   REQUIRED NEW FILE
                                                               (`mod executable;` does
                                                               not build without it)
crates/famp/tests/claude_grok_hook_coexistence.rs  168 lines   new test file
```

---

## 11. `git status --short`

```
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
?? INSTALLATION_FOUNDATION_REPORT.md
?? crates/famp/src/cli/executable.rs
?? crates/famp/tests/claude_grok_hook_coexistence.rs
```

Nothing staged, no branch change, no commit, no push, no PR.

---

## 12. Scratch-artifact exclusion

`INSTALLATION_FOUNDATION_REPORT.md` remains **untracked and unmodified**
(6506 bytes) — excluded from the product change.

`INSTALLATION_FOUNDATION_CODE_REVIEW.md` (the review) and this file
(`INSTALLATION_FOUNDATION_CORRECTIONS.md`) are likewise untracked scratch
artifacts and should also be excluded unless you want them published.

---

## 13. Remaining risks before commit

1. **macOS code is compiled only on macOS.** The ~8 new lines inside
   `#[cfg(target_os = "macos")]` (registration probe, outcome classification,
   advisory `writeln!`) cannot be type-checked here — only
   `x86_64-unknown-linux-gnu` is installed. The decision logic they call is pure
   and tested cross-platform, and the glue mirrors existing patterns, but a
   macOS `cargo check` before merge would close this.
2. **`shellcheck` unavailable**, so the `just check-shellcheck` CI gate on both
   assets is unverified. `FAMP_BIN=@FAMP_BIN@` is an ordinary unquoted
   assignment word and no rule obviously applies — but that is reasoning, not a
   run.
3. **`CliError::Daemon` still double-prints** (`"daemon error: {0}"` + the main
   binary's source walk) for *every* daemon error. Pre-existing and documented
   as intentional in `error.rs`; left alone. `DaemonError::FampExecutable` was
   not converted to a typed variant (L4) — instead the resolver's source chain
   is flattened into the string so no cause is lost.
4. **Codex probe ETXTBSY flake** (§8) is mitigated, not eliminated — the residual
   window is a test-harness race, not production behaviour.
5. **`posix_shell_literal` is now `pub`** so integration tests render assets
   exactly as production does. A small, deliberate public-API addition to
   `famp::cli::executable`.
6. **Install tests now depend on `assert_cmd::cargo::cargo_bin("famp")` or
   staged stubs** rather than the host `PATH` — the same convention
   `install_codex.rs` already used. The whole suite passes with `famp` absent
   from `PATH`.
