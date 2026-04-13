---
phase: 00-toolchain-workspace-scaffold
plan: 03
type: execute
wave: 3
depends_on: ["00-02"]
files_modified:
  - Justfile
  - .config/nextest.toml
  - .github/workflows/ci.yml
autonomous: true
requirements: [TOOL-03, TOOL-04, TOOL-05]
must_haves:
  truths:
    - "`just build`, `just test`, `just lint`, `just fmt-check`, `just ci` all exit 0 on the empty workspace"
    - "`cargo nextest run --workspace` exits 0 using the `default` profile"
    - "GitHub Actions workflow parses and runs fmt-check, clippy, build, nextest, doc-test, audit jobs"
    - "Local `just ci` mirrors the CI job set exactly (CI-parity rule)"
    - "A green `just ci` implies a green CI run on push"
  artifacts:
    - path: Justfile
      provides: "Task runner with build/test/lint/fmt/ci targets"
      contains: "cargo nextest run"
    - path: .config/nextest.toml
      provides: "Nextest profiles: default (local) and ci (fail-fast false)"
      contains: "[profile.ci]"
    - path: .github/workflows/ci.yml
      provides: "GitHub Actions workflow with 6 jobs and Swatinem cache"
      contains: "Swatinem/rust-cache"
  key_links:
    - from: Justfile
      to: .github/workflows/ci.yml
      via: "identical command surface in both"
      pattern: "cargo (fmt|clippy|build|nextest)"
    - from: .github/workflows/ci.yml
      to: Cargo.toml
      via: "cargo commands invoke workspace"
      pattern: "--workspace"
---

<objective>
Wire the CI-parity gate: a `Justfile` with build/test/lint/fmt/ci targets that maps 1:1 to a GitHub Actions workflow running fmt-check, clippy, build, nextest, doc-test, and audit on every push. Nextest is installed as the default test runner via a dedicated config file. A green `just ci` locally MUST imply a green CI run.

Purpose: Closes TOOL-03 (just), TOOL-04 (nextest), TOOL-05 (CI). Establishes the edit-build-test loop developer experience for Phase 1+.
Output: Justfile, .config/nextest.toml, .github/workflows/ci.yml
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@CLAUDE.md
@.planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md
@Cargo.toml
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write Justfile + nextest config</name>
  <read_first>
    - CLAUDE.md (§`just` — task runner; §Testing — nextest)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-11, D-12, D-13)
  </read_first>
  <files>Justfile, .config/nextest.toml</files>
  <action>
Create `Justfile` at repo root with EXACT content (per D-11, targets map 1:1 to CI jobs):

```make
# FAMP — task runner
# Run `just` with no args to see available recipes.

default:
    @just --list

# Build the entire workspace with all targets
build:
    cargo build --workspace --all-targets

# Run all tests via cargo-nextest (unit + integration)
test:
    cargo nextest run --workspace

# Run doc tests (nextest does not run doctests)
test-doc:
    cargo test --workspace --doc

# Run clippy with workspace-strict settings and deny warnings
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format all sources
fmt:
    cargo fmt --all

# Check formatting without modifying (CI gate)
fmt-check:
    cargo fmt --all -- --check

# Run `cargo audit` for RustSec advisories
audit:
    cargo audit

# Full local CI-parity gate. A green `just ci` implies a green GitHub Actions run.
ci: fmt-check lint build test test-doc
    @echo "✓ local CI-parity checks passed"

# Clean build artifacts
clean:
    cargo clean
```

Create `.config/nextest.toml` with EXACT content (per D-13):

```toml
[profile.default]
# Local developer profile: fail fast so feedback is quick
fail-fast = true
slow-timeout = { period = "60s", terminate-after = 2 }

[profile.ci]
# CI profile: surface every failure in one run
fail-fast = false
slow-timeout = { period = "120s", terminate-after = 3 }
failure-output = "immediate-final"
```

Do NOT install `just` or `cargo-nextest` in this task — they are developer tools installed once via `cargo install` per README, and installed in CI via `taiki-e/install-action` in Task 2.

Verify locally:
- `just --list` prints all recipes
- `just fmt-check` exits 0 (files from Plan 02 were written with rustfmt-compatible formatting)
- `just lint` exits 0
- `just build` exits 0
- `just test` exits 0 (13 smoke tests pass)
- `just ci` exits 0
  </action>
  <verify>
    <automated>test -f Justfile && test -f .config/nextest.toml && grep -q "cargo nextest run --workspace" Justfile && grep -q "ci: fmt-check lint build test test-doc" Justfile && grep -q "\[profile.ci\]" .config/nextest.toml && just --list && just ci</automated>
  </verify>
  <acceptance_criteria>
    - `Justfile` exists at repo root
    - Contains recipes: `build`, `test`, `test-doc`, `lint`, `fmt`, `fmt-check`, `audit`, `ci`, `clean` (grep each recipe name)
    - `ci` recipe depends on `fmt-check lint build test test-doc` (literal substring match)
    - `test` recipe invokes `cargo nextest run --workspace`
    - `lint` recipe invokes `cargo clippy --workspace --all-targets -- -D warnings`
    - `.config/nextest.toml` exists
    - Contains `[profile.default]` and `[profile.ci]` sections
    - `[profile.ci]` contains `fail-fast = false`
    - `just --list` command executes and exits 0
    - `just ci` command executes and exits 0 (full local CI-parity check)
  </acceptance_criteria>
  <done>Justfile + nextest config committed; `just ci` is the single pre-push gate and passes on the empty workspace.</done>
