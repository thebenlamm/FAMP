// Phase 02 plans 02-04/02-06: this v0.8 conversation test drives both
// `famp send` and `famp await` over the federation HTTPS path. With both
// CLIs rewired to BusClient (CLI-02, CLI-05) the test infrastructure is
// gone — there's no `famp listen` daemon on the bus path. Plan 02-12
// owns the replacement integration tests against a real broker; this
// file is reduced to a placeholder to keep the crate compiling through
// wave 4.

#![allow(unused_crate_dependencies)]
