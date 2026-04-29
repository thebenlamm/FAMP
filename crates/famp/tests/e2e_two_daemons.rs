// Phase 02 plans 02-04/02-06: this Phase-4 E2E drives the full federation
// `famp listen` HTTPS path with both `famp send` (CLI-02) and
// `famp await` (CLI-05). Both CLIs are rewired to BusClient in wave 4 —
// `AwaitArgs` and `AwaitOutcome` shapes change and the daemon-pair flow
// is replaced by a single broker. Plan 02-12 owns the broker-driven
// replacement coverage; the cross-host E2E itself is v1.0 federation
// territory per ARCHITECTURE.md.

#![allow(unused_crate_dependencies)]
