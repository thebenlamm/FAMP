---
phase: 12-v1-0-0-release-gate
plan: 04
requirement: REL-03
---

# 12-CI-ATTESTATION — Green-Gate Attestation at the Tag-Candidate SHA

This record is the live, independently re-queried evidence that the exact
commit `v1.0.0` will be tagged at has a fully green CI run. It supersedes any
prior written claim about CI state — in particular `.planning/REQUIREMENTS.md`
line 89's now-corrected claim that CI's green run was on `ca59f48` (a
docs-only commit that in fact triggered **zero** check-runs, per `ci.yml`'s
`paths-ignore` on `docs/**`, `.planning/**`, `**/*.md`). The SHA below was
read once via `git rev-parse HEAD` immediately after Task 1's bump commit and
is used by value throughout — never `HEAD`, which a parallel window could
move.

## Tag Candidate

- **SHA:** `5edff41835b9c8e6daa59a51efce549460d88e5b`
- **Subject:** `feat(12-04): bump workspace version 1.0.0-rc.1 -> 1.0.0 (REL-05 bump)`
- **Branch:** confirmed via `git branch -r --contains <sha>` → `origin/main`
- **Query timestamp:** 2026-07-29T23:45:13Z (all 11 check-runs polled to `completed` before this query; conclusions read only after every run reported `completed`, never while any run was still `in_progress` or `queued`)

## Check-Run Attestation

**Total check-runs:** 11 (`total_count` from `gh api /repos/thebenlamm/FAMP/commits/<sha>/check-runs`) — non-zero, satisfying the `paths-ignore`-trap guard (a docs-only commit yields `total_count == 0`, which is NOT-GREEN and would block; this commit touches `Cargo.toml`/`Cargo.lock`/`crates/famp/src/cli/mod.rs`, all non-ignored paths, so the workflow fired for real).
**Not-completed count:** 0. **Not-success count:** 0. Every one of the 11 expected job names (matched by NAME, not array position — the API does not guarantee stable ordering) is present with `conclusion: success`.

| Name | Run ID | Status | Conclusion |
|------|--------|--------|------------|
| fmt-check | 90738389929 | completed | success |
| clippy | 90738389900 | completed | success |
| build (ubuntu-latest) | 90738389974 | completed | success |
| build (macos-latest) | 90738389931 | completed | success |
| test (ubuntu-latest) | 90738472116 | completed | success |
| test (macos-latest) | 90738472077 | completed | success |
| doc-test | 90738389909 | completed | success |
| audit | 90738389917 | completed | success |
| famp-canonical RFC 8785 conformance gate | 90738389890 | completed | success |
| famp-crypto §7.1c worked-example + RFC 8032 gate | 90738389915 | completed | success |
| smoke-test (Quick Start install path) | 90738389732 | completed | success |

**Workflow runs (Actions run list, same SHA):**

| Workflow | Run ID | Conclusion | URL |
|----------|--------|------------|-----|
| ci | 30500333880 | success | https://github.com/thebenlamm/FAMP/actions/runs/30500333880 |
| smoke-test | 30500333868 | success | https://github.com/thebenlamm/FAMP/actions/runs/30500333868 |

**Cross-host E2E coverage note:** REL-03's two named cross-host E2E tests,
`e2e_cross_host_delivery` and `e2e_shipping_surface`, do not appear as
standalone check-runs — both live in `crates/famp-gateway/tests/` and execute
inside the `test` matrix jobs (`cargo nextest run --workspace --profile ci`,
bounded to `max-threads = 2` via the `gateway-subprocess` nextest test-group).
A green `test (ubuntu-latest)` and `test (macos-latest)` above is the
evidence for both.

## §16 Re-Attestation by Citation

Design review C §16's "Exact release ruling" lists nine conditions for
tagging `v1.0.0`. Items 1, 2, 3, 4, 5, 7 were already satisfied by Phase 11
and are re-attested here by citation to `11-VERIFICATION.md` (no new
federation logic in this phase). Items 6, 8, 9 are confirmed closed by this
phase's own plans.

