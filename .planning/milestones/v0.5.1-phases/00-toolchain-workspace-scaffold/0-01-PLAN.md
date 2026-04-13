---
phase: 00-toolchain-workspace-scaffold
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - rust-toolchain.toml
  - .gitignore
  - LICENSE-APACHE
  - LICENSE-MIT
  - README.md
  - docs/.gitkeep
autonomous: true
requirements: [TOOL-01]
must_haves:
  truths:
    - "rustup respects rust-toolchain.toml and installs 1.87.0 automatically on entry"
    - "`cargo --version` prints 1.87.0 on any machine that clones the repo"
    - "README documents exact bootstrap commands copy-pasteable from CLAUDE.md"
    - "License files exist on disk for crate metadata to reference in Plan 02"
  artifacts:
    - path: rust-toolchain.toml
      provides: "Pinned Rust toolchain version"
      contains: 'channel = "1.87.0"'
    - path: LICENSE-APACHE
      provides: "Apache 2.0 license text"
    - path: LICENSE-MIT
      provides: "MIT license text"
    - path: README.md
      provides: "Phase 0 bootstrap instructions"
    - path: .gitignore
      provides: "Rust + editor ignore rules"
  key_links:
    - from: rust-toolchain.toml
      to: "cargo invocations in Plan 02 + Plan 03"
      via: "rustup auto-selects toolchain on cd into repo"
      pattern: 'channel = "1.87.0"'
---

<objective>
Pin the Rust toolchain to 1.87.0 and lay down repository hygiene files (gitignore, licenses, README bootstrap, docs/ placeholder) so that Plan 02's workspace scaffold can compile against a reproducible toolchain and crate metadata can reference the license files on disk.

Purpose: Establishes the reproducible build floor (TOOL-01) and the minimum repo hygiene Plan 02 depends on.
Output: rust-toolchain.toml, .gitignore, LICENSE-APACHE, LICENSE-MIT, README.md, docs/.gitkeep
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@CLAUDE.md
@.planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md
@.planning/REQUIREMENTS.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Pin Rust toolchain + write .gitignore</name>
  <read_first>
    - CLAUDE.md (§Technology Stack, §Installation commands)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-01, D-02, D-03, D-26)
  </read_first>
  <files>rust-toolchain.toml, .gitignore, docs/.gitkeep</files>
  <action>
Create `rust-toolchain.toml` at repo root with EXACT content (per D-01):

