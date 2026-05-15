# Quick Task 260515-kqx: Implement Option 3 batch AwaitOk delivery to fix burst message loss

**Date:** 2026-05-15
**Status:** In progress

## Goal

Change `BusReply::AwaitOk` from one envelope to a capped batch so messages queued while an agent is not parked are delivered on the next `Await` call instead of being skipped.

## Tasks

1. Add the broker regression test first: a 3-agent burst to a channel while a fourth joined agent is composing; the fourth agent's next `Await` must return all three envelopes in one `AwaitOk`.
2. Update the bus protocol and broker state machine:
   - `AwaitOk { envelopes, mailbox, next_offset }`
   - canonical-session mailbox offsets
   - immediate backlog drain before parking
   - capped wake batches after mailbox append intent
3. Update all consumers and docs:
   - CLI `famp await`
   - `famp wait-reply`
   - MCP `famp_await` shape and descriptor
   - listen-mode hook wake string for singular vs. plural batches
   - backlog note for future `famp inspect waiters`

## Verification

- Red test before implementation: targeted `famp-bus` broker test fails with old protocol/behavior.
- Green after implementation: targeted broker test, bus tests touching AwaitOk, MCP/CLI affected tests.
- Run `just install` after `server.rs` changes so `~/.cargo/bin/famp` is updated.
