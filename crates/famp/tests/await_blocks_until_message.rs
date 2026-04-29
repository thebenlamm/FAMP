// Phase 02 plan 02-06 (CLI-05): `famp await` is now a single-shot bus
// round-trip via BusClient — the inbox.jsonl polling shape this test
// exercised is gone. Plan 02-12 owns the replacement integration test
// (`test_await_unblocks` against a real broker subprocess driving
// `BusMessage::Send` from another connection); this file is reduced to
// a placeholder to keep the crate compiling through wave 4.

#![allow(unused_crate_dependencies)]
