# Federation tests — retired 2026-07-27 (Phase 10, TEST-01)

This directory formerly held 27 parked test files (26 `.rs` + one
`e2e_two_daemons.rs.deferred`) exercising the v0.8 federation CLI
(`famp init` / `setup` / `listen` / `peer add`), which was hard-deleted in
v0.9 Phase 4 (commit `feat!(04): remove federation CLI surface ...`). They
were kept dormant here as intent documents pending the v1.0 federation
milestone.

**Phase 10's triage concluded 27/27 RETIRE, 0 REACTIVATE**: every file
traces to a deleted v0.8 CLI symbol with no rewrite target against the
current `famp-bus`/`famp-gateway` API — the bar for reactivation (D-02: "the
behavior still exists on a shipping surface today") was not met by any of
them. 12 of the 27 dispositions independently point at a named, currently-
green covering test that proves the same intent against the live API.

All 27 files were deleted in this commit. See [`TRIAGE.md`](./TRIAGE.md) for
the full per-file rationale ledger — every removal is documented, not
silent.

This directory is retained only to hold that ledger and this banner.
Reactivation from this history is not applicable: any future federation-
adjacent test need should be authored from scratch against the current
`famp-bus`/`famp-gateway` API, not resurrected from a file that once lived
here.

## See also

- [`TRIAGE.md`](./TRIAGE.md) — the TEST-01 retirement ledger (this phase).
- [`docs/history/v0.9-prep-sprint/famp-local/`](../../../../docs/history/v0.9-prep-sprint/famp-local/) - archived prep-sprint scaffolding
- [`docs/MIGRATION-v0.8-to-v0.9.md`](../../../../docs/MIGRATION-v0.8-to-v0.9.md) - migration guide
- `v0.8.1-federation-preserved` git tag - escape hatch for federation users (run `git checkout v0.8.1-federation-preserved` to restore the v0.8 federation CLI)
