---
phase: 00-toolchain-workspace-scaffold
plan: 02
type: execute
wave: 2
depends_on: ["00-01"]
files_modified:
  - Cargo.toml
  - rustfmt.toml
  - crates/famp-core/Cargo.toml
  - crates/famp-core/src/lib.rs
  - crates/famp-canonical/Cargo.toml
  - crates/famp-canonical/src/lib.rs
  - crates/famp-crypto/Cargo.toml
  - crates/famp-crypto/src/lib.rs
  - crates/famp-envelope/Cargo.toml
  - crates/famp-envelope/src/lib.rs
  - crates/famp-identity/Cargo.toml
  - crates/famp-identity/src/lib.rs
  - crates/famp-causality/Cargo.toml
  - crates/famp-causality/src/lib.rs
  - crates/famp-fsm/Cargo.toml
  - crates/famp-fsm/src/lib.rs
  - crates/famp-protocol/Cargo.toml
  - crates/famp-protocol/src/lib.rs
  - crates/famp-extensions/Cargo.toml
  - crates/famp-extensions/src/lib.rs
  - crates/famp-transport/Cargo.toml
  - crates/famp-transport/src/lib.rs
  - crates/famp-transport-http/Cargo.toml
  - crates/famp-transport-http/src/lib.rs
  - crates/famp-conformance/Cargo.toml
  - crates/famp-conformance/src/lib.rs
  - crates/famp/Cargo.toml
  - crates/famp/src/lib.rs
  - crates/famp/src/bin/famp.rs
