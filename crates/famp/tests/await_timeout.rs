// Phase 02 plan 02-06 (CLI-05): the v0.8 `inbox.jsonl` polling timeout
// path is gone — `famp await` now returns `BusReply::AwaitTimeout {}`
// from the broker, surfaced to the CLI as `AwaitOutcome { timed_out:
// true, .. }` and printed as `{"timeout":true}`. Plan 02-12 owns the
// replacement integration test against a real broker subprocess; this
// file is reduced to a placeholder to keep the crate compiling through
// wave 4.

#![allow(unused_crate_dependencies)]
