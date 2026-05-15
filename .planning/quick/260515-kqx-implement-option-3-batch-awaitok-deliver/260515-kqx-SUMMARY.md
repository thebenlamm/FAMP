---
status: complete
quick_id: 260515-kqx
date: 2026-05-15
commit: 146ca9f
---

# Quick Task 260515-kqx: Batch AwaitOk Delivery - Summary

Implemented batch `AwaitOk` delivery for `famp await` / `famp_await` so queued burst messages are delivered together instead of waiting for a later wake message.

## Changes

- Changed `BusReply::AwaitOk` to carry `envelopes`, `mailbox`, and `next_offset`.
- Added canonical-session await offsets so short-lived proxy awaits do not replay from zero.
- Made `Await` drain the first mailbox with backlog before parking, capped to 50 envelopes.
- Updated direct/channel waiter wake paths to batch queued mailbox entries after the append intent.
- Updated CLI, MCP, wait-reply, listen-mode hook, and affected tests for the batch shape.
- Added backlog note for future `famp inspect waiters`.

## Verification

- Red test first: `cargo test -p famp-bus test_channel_burst_while_not_parked_batches_on_next_await` failed against the old `AwaitOk { envelope }` shape.
- `cargo test -p famp-bus`
- `cargo test -p famp --test mcp_bus_e2e`
- `cargo test -p famp --test wait_reply`
- `cargo test -p famp --test hook_runner_await --test install_claude_code`
- `bash -n crates/famp/assets/famp-await.sh`
- `cargo clippy -p famp-bus -p famp --all-targets -- -D warnings`
- `just install`