autonomous: true
requirements: [TOOL-02, TOOL-06, TOOL-07]
must_haves:
  truths:
    - "`cargo build --workspace` succeeds with zero warnings on empty stubs"
    - "`cargo clippy --workspace --all-targets -- -D warnings` passes on empty stubs"
    - "Every one of the 13 crates compiles to an rlib (or bin for famp)"
    - "`famp` binary executes and prints the placeholder string"
    - "All external crate versions are pinned exactly once in [workspace.dependencies]"
    - "No crate in the workspace declares its own version for a workspace dep"
  artifacts:
    - path: Cargo.toml
      provides: "Workspace root with [workspace], [workspace.package], [workspace.dependencies], [workspace.lints]"
      contains: "[workspace]"
      min_lines: 80
    - path: rustfmt.toml
      provides: "Stable rustfmt config"
      contains: "max_width"
    - path: crates/famp-core/src/lib.rs
      provides: "Empty stub for famp-core"
      contains: "#![forbid(unsafe_code)]"
    - path: crates/famp/src/bin/famp.rs
      provides: "Umbrella binary proving workspace produces an executable"
      contains: "famp v0.5.1 placeholder"
  key_links:
    - from: crates/*/Cargo.toml
      to: Cargo.toml
      via: "[lints] workspace = true and [package] inherited fields"
      pattern: "workspace = true"
---

<objective>
Scaffold the full 13-crate Cargo workspace (12 library crates + 1 umbrella with binary) with `[workspace.dependencies]` pinning every crate from the CLAUDE.md tech-stack table, `[workspace.lints]` enforcing strict clippy + `unsafe_code = "forbid"`, and every member crate inheriting via `workspace = true`. On completion, `cargo build --workspace` and `cargo clippy --workspace -- -D warnings` both succeed on empty stubs.

Purpose: Locks the final crate DAG (TOOL-02), centralizes version pinning (TOOL-06), and activates strict lint baseline (TOOL-07) so every subsequent phase inherits the constraints for free.
Output: Root Cargo.toml, rustfmt.toml, 13 crate directories with Cargo.toml + src/lib.rs stubs, 1 bin.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@CLAUDE.md
@.planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md
@rust-toolchain.toml
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write workspace root Cargo.toml + rustfmt.toml</name>
  <read_first>
    - CLAUDE.md (§Technology Stack — full version table; §Lint / format — clippy settings; §Cargo workspace)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-04 through D-17)
  </read_first>
  <files>Cargo.toml, rustfmt.toml</files>
  <action>
Create `Cargo.toml` at repo root with EXACT content:

```toml
[workspace]
resolver = "2"
members = [
  "crates/famp-core",
  "crates/famp-canonical",
  "crates/famp-crypto",
  "crates/famp-envelope",
  "crates/famp-identity",
  "crates/famp-causality",
  "crates/famp-fsm",
  "crates/famp-protocol",
  "crates/famp-extensions",
  "crates/famp-transport",
  "crates/famp-transport-http",
  "crates/famp-conformance",
  "crates/famp",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.87"
license = "Apache-2.0 OR MIT"
repository = "https://github.com/benlamm/FAMP"
authors = ["FAMP contributors"]

[workspace.dependencies]
# Crypto
ed25519-dalek = { version = "2.2.0", default-features = false, features = ["std", "zeroize"] }
sha2 = { version = "0.11.0", default-features = false }

# Canonical JSON + serde
serde = { version = "1.0.228", features = ["derive"] }
serde_json = { version = "1.0.149", default-features = false, features = ["std"] }
serde_jcs = "0.2.0"

# Identifiers
uuid = { version = "1.23.0", features = ["v7", "serde"] }
base64 = "0.22.1"

# HTTP / async
axum = "0.8.8"
reqwest = { version = "0.13.2", default-features = false, features = ["rustls-tls-native-roots", "json"] }
rustls = { version = "0.23.38", default-features = false, features = ["ring", "std", "tls12"] }
tokio = { version = "1.51.1", default-features = false }

# Errors
thiserror = "2.0.18"
anyhow = "1.0.102"

# Testing
proptest = "1.11.0"
stateright = "0.31.0"
insta = "1.47.2"

[workspace.lints.rust]
unsafe_code = "forbid"
unused_crate_dependencies = "warn"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"

[profile.release]
lto = "thin"
codegen-units = 1
```

Create `rustfmt.toml` at repo root with EXACT content (D-17 — stable rustfmt only):

```toml
edition = "2021"
max_width = 100
```

Verify `cargo metadata --format-version 1 --no-deps` exits 0 after creation (workspace parses). The stub crate directories don't exist yet — that's Task 2 — so `cargo build` will fail at this point and that's expected.
  </action>
  <verify>
    <automated>test -f Cargo.toml && test -f rustfmt.toml && grep -q '^\[workspace\]$' Cargo.toml && grep -q '^\[workspace.dependencies\]$' Cargo.toml && grep -q 'ed25519-dalek = { version = "2.2.0"' Cargo.toml && grep -q 'unsafe_code = "forbid"' Cargo.toml && grep -q 'max_width = 100' rustfmt.toml</automated>
  </verify>
  <acceptance_criteria>
    - `Cargo.toml` exists with `[workspace]` table
    - `members` array lists exactly 13 crates (grep count: `grep -c '  "crates/famp' Cargo.toml` equals 13)
    - `[workspace.dependencies]` pins all of: ed25519-dalek 2.2.0, sha2 0.11.0, serde 1.0.228, serde_json 1.0.149, serde_jcs 0.2.0, uuid 1.23.0, base64 0.22.1, axum 0.8.8, reqwest 0.13.2, rustls 0.23.38, tokio 1.51.1, thiserror 2.0.18, anyhow 1.0.102, proptest 1.11.0, stateright 0.31.0, insta 1.47.2 (each grep-able by name+version)
    - `[workspace.lints.rust]` contains `unsafe_code = "forbid"` (TOOL-07)
    - `[workspace.lints.clippy]` contains `unwrap_used = "deny"` and `expect_used = "deny"`
    - `rustfmt.toml` exists with `edition = "2021"` and `max_width = 100`
    - `[workspace.package]` present with `rust-version = "1.87"` and `license = "Apache-2.0 OR MIT"`
  </acceptance_criteria>
  <done>Workspace root TOML declares the full 13-crate DAG, pins every dep once, and enforces strict lints at workspace level. Crate bodies come in Task 2.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Scaffold all 13 crate stubs with inherited lints + smoke tests</name>
  <read_first>
    - Cargo.toml (written in Task 1 — for [workspace.dependencies] reference)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-23, D-24, D-25)
  </read_first>
  <files>
    crates/famp-core/Cargo.toml, crates/famp-core/src/lib.rs,
    crates/famp-canonical/Cargo.toml, crates/famp-canonical/src/lib.rs,
    crates/famp-crypto/Cargo.toml, crates/famp-crypto/src/lib.rs,
    crates/famp-envelope/Cargo.toml, crates/famp-envelope/src/lib.rs,
    crates/famp-identity/Cargo.toml, crates/famp-identity/src/lib.rs,
    crates/famp-causality/Cargo.toml, crates/famp-causality/src/lib.rs,
    crates/famp-fsm/Cargo.toml, crates/famp-fsm/src/lib.rs,
    crates/famp-protocol/Cargo.toml, crates/famp-protocol/src/lib.rs,
    crates/famp-extensions/Cargo.toml, crates/famp-extensions/src/lib.rs,
    crates/famp-transport/Cargo.toml, crates/famp-transport/src/lib.rs,
    crates/famp-transport-http/Cargo.toml, crates/famp-transport-http/src/lib.rs,
    crates/famp-conformance/Cargo.toml, crates/famp-conformance/src/lib.rs,
    crates/famp/Cargo.toml, crates/famp/src/lib.rs, crates/famp/src/bin/famp.rs
  </files>
  <behavior>
    - Test 1: `cargo build --workspace --all-targets` exits 0 with zero warnings
    - Test 2: `cargo clippy --workspace --all-targets -- -D warnings` exits 0
    - Test 3: `cargo test --workspace` reports at least 13 tests run (one smoke test per crate)
    - Test 4: `cargo run --bin famp` prints `famp v0.5.1 placeholder`
    - Test 5: `grep -r "workspace = true" crates/*/Cargo.toml` returns 13 matches (every crate inherits lints)
  </behavior>
  <action>
For EACH of the 12 library crates (famp-core, famp-canonical, famp-crypto, famp-envelope, famp-identity, famp-causality, famp-fsm, famp-protocol, famp-extensions, famp-transport, famp-transport-http, famp-conformance):

**`crates/{name}/Cargo.toml`** — EXACT template (substitute `{name}`):

```toml
[package]
name = "{name}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "FAMP v0.5.1 — {name} crate (stub)"