```toml
[toolchain]
channel = "1.87.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

Create `.gitignore` at repo root with EXACT content (per D-26):

```
# Rust
target/
Cargo.lock.bak
**/*.rs.bk

# macOS
.DS_Store

# Editor
*.swp
*.swo
.idea/
.vscode/
```

Create empty `docs/.gitkeep` placeholder so `docs/` exists in git (Phase 1 writes `docs/FAMP-v0.5.1-spec.md` here — see CONTEXT code_context §Integration Points).

Do NOT install rustup in this task — toolchain install is a user action documented in README (Task 3). `rust-toolchain.toml` is declarative; `cargo` reads it automatically.
  </action>
  <verify>
    <automated>test -f rust-toolchain.toml && grep -q 'channel = "1.87.0"' rust-toolchain.toml && test -f .gitignore && grep -q '^target/$' .gitignore && test -f docs/.gitkeep</automated>
  </verify>
  <acceptance_criteria>
    - `rust-toolchain.toml` exists at repo root
    - File contains literal string `channel = "1.87.0"` (grep exits 0)
    - File contains literal string `components = ["rustfmt", "clippy"]` (grep exits 0)
    - `.gitignore` exists at repo root
    - `.gitignore` contains a line exactly `target/` (grep `^target/$` exits 0)
    - `docs/.gitkeep` exists (zero-byte file OK)
  </acceptance_criteria>
  <done>Toolchain pin file and gitignore committed; docs/ directory tracked in git.</done>
</task>

<task type="auto">
  <name>Task 2: Write license files</name>
  <read_first>
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-07, D-28)
  </read_first>
  <files>LICENSE-APACHE, LICENSE-MIT</files>
  <action>
Per D-07, every crate in Plan 02 will declare `license = "Apache-2.0 OR MIT"`. Both license files MUST exist on disk before Plan 02 runs (otherwise `cargo publish --dry-run` and some lints warn).

Create `LICENSE-APACHE` with the standard Apache License 2.0 text from https://www.apache.org/licenses/LICENSE-2.0.txt (plain text, no substitutions). The full license begins with `                                 Apache License` and ends with `limitations under the License.`.

Create `LICENSE-MIT` with the standard MIT License text:

```
MIT License

Copyright (c) 2026 FAMP contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
  </action>
  <verify>
    <automated>test -f LICENSE-APACHE && test -f LICENSE-MIT && grep -q "Apache License" LICENSE-APACHE && grep -q "MIT License" LICENSE-MIT</automated>
  </verify>
  <acceptance_criteria>
    - `LICENSE-APACHE` exists and contains literal string `Apache License`
    - `LICENSE-MIT` exists and contains literal string `MIT License`
    - `LICENSE-MIT` contains `Copyright (c) 2026 FAMP contributors`
    - `LICENSE-APACHE` file size > 9000 bytes (full Apache 2.0 text, not a stub)
  </acceptance_criteria>
  <done>Both license files on disk, ready to be referenced by `license = "Apache-2.0 OR MIT"` in Plan 02 crate metadata.</done>
</task>

<task type="auto">
  <name>Task 3: Write README with bootstrap instructions</name>
  <read_first>
    - CLAUDE.md (§Installation commands Phase 0 bootstrap)
    - .planning/phases/00-toolchain-workspace-scaffold/0-CONTEXT.md (D-27, D-11, D-12)
  </read_first>
  <files>README.md</files>
  <action>
Create `README.md` at repo root with sections:

1. Project title + one-line description: "FAMP — Federated Agent Messaging Protocol (Rust reference implementation)"
2. Status line: "Phase 0: Toolchain & Workspace Scaffold"
3. `## Prerequisites` — macOS or Linux, git, curl
4. `## Bootstrap` — EXACT commands copy-pasteable:

```bash
# 1. Install rustup (toolchain manager). Skip if already installed.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# 2. Enter the repo (rust-toolchain.toml auto-installs 1.87.0)
cd FAMP
rustc --version   # should print: rustc 1.87.0

# 3. Install dev tools (one-time)
cargo install cargo-nextest --locked
cargo install just --locked

# 4. Verify the full CI-parity loop
just ci
```

5. `## Daily loop` — Document the four Justfile targets (`just build`, `just test`, `just lint`, `just fmt`) and the single pre-push gate (`just ci`). Note: "A green `just ci` locally implies a green GitHub Actions run — the Justfile and CI workflow mirror each other exactly."

6. `## License` — "Dual-licensed under Apache-2.0 OR MIT. See LICENSE-APACHE and LICENSE-MIT."

7. `## Status` — "Phase 0 is the bootstrap phase. Zero FAMP protocol code yet — this phase establishes the reproducible build + test + lint loop."

Keep README under 120 lines; do not add architecture diagrams or protocol explanations (those belong in Phase 1 docs).
  </action>
  <verify>
    <automated>test -f README.md && grep -q "rust-toolchain.toml auto-installs 1.87.0" README.md && grep -q "just ci" README.md && grep -q "cargo install cargo-nextest" README.md && grep -q "Apache-2.0 OR MIT" README.md</automated>
  </verify>
  <acceptance_criteria>
    - `README.md` exists at repo root
    - Contains bootstrap section with `curl --proto '=https'` rustup install command
    - Contains `cargo install cargo-nextest --locked`
    - Contains `cargo install just --locked`
    - Contains reference to `just ci` as pre-push gate
    - Contains `Apache-2.0 OR MIT` license statement
    - Line count ≤ 120: `[ $(wc -l < README.md) -le 120 ]`
  </acceptance_criteria>
  <done>README gives a first-time Rust developer a copy-pasteable path from zero to green `just ci`.</done>
</task>

</tasks>

<verification>
Run after all tasks complete:

```bash
test -f rust-toolchain.toml && \
test -f .gitignore && \
test -f LICENSE-APACHE && \
test -f LICENSE-MIT && \
test -f README.md && \
test -f docs/.gitkeep && \
grep -q 'channel = "1.87.0"' rust-toolchain.toml
```

Exit 0 = plan complete.
</verification>

<success_criteria>
- rust-toolchain.toml pins 1.87.0 (TOOL-01)
- .gitignore excludes target/ and editor cruft
- Both license files on disk (required for Plan 02 crate metadata)
- README documents full bootstrap from zero
- docs/ directory exists in git for Phase 1 consumption
</success_criteria>

<output>
After completion, create `.planning/phases/00-toolchain-workspace-scaffold/0-01-SUMMARY.md`
</output>
