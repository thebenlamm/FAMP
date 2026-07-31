# Installation Foundation Implementation Report

Implemented the installation-foundation change without creating branches, committing, pushing, or opening a PR. The working tree was initially clean.

## 1. Implementation summary

- Added a shared validated executable resolver with precedence:
  1. `FAMP_INSTALL_FAMP_BIN`
  2. running executable when stem is `famp`
  3. `which famp`
  4. actionable typed error
- Validates absolute lexical paths, regular files, Unix execute permission, UTF-8, broken links, and preserves symlinks.
- Removed Cargo fallback paths and installer-local resolution.
- Wired the resolved path into Claude, Codex, Grok, launchd, and systemd.
- Converted Claude/Grok shell assets to rendered templates using shared POSIX shell quoting.
- Ensured Codex resolution and capability probing happen before mutation.
- Made systemd generation a pure preflight before sandbox directory creation.
- Preserved runtime broker auto-spawn behavior.

## 2. Modified and added files

Added:

- `crates/famp/src/cli/executable.rs`

Modified:

- `crates/famp/assets/famp-await.sh`
- `crates/famp/assets/hook-runner.sh`
- `crates/famp/src/cli/daemon/install.rs`
- `crates/famp/src/cli/error.rs`
- `crates/famp/src/cli/install/await_hook.rs`
- `crates/famp/src/cli/install/claude_code.rs`
- `crates/famp/src/cli/install/codex.rs`
- `crates/famp/src/cli/install/grok.rs`
- `crates/famp/src/cli/install/hook_runner.rs`
- `crates/famp/src/cli/mcp/error_kind.rs`
- `crates/famp/src/cli/mod.rs`
- `crates/famp/src/cli/uninstall/codex.rs`

## 3. Deviations and rationale

- Retained the existing systemd whitespace rejection instead of adding systemd escaping. It now occurs during pure preflight before filesystem mutation.
- `just check-shellcheck` could not run because `just` and `shellcheck` are not installed. Both shell templates passed `bash -n`.
- The full workspace suite cannot complete in this sandbox because four existing Unix-domain socket tests receive `EPERM`.

## 4. Exact validation commands run

```bash
git remote -v
git status --short --branch
pwd

cargo check -p famp --all-targets
cargo fmt --all
cargo fmt --all -- --check
cargo clippy -p famp --all-targets -- -D warnings

cargo test -p famp --lib cli::executable::tests
cargo test -p famp --lib cli::install::claude_code::tests
cargo test -p famp --lib cli::install::codex::tests
cargo test -p famp --lib cli::install::grok::tests
cargo test -p famp --lib cli::daemon::install::tests

cargo test -p famp \
  --test install_claude_code \
  --test install_codex \
  --test install_grok \
  --test install_uninstall_roundtrip \
  --test codex_install_uninstall_roundtrip \
  --test grok_install_uninstall_roundtrip

cargo test --workspace

just check-shellcheck
bash -n crates/famp/assets/hook-runner.sh
bash -n crates/famp/assets/famp-await.sh

git diff --check
```

## 5. Test and lint results

- Formatting: passed.
- Clippy for affected crate/all targets: passed with `-D warnings`.
- Resolver tests: 6 passed.
- Claude focused tests: 7 passed.
- Codex focused tests: 7 passed.
- Grok focused tests: 7 passed.
- Daemon generator tests: 5 passed.
- Installer/roundtrip integration tests: 21 passed.
- Shell syntax: both templates passed `bash -n`.
- Full workspace:
  - FAMP library: 283 passed, 4 environment-blocked.
  - Failures are existing socket-bind tests receiving `Operation not permitted`:
    - `bus_client::spawn::tests::returns_ok_when_socket_already_accepting`
    - three `cli::broker::tests::test_bind_exclusive_*` tests
- `just check-shellcheck`: unavailable because `just` and `shellcheck` are missing.

## 6. Remaining search matches

Production resolution matches:

```text
crates/famp/src/cli/executable.rs: which::which("famp")
crates/famp/src/cli/executable.rs: current_exe()
```

These are the intended shared resolver.

```text
crates/famp/src/bus_client/spawn.rs: current_exe()
```

This is the explicitly out-of-scope immediate runtime broker auto-spawn path.

The only `command -v famp` matches are negative test assertions verifying rendered assets do not contain it. No Cargo fallback, bare hook invocation, or duplicated installer-local resolver remains in production code/assets.

## 7. Migration implications

Existing installed configurations remain unchanged. Rerunning `install-claude-code`, `install-codex`, `install-grok`, or `daemon install` updates FAMP-owned entries to the newly selected absolute executable. Cargo source installs remain supported when `.cargo/bin/famp` is selected through the normal resolver.

## 8. `git diff --stat`

Tracked diff plus the untracked new resolver file:

```text
13 files changed, 671 insertions(+), 184 deletions(-)
```

The standard tracked-only `git diff --stat` reports:

```text
12 files changed, 334 insertions(+), 184 deletions(-)
```

## 9. `git status --short`

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
?? crates/famp/src/cli/executable.rs
```

## 10. Risks or unresolved questions

- macOS launchd generation is unit-tested, but live `launchctl` execution was not possible on this Linux host.
- Live systemd installation was intentionally not invoked.
- Shellcheck remains unverified until `just`/`shellcheck` is available.
- The systemd whitespace policy remains rejection-by-design.

## 11. Highest-risk diff walkthrough

- `executable.rs`: security and correctness boundary—precedence, strict explicit override semantics, metadata validation, symlink preservation, and POSIX quoting.
- `install/codex.rs`: ordering-sensitive path—resolution and native-hook probe now precede all MCP, shim, hook, and trust mutation.
- `daemon/install.rs`: service-manager safety—validated executable is passed explicitly; systemd content and whitespace rejection are computed before the sandbox probe mutates directories.
- Shell templates: installed scripts now pin `FAMP_BIN` using apostrophe-safe POSIX quoting and invoke only `"$FAMP_BIN"`.
- `uninstall/codex.rs`: reads the previously configured MCP command for precise legacy hook/trust cleanup now that installer-local resolution has been removed.