[lints]
workspace = true

[dependencies]
```

**`crates/{name}/src/lib.rs`** — EXACT content:

```rust
//! `{name}` — FAMP v0.5.1 reference implementation.
//!
//! Phase 0 stub. Bodies land in later phases.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles_and_links() {
        // Smoke test per D-25: ensures nextest reports >0 tests per crate
        // so a broken runner fails loudly instead of silently passing.
    }
}
```

For the **umbrella crate `crates/famp/`**:

**`crates/famp/Cargo.toml`**:

```toml
[package]
name = "famp"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "FAMP v0.5.1 — umbrella crate and CLI"

[lints]
workspace = true

[dependencies]

[[bin]]
name = "famp"
path = "src/bin/famp.rs"
```

**`crates/famp/src/lib.rs`** (D-24):

```rust
//! `famp` — FAMP v0.5.1 umbrella crate.
//!
//! Phase 0 stub. Re-exports land in Phase 8.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles_and_links() {}
}
```

**`crates/famp/src/bin/famp.rs`** (D-24):

```rust
#![forbid(unsafe_code)]

fn main() {
    println!("famp v0.5.1 placeholder");
}
```

**Implementation order:**
1. Create all 13 directories: `mkdir -p crates/{famp-core,famp-canonical,famp-crypto,famp-envelope,famp-identity,famp-causality,famp-fsm,famp-protocol,famp-extensions,famp-transport,famp-transport-http,famp-conformance,famp}/src`
2. Create `crates/famp/src/bin/` additionally
3. Write each Cargo.toml + lib.rs (use the Write tool, not heredoc)
4. Run `cargo build --workspace --all-targets` — MUST succeed
5. Run `cargo clippy --workspace --all-targets -- -D warnings` — MUST succeed (empty stubs pass strict clippy because they have no public items to trigger pedantic lints)
6. Run `cargo test --workspace` — MUST report 13 passing tests
7. Run `cargo run --bin famp` — MUST print `famp v0.5.1 placeholder`

**If clippy fails on empty stubs:** a specific pedantic lint may need to move to `allow` in root `Cargo.toml` `[workspace.lints.clippy]`. CONTEXT D-16 authorizes this fine-tuning. Document any additions in the commit message.

**Do NOT** add any `[dependencies]` entries to the empty stubs. Phase 0 locks the DAG; bodies land in later phases by flipping `workspace = true` flags.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets 2>&1 | tee /tmp/build.log && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee /tmp/clippy.log && cargo test --workspace 2>&1 | tee /tmp/test.log && cargo run --bin famp 2>&1 | grep -q "famp v0.5.1 placeholder" && [ $(grep -c "workspace = true" crates/*/Cargo.toml) -eq 13 ]</automated>
  </verify>
  <acceptance_criteria>
    - All 13 `crates/*/Cargo.toml` files exist
    - All 13 `crates/*/src/lib.rs` files exist
    - `crates/famp/src/bin/famp.rs` exists
    - Every `src/lib.rs` contains literal `#![forbid(unsafe_code)]`
    - Every `src/lib.rs` contains a `crate_compiles_and_links` smoke test
    - Every `crates/*/Cargo.toml` contains `[lints]\nworkspace = true` (grep `workspace = true` returns exactly 13)
    - `cargo build --workspace --all-targets` exits 0 with no warnings
    - `cargo clippy --workspace --all-targets -- -D warnings` exits 0
    - `cargo test --workspace` reports ≥ 13 tests passed, 0 failed
    - `cargo run --bin famp` stdout contains `famp v0.5.1 placeholder`
    - No stub crate declares any external `[dependencies]` (grep `^[a-z].*=.*version` in crates/*/Cargo.toml returns 0 non-metadata lines)
  </acceptance_criteria>
  <done>13-crate workspace compiles, lints clean, tests run, and the umbrella binary runs. The full DAG is locked; later phases just fill in bodies.</done>
</task>

</tasks>

<verification>
```bash
cargo build --workspace --all-targets && \
cargo clippy --workspace --all-targets -- -D warnings && \
cargo test --workspace && \
cargo run --bin famp | grep -q "famp v0.5.1 placeholder" && \
[ $(ls crates/ | wc -l) -eq 13 ]
```
</verification>

<success_criteria>
- Cargo.toml pins every crate from CLAUDE.md tech stack table (TOOL-06)
- `unsafe_code = "forbid"` active at workspace level (TOOL-07)
- 13 crates compile, clippy-clean, test-green on empty stubs (TOOL-02)
- `famp` binary executes
- Zero per-crate version drift possible
</success_criteria>

<output>
After completion, create `.planning/phases/00-toolchain-workspace-scaffold/0-02-SUMMARY.md`
</output>
