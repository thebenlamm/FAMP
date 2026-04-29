// Phase 02 plans 02-04/02-06: this v0.8 test uses both
// `famp::cli::await_cmd::AwaitArgs` (rewired by 02-06) and
// `famp::cli::send::run_at` over the federation HTTPS path (rewired by
// 02-04). Both AwaitArgs and SendArgs shapes change and `run_at` now
// takes a bus socket path rather than `FAMP_HOME`. Plan 02-12 owns the
// replacement broker-driven coverage; this file is reduced to a
// placeholder to keep the crate compiling through wave 4.

#![allow(unused_crate_dependencies)]
