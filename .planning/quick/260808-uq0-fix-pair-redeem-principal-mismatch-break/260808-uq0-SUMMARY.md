---
quick_id: 260808-uq0
type: quick
status: complete
description: fix pair redeem principal mismatch breaking follower-to-inviter verification
completed: 2026-08-08
files_modified:
  - crates/famp/src/cli/pair/redeem.rs
  - crates/famp/src/cli/pair/invite.rs
  - crates/famp/src/cli/pair/status.rs
  - crates/famp/tests/pair_cli.rs
  - crates/famp-gateway/tests/pairing_e2e.rs
commits:
  - 733410f test(uq0): add RED invariant test for pair redeem principal mismatch
  - 2ff774d fix(uq0): pin pair-redeem principal under --as, matching send's from
  - f092bd5 fix(uq0): validate a pin before it overwrites the peer keyring
---

# Quick Task 260808-uq0: Fix pair redeem principal mismatch — Summary

`famp pair redeem` pinned the redeemer under a hardcoded
`agent:{own_domain}/gateway` principal that no envelope ever carries
(`famp send` builds `from = agent:{own_domain}/{identity}`), so every
follower→inviter message was rejected `UnpinnedKey`. Added a required
`--as` flag deriving the correct principal, updated the invite artifact
to tell followers to supply it, and fixed a separate save-before-validate
ordering bug in the inviter-side pin path that could brick the gateway's
keyring on a failed pin.

## Task 1 — RED invariant test

Added `redeem_pins_principal_matching_send_from_for_same_identity` to
`crates/famp/tests/pair_cli.rs`. It drives `redeem::run_at` through the
existing `spawn_mock_inviter` fixture, captures the principal actually
submitted in `RedemptionRequest.principal` (via the invite record's
`Redeemed { by, .. }` state on the inviter side), and asserts it equals
`format!("agent:{own_domain}/{identity}")` — `send/mod.rs:679`'s exact
construction, copied verbatim per the plan (the real
`build_remote_envelope_value` is private to `cli::send::mod`).

**Red output before the fix** (`cargo test -p famp --test pair_cli
redeem_pins_principal_matching_send_from_for_same_identity`):

```
thread 'redeem_pins_principal_matching_send_from_for_same_identity' panicked at crates/famp/tests/pair_cli.rs:547:5:
assertion `left == right` failed: pair redeem must submit the SAME principal `famp send`'s build_remote_envelope_value (send/mod.rs:679) uses as `from` for identity 'alice': pinned='agent:alice-host.test/gateway' expected='agent:alice-host.test/alice'
  left: "agent:alice-host.test/gateway"
 right: "agent:alice-host.test/alice"
test redeem_pins_principal_matching_send_from_for_same_identity ... FAILED
```

Pinned principal ends in `/gateway` as predicted — red for the right
reason.

**Control** (`cargo test -p famp --test pair_cli pair_errors_avoid_jargon`):
both `pair_errors_avoid_jargon` and its sanity-check sibling passed while
the invariant test was red, proving the red was specific to this defect
and not an environment failure.

## Task 2 — `--as` on `famp pair redeem`

- `redeem.rs`: added a required `#[arg(long = "as")] as_identity: String`
  to `PairRedeemArgs`. Replaced the hardcoded `format!("agent:{own_domain}/gateway")`
  with `resolve_as_principal(&args.as_identity, &own_domain)`, implementing
  D2: a bare leaf (e.g. `alice`) combines with the resolved own-domain; a
  full `agent:<authority>/<name>` principal is accepted only if its
  authority equals own-domain, else a `CliError::Generic` naming both
  values.
- `invite.rs`: the printed redeem command now reads
  `famp pair redeem --from <url> --as <your-agent-name>` (D4 marked
  placeholder — the inviter cannot know the follower's chosen name).
- Task 1's test now passes with `as_identity: "alice"` added to its
  `PairRedeemArgs`; the other three `PairRedeemArgs` construction sites
  in `pair_cli.rs` and the process-level test needed `--as` added too to
  keep compiling (`gateway`/`redeemer`/`redeemer2` — chosen to preserve
  each test's existing assertions unchanged).

**Verify:** `cargo test -p famp --test pair_cli` — 16/16 passed.
`cargo test -p famp-gateway --test pairing_e2e` — 5/5 passed, including
`redeem_argv_refuses_positional_code` (PAIR-06 red path unaffected —
clap still rejects the code-shaped positional before any required-flag
check).

## Task 3 — `pin()` save-before-validate ordering

`status.rs`'s `pin()` previously called `save_to_file` directly on
`keyring_path`, then reload-validated the result — a pin that failed
validation had already overwritten the last-good keyring on disk with
one `Keyring::load_from_file` might refuse to load, bricking the gateway
at next start. Fixed by writing to a same-directory `<path>.tmp-pin`
sibling, reload-validating from that temp file, and `std::fs::rename`-ing
it over `keyring_path` only on a validated reload; a failed validation
removes the temp file and leaves `keyring_path` untouched. No signature
change — `pin()`'s parameters and return type are unchanged, so no scope
expansion was needed.

**Verify:** `cargo test -p famp --test pair_cli` — 16/16 passed (including
`status_observe_before_pin_keyring_unchanged_at_write_time`, which still
proves the pin lands and PAIR-07 ordering holds).

## Commands run (final)

```
cargo test -p famp --test pair_cli        # 16 passed; 0 failed
cargo test -p famp-gateway --test pairing_e2e   # 5 passed; 0 failed
just lint                                  # cargo clippy --workspace --all-targets -- -D warnings: clean
cargo fmt -p famp / cargo fmt -p famp (per commit)
```

Did not run `cargo nextest` or bare `cargo test --workspace` per this
repo's known hang.

## Deviations from Plan

**1. [Rule 3 — blocking compile issue] Updated `crates/famp-gateway/tests/pairing_e2e.rs`**
- **Found during:** Task 2
- **Issue:** This file constructs `redeem::PairRedeemArgs` directly
  (`happy_path_pins_both_sides_mutually`) and was not in the plan's
  `files_modified` list, but adding the required `as_identity` field to
  the struct broke its compilation.
- **Fix:** Added `as_identity: "gateway".to_string()`, matching the
  test's existing `own_domain` ("redeemer.test") + expected principal
  (`agent:redeemer.test/gateway`) assertion further down, so no other
  assertion in that test needed to change.
- **Files modified:** `crates/famp-gateway/tests/pairing_e2e.rs`
- **Commit:** 2ff774d

No other deviations. `docs/FOLLOWER-SETUP.md` was not touched.

## Out of scope — confirmed untouched

- `docs/FOLLOWER-SETUP.md` — parallel audit; `git log -1 -- docs/FOLLOWER-SETUP.md`
  still shows `aaac461`, no new commit against it.
- Gateway keyring hot-reload — unchanged, tracked separately per the plan.

## Self-Check

- FOUND: crates/famp/src/cli/pair/redeem.rs (modified, `resolve_as_principal` present)
- FOUND: crates/famp/src/cli/pair/invite.rs (modified, `--as <your-agent-name>` present)
- FOUND: crates/famp/src/cli/pair/status.rs (modified, `.tmp-pin` rename present)
- FOUND: crates/famp/tests/pair_cli.rs (invariant test present, green)
- FOUND: crates/famp-gateway/tests/pairing_e2e.rs (compile fix present)
- FOUND commit 733410f, 2ff774d, f092bd5 in `git log --oneline`

## Self-Check: PASSED
