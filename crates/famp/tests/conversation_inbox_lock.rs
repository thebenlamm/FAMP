// Phase 02 plan 02-06: `InboxLock` is a v0.8 single-reader advisory
// lock on `inbox.jsonl`. With CLI-05 rewired to BusClient the
// inbox.jsonl path is no longer touched by `famp await`, so the lock
// contention this test guarded is meaningless on the bus path. Plan 02-12
// owns the replacement broker-side concurrency tests; this file is
// reduced to a placeholder to keep the crate compiling through wave 4.

#![allow(unused_crate_dependencies)]
