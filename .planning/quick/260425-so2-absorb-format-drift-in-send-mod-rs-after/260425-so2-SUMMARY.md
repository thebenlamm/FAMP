---
quick: 260425-so2
slug: absorb-format-drift-in-send-mod-rs-after
type: style
status: Verified
date-completed: 2026-04-25
key-files:
  modified:
    - crates/famp/src/cli/send/mod.rs
commits:
  fmt: 0807ef9
---

# Quick Task 260425-so2: Format Drift Absorb — Summary

## One-liner

Ran `cargo fmt --all` to absorb two stylistic line-collapse items in
`crates/famp/src/cli/send/mod.rs` introduced during today's pc7 / lny / rz6
commits, closing the resume-doc closing-item check.

## What changed

- **`crates/famp/src/cli/send/mod.rs:22`** — multi-line `use famp_envelope::body::request::{...}`
  collapsed to a single line (within rustfmt's max width).
- **`crates/famp/src/cli/send/mod.rs:563`** — multi-line `eprintln!(...)` collapsed
  to a single line (within rustfmt's max width).

Net diff: +2 / -6 lines. No semantic change.

## Verification

- `cargo fmt --all -- --check` now exits 0 from a clean tree (was exit 1
  pre-task).
- `cargo test -p famp` green (smoke check; non-semantic change does not
  warrant full workspace re-run).

## Why this matters

Closes the final item on the resume doc's punch list. CI's fmt-check gate
(if added later) won't fail on stale today's-work drift. Future devs
reading recent quick commits won't see "but cargo fmt complains."

## Deviations from plan

None.

## Out-of-scope follow-ups

None.
