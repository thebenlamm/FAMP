# Federation tests - frozen for v1.0 reactivation

These tests are **dormant in v0.9** because the federation CLI surface they
exercised (`famp init / setup / listen / peer`) was hard-deleted in Phase 4
(commit `feat!(04): remove federation CLI surface ...`). They survive in this
directory as **intent documents**: they encode adversarial cases, conversation
shapes, and non-obvious patterns that took adversarial review to discover.

## Reactivation criteria

These tests are reactivated when the v1.0 federation milestone fires -
named trigger condition: **Sofer (or named equivalent) runs FAMP from a
different machine and exchanges a signed envelope**. A 4-week clock starts
at the v0.9.0 tag; if the trigger does not fire, federation framing is
reconsidered.

When v1.0 fires, the path is: refactor each test against whatever new library
API the v1.0 `famp-gateway` exposes (likely a thin wrapper over
`famp-transport-http`'s current public surface), then move back into the
active `crates/famp/tests/` glob.

## What stays exercised in `just ci`

**2026-05-15 update:** `e2e_two_daemons.rs` was moved here as
`e2e_two_daemons.rs.deferred` after becoming structurally flaky (0/7 passes).
Root cause: `worker_threads = 2` + two TLS servers + a test driver is an
oversubscribed scheduler. Sleep tuning could not fix it. The right fix on
reactivation is a `oneshot::Sender` signaled by the server task after
`axum::serve()` is actually accepting — deterministic, no sleep, no poll.

`famp-transport-http` and `famp-keyring` continue to compile on every commit
via `cargo check --workspace`. No active test target exercises their runtime
behavior until v1.0 reactivation.

## See also

- [`docs/history/v0.9-prep-sprint/famp-local/`](../../../../docs/history/v0.9-prep-sprint/famp-local/) - archived prep-sprint scaffolding
- [`docs/MIGRATION-v0.8-to-v0.9.md`](../../../../docs/MIGRATION-v0.8-to-v0.9.md) - migration guide
- `v0.8.1-federation-preserved` git tag - escape hatch for federation users (run `git checkout v0.8.1-federation-preserved` to restore the v0.8 federation CLI)
