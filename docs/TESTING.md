<!-- generated-by: gsd-doc-writer -->
# FAMP Testing Guide

This document covers the test framework, how to run tests locally, how to write
new tests, known gotchas, and how tests map to CI jobs.

---

## Test Framework and Setup

FAMP uses **cargo-nextest** as the primary test runner for unit and integration
tests, and plain **cargo test** for doc tests (nextest does not run doctests).

### Key testing libraries (`[workspace.dependencies]`)

| Library | Version | Purpose |
|---|---|---|
| `cargo-nextest` | (external tool) | Parallel test runner; CI parity via `.config/nextest.toml` profiles |
| `proptest` | 1.11.0 | Property-based testing for FSM transitions, bus fanout, crypto primitives |
| `insta` | 1.47.2 | Snapshot assertions for structured output and install artifacts |
| `assert_cmd` | 2.0 | CLI end-to-end tests (famp integration tests only) |
| `temp-env` | 0.3 | Scoped environment-variable mutation in tests |

`stateright` 0.31.0 is declared as a workspace dependency but is deferred to
a later milestone — it is not used in any current test file.

### Prerequisites

Install cargo-nextest and `just` before running the test suite:

```bash
cargo install cargo-nextest --locked
cargo install just
```

No additional environment setup is required for unit and integration tests.
Integration tests that spawn broker subprocesses create their own temporary
sockets and home directories.

---

## Running Tests

### Full test suite

```bash
just test
# equivalent: cargo nextest run --workspace
```

Runs all unit and integration tests for every crate in the workspace using
the `default` nextest profile (fail-fast enabled, 60 s slow timeout).

### Doc tests (separate step — nextest does not run them)

```bash
just test-doc
# equivalent: cargo test --workspace --doc
```

### Conformance gates (run before committing crypto or canonical changes)

```bash
# famp-canonical RFC 8785 JCS conformance (sampled subset, per-PR gate)
just test-canonical-strict
# equivalent: cargo nextest run -p famp-canonical --no-fail-fast

# famp-crypto §7.1c worked example + RFC 8032 vectors + doc tests
just test-crypto
# equivalent: cargo nextest run -p famp-crypto && cargo test -p famp-crypto --doc

# famp-core wire-string fixtures + exhaustive-match gate + doc tests
just test-core
# equivalent: cargo nextest run -p famp-core && cargo test -p famp-core --doc
```

### Fast feedback — single crate

```bash
# famp-canonical only (fastest loop for canonicalization changes)
just test-canonical
# equivalent: cargo nextest run -p famp-canonical

# Any other crate
cargo nextest run -p famp-bus
cargo nextest run -p famp-fsm
cargo nextest run -p famp-envelope
```

### Known gotcha: `cargo nextest -p famp` hangs

Running `cargo nextest run -p famp` (the top-level umbrella crate) stalls
in the test-binary `--list` phase. Use plain `cargo test` instead when you
need to run a single test or test file from that crate:

```bash
# Run a single integration test file
cargo test --test inspect_broker

# Run the library unit tests only
cargo test -p famp --lib

# Run a specific test by name
cargo test --test mcp_bus_e2e mcp_bus_e2e
```

The `just test` recipe runs `cargo nextest run --workspace`, which is not
affected by this hang because nextest handles workspace discovery differently
from single-package invocations.

### 100 M float corpus (nightly / release tags only)

```bash
just test-canonical-full
# equivalent: cargo nextest run -p famp-canonical --features full-corpus --no-fail-fast
```

Requires the cyberphone es6testfile corpus to be present at
`crates/famp-canonical/tests/vectors/full_corpus/es6testfile100m.txt`.
The nightly CI workflow downloads and verifies the corpus automatically.

### Local CI-parity gate

```bash
just ci
```

Runs fmt-check, clippy, build, all conformance gates, the full test suite,
doc tests, spec lint, structural invariant checks, and a publish dry-run.
A green `just ci` implies a green GitHub Actions run.

---

## Writing New Tests

### File naming and placement

Each crate under `crates/` has a `tests/` directory for integration tests and
`#[cfg(test)]` modules inside source files for unit tests.

| Test kind | Location | Example |
|---|---|---|
| Unit test | Inside source file, `#[cfg(test)]` block | `crates/famp-fsm/src/state.rs` |
| Integration test | `crates/<crate>/tests/<name>.rs` | `crates/famp-bus/tests/prop01_dm_fanin_order.rs` |
| Property test | `crates/<crate>/tests/<name>.rs` using `proptest!` macro | `crates/famp-fsm/tests/proptest_matrix.rs` |
| Snapshot test | Integration test using `insta::assert_snapshot!` | `crates/famp/tests/install_uninstall_roundtrip.rs` |
| CLI test | `crates/famp/tests/<name>.rs` using `assert_cmd` | `crates/famp/tests/inspect_broker.rs` |