</task>

<task type="auto">
  <name>Task 2: Write GitHub Actions CI workflow</name>
  <read_first>
    - CLAUDE.md (§CI — GitHub Actions; action versions)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-18 through D-22)
    - Justfile (written in Task 1 — mirror command surface)
  </read_first>
  <files>.github/workflows/ci.yml</files>
  <action>
Create `.github/workflows/ci.yml` with EXACT content (per D-18 through D-22):

```yaml
name: ci

on:
  push:
    branches: ["**"]
  pull_request:
    branches: ["**"]
  schedule:
    # Daily advisory scan
    - cron: "0 7 * * *"

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  fmt-check:
    name: fmt-check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check

  clippy:
    name: clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets -- -D warnings

  build:
    name: build (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --workspace --all-targets

  test:
    name: test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest
      - run: cargo nextest run --workspace --profile ci

  doc-test:
    name: doc-test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace --doc

  audit:
    name: audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Validation rules** (D-19, D-20, D-22):
- Six distinct jobs: `fmt-check`, `clippy`, `build`, `test`, `doc-test`, `audit`
- `build` and `test` run on matrix `[ubuntu-latest, macos-latest]` (Windows deferred per D-18)
- Uses `Swatinem/rust-cache@v2` in every cargo-using job (D-20)
- Uses `taiki-e/install-action@v2` for cargo-nextest in the `test` job (D-20 — avoids compiling nextest from source every run)
- `concurrency` block cancels superseded runs (D-22)
- Uses `dtolnay/rust-toolchain@stable` which respects `rust-toolchain.toml` (D-02)
- Triggers: `push` + `pull_request` on any branch + daily cron for audit (D-18, D-20)
- `test` job uses `--profile ci` to invoke the nextest CI profile written in Task 1
- `RUSTFLAGS: "-D warnings"` env guards against any warning slipping through

**Validate the workflow file parses as YAML** (no remote execution):

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"
```

OR if `python3`/`yaml` unavailable:

```bash
ruby -ryaml -e "YAML.load_file('.github/workflows/ci.yml')"
```

The workflow CANNOT be smoke-tested locally (needs a push to GitHub); validation is YAML-parse + structural grep checks. The first real run happens when the repo is pushed.
  </action>
  <verify>
    <automated>test -f .github/workflows/ci.yml && python3 -c "import yaml; wf = yaml.safe_load(open('.github/workflows/ci.yml')); jobs = wf['jobs']; assert set(jobs.keys()) == {'fmt-check','clippy','build','test','doc-test','audit'}, f'jobs mismatch: {list(jobs.keys())}'; assert 'Swatinem/rust-cache@v2' in open('.github/workflows/ci.yml').read(); assert 'taiki-e/install-action@v2' in open('.github/workflows/ci.yml').read(); assert 'cancel-in-progress: true' in open('.github/workflows/ci.yml').read(); print('ci.yml structure OK')"</automated>
  </verify>
  <acceptance_criteria>
    - `.github/workflows/ci.yml` exists
    - Parses as valid YAML (python3 yaml.safe_load succeeds)
    - Contains exactly six jobs named: `fmt-check`, `clippy`, `build`, `test`, `doc-test`, `audit` (set equality)
    - Contains literal string `Swatinem/rust-cache@v2`
    - Contains literal string `taiki-e/install-action@v2`
    - Contains literal string `dtolnay/rust-toolchain@stable`
    - Contains literal string `cargo nextest run --workspace --profile ci`
    - Contains literal string `cargo fmt --all -- --check`
    - Contains literal string `cargo clippy --workspace --all-targets -- -D warnings`
    - Contains literal string `cargo build --workspace --all-targets`
    - Contains literal string `rustsec/audit-check@v2`
    - Contains `cancel-in-progress: true`
    - `build` and `test` matrix contains both `ubuntu-latest` and `macos-latest`
    - Triggers: `push`, `pull_request`, and `schedule` (cron) all present
  </acceptance_criteria>
  <done>CI workflow committed. First push to GitHub will exercise the full green loop. Branch protection setup is a manual post-merge step documented in README (D-21).</done>
</task>

</tasks>

<verification>
```bash
# Local gate
just ci

# Workflow structural validation
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"

# File presence
test -f Justfile && test -f .config/nextest.toml && test -f .github/workflows/ci.yml
```
</verification>

<success_criteria>
- `just` targets all green on empty workspace (TOOL-03)
- `cargo nextest run` is the default test runner via `.config/nextest.toml` (TOOL-04)
- GitHub Actions workflow runs fmt, clippy, build, nextest, doc-test, audit on every push (TOOL-05)
- Justfile and workflow command surfaces mirror each other (CI-parity learned rule)
- `just ci` is the single pre-push gate
</success_criteria>

<output>
After completion, create `.planning/phases/00-toolchain-workspace-scaffold/0-03-SUMMARY.md`
</output>
