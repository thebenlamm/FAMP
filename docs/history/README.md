# Project History

This directory is the **curated record of how FAMP came to be**, extracted
from a private working archive of planning artifacts. Four documents live
here:

- [`MILESTONES.md`](MILESTONES.md) — what each shipped milestone delivered,
  with test counts, requirement counts, and the engineering choices that
  defined it.
- [`RETROSPECTIVE.md`](RETROSPECTIVE.md) — living per-milestone lessons:
  what worked, what was inefficient, what patterns held across milestones.
- [`ROADMAP.md`](ROADMAP.md) — milestone roadmap with completed phases,
  the v0.9 re-scope, the v1.0 federation profile sketch, and the open
  backlog (999.x items captured during dogfooding).
- [`PROJECT.md`](PROJECT.md) — the project's intent, scope model, and
  key decisions (Personal Profile vs. Federation Profile, why Rust, why
  `serde_jcs`, why `verify_strict`-only, etc.).

## What this is — and is not

**This is not the development log.** Day-to-day phase plans, decision
discussions, and quick-task artifacts live in a private workspace.
What's here was selected because it explains *why* the implementation
took the shape it did — useful for anyone evaluating the protocol or
contributing to it.

**This is also not a substitute for the spec.** The authoritative
protocol contract is [`FAMP-v0.5.1-spec.md`](../../FAMP-v0.5.1-spec.md)
at the repo root.

## Stale-link advisory

Some internal references in these files point to archived artifacts in
the maintainer's private workspace (e.g., `milestones/v0.6-ROADMAP.md`,
`milestones/v0.6-phases/`). Those links will not resolve from this
directory. The surrounding prose stands on its own — the link targets
were intermediate execution artifacts, not the public design record.

External references (the spec, `scripts/famp-local`, the v0.9 design
spec under `docs/superpowers/specs/`) have been re-pointed to their
public locations.

## How this gets updated

These files are snapshots of the curated narrative as of the most
recent shipped milestone. They are updated at milestone boundaries,
not on every commit. For current implementation state, read the code
and CI status; for protocol contract, read the spec.