Integration test files are named after what they test, not after the module
they cover. Property tests are conventionally prefixed with `prop` (e.g.,
`prop01_dm_fanin_order.rs`, `prop02_channel_fanout.rs`).

### Shared test helpers

Tests in the `famp` crate share helpers via `crates/famp/tests/common/`:

| Helper | Purpose |
|---|---|
| `child_guard.rs` | `ChildGuard` RAII wrapper — kills and waits a `Child` on drop |
| `listen_harness.rs` | In-process daemon launch for listen-subprocess tests |
| `mcp_harness.rs` | MCP tool call scaffolding |
| `conversation_harness.rs` | Multi-turn conversation setup helpers |
| `two_daemon_harness.rs` | Spawns two daemons with mutual peer registration |

Tests in the `famp-bus` crate share a `TestEnv` struct via
`crates/famp-bus/tests/common/mod.rs` that combines `InMemoryMailbox` and
`FakeLiveness` for actor-layer tests without I/O.

### ChildGuard convention (mandatory for subprocess tests)

Any integration test that spawns a `famp register` or `famp daemon` subprocess
**must** wrap the `std::process::Child` in a `ChildGuard`. `ChildGuard`
performs a `kill` + `wait` on `Drop`, which prevents process leaks when a test
panics and avoids cascading respawn races on the shared broker socket.

Include the guard by path:

```rust
#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

let child = Command::new("famp").args(["daemon", "start"]).spawn()?;
let _guard = ChildGuard::new(child); // reaped on scope exit, even on panic
```

Tests that leak broker children have caused flaky respawn races observed in
CI and on macOS (`EADDRINUSE` even with port-0 binding).

### Property tests with proptest

Add `proptest = { workspace = true }` to the crate's `[dev-dependencies]` and
use the `proptest!` macro:

```rust
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn my_invariant(input in arb_my_type()) {
        // assert invariant holds for all generated inputs
    }
}
```

### Snapshot tests with insta

Snapshot files live in `crates/<crate>/tests/snapshots/`. Review pending
snapshots with `cargo insta review` after a test run that produces new
snapshots. Commit snapshot files alongside the test.

---

## Coverage Requirements

No coverage thresholds are configured. The conformance and property-test gates
serve as the correctness boundary: tests in `test-canonical-strict` and
`test-crypto` must pass with `--no-fail-fast` before a PR is merged.

---

## CI Integration

Three workflows run tests automatically.

### `ci.yml` — per push and pull request

Triggers on every push and pull request to any branch (excluding changes to
`docs/**`, `.planning/**`, and `**/*.md`).

| Job | OS | Command |
|---|---|---|
| `fmt-check` | ubuntu | `cargo fmt --all -- --check` |
| `clippy` | ubuntu | `cargo clippy --workspace --all-targets -- -D warnings` |
| `build` | ubuntu + macos | `cargo build --workspace --all-targets` |
| `test-canonical` | ubuntu | `just test-canonical-strict` |
| `test-crypto` | ubuntu | `just test-crypto` |
| `test` | ubuntu + macos | `cargo nextest run --workspace --profile ci` |
| `doc-test` | ubuntu | `cargo test --workspace --doc` |
| `audit` | ubuntu | RustSec advisory check via `rustsec/audit-check` |

The `test` job runs with the `ci` nextest profile: `fail-fast = false`, 120 s
slow timeout, `failure-output = "immediate-final"`.

Concurrency: each `github.ref` cancels any in-progress run for the same ref.

### `smoke-test.yml` — install path verification

Triggers on pushes and pull requests that touch `crates/**`, `Cargo.toml`,
`Cargo.lock`, `rust-toolchain.toml`, or `README.md`.

Installs the `famp` binary from source using `cargo install` and asserts that
`famp install-claude-code` writes all five expected artifacts. Run locally with:

```bash
just smoke-test
```

### `nightly-full-corpus.yml` — 100 M float corpus gate

Triggers on release tags (`v*`) and manual dispatch. Downloads the
cyberphone es6testfile100m corpus, verifies its SHA-256, and runs
`just test-canonical-full`. This is a required release gate; it is not run
per-PR.

---

## nextest Profile Notes

`.config/nextest.toml` defines two profiles.

**`default`** (local development):
- `fail-fast = true` — stop on first failure for fast feedback
- `slow-timeout = { period = "60s", terminate-after = 2 }`

**`ci`** (GitHub Actions):
- `fail-fast = false` — surface all failures in one run
- `slow-timeout = { period = "120s", terminate-after = 3 }`
- `failure-output = "immediate-final"`

Two test groups throttle concurrency for subprocess-heavy tests:

| Group | Filter | `max-threads` |
|---|---|---|
| `listen-subprocess` | `package(famp)` and listen/conversation tests | 4 |
| `inspect-subprocess` | `package(famp)` and inspect tests | 1 |

The `listen-subprocess` limit was introduced to eliminate `EADDRINUSE` flakes
observed on macOS when many port-0 listener tests ran in parallel.
