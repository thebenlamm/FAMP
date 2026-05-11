# ADR 0002: Dual-license under Apache-2.0 OR MIT

**Status:** Accepted (retroactive — codifies a Phase 0 default)
**Date:** 2026-05-11
**Context commit:** `02c0db5` (license files added 2026-04-12)

## Context

Every workspace crate inherits `license = "Apache-2.0 OR MIT"` from
`[workspace.package]`. The two license texts ship at the repo root as
`LICENSE-APACHE` and `LICENSE-MIT`, and the `## License` section of the
README points at both.

This was adopted as the default during the Phase 0 toolchain scaffold
(`.planning/milestones/v0.5.1-phases/00-toolchain-workspace-scaffold/`,
decision D-07) without a written rationale. This ADR records the
reasoning so future contributors don't have to reverse-engineer it.

## Decision

FAMP is dual-licensed under **Apache-2.0 OR MIT**, at the downstream
user's choice. All crates in the workspace inherit this from
`[workspace.package].license`. New crates added to the workspace MUST
use `license.workspace = true` rather than declaring a per-crate
license.

## Rationale

1. **MIT alone — maximum permissiveness, but silent on patents.**
   Short, well-understood, GPL-compatible. Says nothing about patent
   grants, which is a real risk for a protocol implementation doing
   signed messages, FSMs, and federation semantics. Enterprise legal
   review typically rejects MIT-only crypto/protocol code on patent
   grounds.

2. **Apache-2.0 alone — explicit patent grant, but GPLv2-incompatible.**
   Includes an express patent license from contributors and a
   patent-retaliation clause. But Apache-2.0 is incompatible with
   GPLv2, which would block a chunk of the Linux/OSS world from
   embedding FAMP.

3. **`OR` (not `AND`) — downstream picks.** Offering both removes both
   objections: GPLv2 projects take the MIT side; corporate/legal-heavy
   downstreams take the Apache-2.0 side for the patent clause. Neither
   side is forced on anyone.

4. **Ecosystem alignment.** This is the license combination used by
   `rustc`, `tokio`, `serde`, `axum`, `ed25519-dalek`, `reqwest`,
   `rustls`, and effectively the entire Rust dependency tree we pull
   in. Matching the ecosystem default means zero license-compatibility
   friction with our own dependencies, no surprise for downstream
   consumers, and `cargo-deny` license scans pass out of the box when
   we add them.

## Alternatives considered

- **MIT-only** — rejected: no patent grant; blocks corporate adoption
  of a crypto/protocol library.
- **Apache-2.0-only** — rejected: GPLv2-incompatible; blocks a
  meaningful slice of OSS downstreams.
- **MPL-2.0** — rejected: file-level copyleft creates friction for
  downstream embedding (the primary use case for a protocol library);
  also a non-default in the Rust ecosystem.
- **BSD-3-Clause / ISC** — rejected: same patent-silence problem as
  MIT, with worse ecosystem alignment.
- **GPL / AGPL** — rejected: a reference protocol implementation needs
  to be embeddable in proprietary federation gateways and corporate
  agent runtimes; copyleft forecloses the primary use case.

## Consequences

- Every contributor's contributions are dual-licensed under both
  licenses by default. A `CONTRIBUTING.md` (when written) should state
  this explicitly, mirroring the standard Rust-project boilerplate.
- New crates added to the workspace inherit the license automatically
  via `license.workspace = true`. CI should not need a license-policy
  check beyond `cargo-deny`'s default allow-list.
- Vendored or copied third-party code must be license-compatible with
  both Apache-2.0 and MIT (i.e., MIT, BSD, ISC, Apache-2.0, or
  Unlicense). GPL/LGPL/MPL code cannot be vendored.
- No spec impact. The protocol itself is unaffected; this ADR governs
  the reference implementation only.

## Re-evaluation

This decision rarely needs revisiting. Triggers that would reopen it:

- A second implementer adopts a license that creates a real
  interop-distribution friction (e.g., an AGPL fork that needs upstream
  patches relicensed).
- The workspace adds a crate whose dependencies force a stricter
  license (e.g., GPL-only crypto), at which point that crate would
  need to live outside the dual-licensed workspace.