| # | §16 Item | Disposition | Citation |
|---|----------|-------------|----------|
| 1 | The shipping client accepts a complete remote principal | Satisfied (Phase 11) | `11-VERIFICATION.md` truth row #1 (`famp send --to agent:<domain>/<name>` splits `Target`/envelope `to`, delivers into the remote mailbox) |
| 2 | The signed envelope contains globally qualified `from` and `to` | Satisfied (Phase 11) | `11-VERIFICATION.md` truth row #1 (`build_envelope_value`/`build_remote_envelope_value`) and truth row #2 (typed, FSM-driving, sign-then-strip envelopes) |
| 3 | `from` is bound to broker-authenticated identity | Satisfied (Phase 11) | `11-VERIFICATION.md` SEC-01 row (`crates/famp-bus/src/broker/handle.rs::send`, `is_self_authored(envelope, Some(&effective_identity))` gate before mailbox write) |
| 4 | No `local.bus` authority crosses federation | Satisfied (Phase 11) | `11-VERIFICATION.md` truth row #1 (bus target stays bare leaf, only the signed envelope carries domain-qualified authority) and SEC-02 row (ingress authoritative only for own domain + addressed mailbox) |
| 5 | The remote gateway verifies and delivers the same signed bytes | Satisfied (Phase 11) | `11-VERIFICATION.md` truth row #6 (`e2e_shipping_surface.rs` drives the real `famp send`, cross-platform fixtures regenerated) and SEC-02 row (`federation_format_ok` wired into `inbox_handler`) |
| 6 | Existing canonicalization and E2E gates remain green | Satisfied (this phase, REL-03) | This record — `famp-canonical RFC 8785 conformance gate` and both `test (*)` jobs (which include `e2e_cross_host_delivery`/`e2e_shipping_surface`) all `success` at SHA `5edff41` above |
| 7 | Ambiguous route configuration fails closed | Satisfied (Phase 11, reconfirmed Phase 12) | `11-VERIFICATION.md` SEC-04 row (`--backs` flag, duplicate/ambiguous config rejects at parse/startup); reconfirmed by `12-02-SUMMARY.md`'s route-config fail-closed fix (`crates/famp-gateway/tests/route_config_fail_closed.rs`) |
| 8 | The documentation states what `send` confirms | Satisfied (this phase, REL-01) | `12-01-SUMMARY.md` — `docs/GATEWAY-SETUP.md` §6, `famp send --help`, and README all state the fire-and-forget send-confirmation boundary, pinned by `gateway_setup_doc_accuracy.rs` |
| 9 | Zed's independent source verdict is reconciled | Satisfied (this phase, REL-02) | `12-02-SUMMARY.md` / `12-REL-02-REVIEW.md` — two-reviewer (self + codex) independent adversarial pass over the shipped trust boundary, 10 findings triaged, 2 real defects fixed with regression tests |

## Deployed Binary Staleness

The actual `~/.cargo/bin/famp` and `~/.cargo/bin/famp-gateway` binaries on
Ben's dogfooded machines (`bens-macbook-air`, `home-devbox` per
`11-HUMAN-UAT.md`'s topology) were built before this bump and will continue
reporting `1.0.0-rc.1` via `famp -V` until reinstalled. This is **expected**
and is **not** a failed bump — do not mistake stale `famp -V` output on those
machines for a regression.

Reinstall commands (not required by REL-05, which governs the tag only, not
live redeployment):

- macOS (`bens-macbook-air`): `just install-all`
- Linux box without `just` (`home-devbox`): the raw per-crate equivalent,
  `cargo install --path crates/famp --locked --force` and
  `cargo install --path crates/famp-gateway --locked --force`

Redeployment is explicitly **not** a gate for this phase or for the
`v1.0.0` tag plan (12-05) — it is an operational follow-up.
