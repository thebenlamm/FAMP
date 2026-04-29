// Phase 02 plan 02-06: the FSM-advance side-effects this test guarded
// (REQUESTED → COMMITTED on commit-class envelopes, COMMITTED →
// COMPLETED on terminal deliver) moved out of `await_cmd::run_at` and
// into the broker. The CLI-side await is now a single-shot bus round-trip
// with no taskdir interaction. Replacement coverage lives in the broker
// property tests (Phase 1, 02-02 follow-ups). This file is reduced to a
// placeholder to keep the crate compiling through wave 4.

#![allow(unused_crate_dependencies)]
