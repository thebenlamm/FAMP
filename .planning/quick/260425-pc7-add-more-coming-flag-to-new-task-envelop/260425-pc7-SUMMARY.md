---
quick_id: 260425-pc7
description: "Add scope.more_coming flag to new_task envelopes (T2.1)"
status: complete
shipped: 2026-04-25
commits: ["0c00ade", "937c34a", "2f71fda", "2a386ba", "756208d", "70009d8"]
---

# Quick 260425-pc7 — Summary

## What shipped

`scope.more_coming: true|false` JSON convention on FAMP `request`
envelopes — sender signal "I'm not done briefing — wait for follow-up
`deliver`s before treating this task as ready to commit." Mirrors the
existing `body.interim` shape on `deliver` envelopes.

Addresses **Gap G4 (orchestrator starvation)** — the agent-a starvation
during this morning's Lampert deck cycle.

## Atomic commits

| # | Hash | Subject |
|---|------|---------|
| 1 | `0c00ade` | test: add RED round-trip test for `scope.more_coming` |
| 2 | `937c34a` | feat: add `REQUEST_SCOPE_MORE_COMING_KEY` constant (turns RED→GREEN) |
| 3 | `2f71fda` | feat: wire `--more-coming` through `famp send` CLI |
| 4 | `2a386ba` | feat: expose `more_coming` in `famp_send` MCP tool |
| 5 | `756208d` | feat: surface `more_coming` in `famp inbox list` output |
| 6 | `70009d8` | style: clippy fixups (`too_long_first_doc_paragraph`, `equatable_if_let`) |

## Implementation choice

`RequestBody.scope` is `serde_json::Value` (freeform), so `more_coming`
is a JSON-level convention inside the scope map — mirroring the existing
`scope.instructions` (ADR 0001) rather than promoting it to a Rust
struct field. This matches the user's spec literally
(`scope.more_coming`) and preserves byte-exact backwards compat:

- **Default false → key omitted entirely.** Existing signed envelopes
  continue to verify byte-for-byte under `verify_strict`.
- Sender helper inserts `"more_coming": true` only when the caller
  explicitly opts in.
- Receiver reads via
  `scope.pointer("/more_coming").and_then(Value::as_bool).unwrap_or(false)`.

## Surfaces

- **Constant**: `crates/famp-envelope/src/body/request.rs::REQUEST_SCOPE_MORE_COMING_KEY`
- **CLI**: `famp send --new-task --more-coming` (clap-required `new_task`)
- **MCP**: `famp_send` tool's `more_coming` boolean (new_task mode only;
  silently ignored elsewhere — MCP can't lean on clap's `requires`)
- **Inbox observability**: `famp inbox list` hoists the flag to a
  top-level `more_coming: <bool>` JSON field on each entry

## Verification

```sh
cargo test --workspace                                          # all green
cargo clippy --workspace --all-targets -- -D warnings           # clean
cargo test -p famp-envelope --test scope_more_coming_round_trip # 2/2
scripts/redeploy-listeners.sh                                   # 8/8 daemons
famp send --help | grep more-coming                             # visible
```

## Out of scope (intentionally deferred)

- Receiver-side auto-commit logic gating — only meaningful AFTER the
  flag is observable in the wild for a few cycles. The flag is now
  observable; the gating change is a follow-up task.
- Backwards-incompat envelope changes — explicitly forbidden.
- Format-drift on adjacent code.

## must_haves (post-ship verification)

- [x] Existing signed envelopes (no `more_coming` key) still
      decode + `verify_strict` cleanly — proven by
      `more_coming_default_false_is_byte_exact_with_legacy`
- [x] New envelopes with `more_coming: true` round-trip with the
      flag set — proven by `more_coming_true_round_trips`
- [x] `famp send --new-task --more-coming` accepts the flag and
      `requires=new_task` gates it correctly
- [x] `famp_send` MCP tool accepts `more_coming` in `new_task` mode
- [x] `famp inbox list` exposes `more_coming` as top-level JSON
- [x] `cargo test --workspace` green
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [x] All 8 daemons restarted on the new binary
