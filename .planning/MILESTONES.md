# Milestones

## v0.5.1 Spec Fork (Shipped: 2026-04-13)

**Phases completed:** 2 phases, 9 plans, 15 tasks

**Key accomplishments:**

- rust-toolchain.toml pinning Rust 1.87.0 with rustfmt + clippy, dual Apache-2.0/MIT license files, .gitignore, docs/ placeholder, and copy-pasteable bootstrap README
- 13-crate Cargo workspace with [workspace.dependencies] pinning all 16 protocol-stack crates, strict clippy deny-all lints, and green cargo build + test on empty stubs
- Justfile + nextest two-profile config + 6-job GitHub Actions workflow establishing a CI-parity gate where `just ci` green locally implies green CI on push
- FAMP-v0.5.1-spec.md stub at repo root with FAMP_SPEC_VERSION = "0.5.1" constant, plus scripts/spec-lint.sh ripgrep anchor lint wired into `just ci` as a mandatory gate.
- One-liner:
- One-liner:
- One-liner:
- 1. [Rule 1 — Bug] Fixed SPEC-01-FULL counter regex in `scripts/spec-lint.sh`

---
