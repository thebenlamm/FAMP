//! TEST-02 CI-gate regression guard (Phase 10 Plan 02).
//!
//! `e2e_cross_host_delivery.rs` (Phase 9, D-03) is the live two-process
//! signed cross-host E2E that satisfies TEST-02: "A live two-process
//! end-to-end test exercises the full signed cross-host task cycle and
//! runs in `just ci`." The research CRUX for this phase empirically
//! confirmed it already runs green under `cargo nextest run --workspace`
//! (= `just test` = what `just ci` invokes) — 969/969, the E2E itself in
//! 9.3s. No new E2E and no nextest test-group are needed (D-03/D-04).
//!
//! This file does NOT modify that E2E. It only pins two already-true
//! properties of it, by reading its own source text and asserting on the
//! content, so a future edit cannot silently regress either:
//!
//! 1. **Presence/enablement guard** (TEST-02, D-04) — fails if the E2E
//!    test fn is deleted/renamed, or if it is marked with Rust's ignore
//!    test attribute (which would pull it out of the default
//!    `cargo nextest run --workspace` set and put TEST-02 back "behind a
//!    manual or ignored path" — exactly what D-04 forbids).
//! 2. **Hermetic / CI-safety guard** (D-05) — fails if the E2E stops
//!    `ChildGuard`-reaping its spawned children, stops binding an
//!    ephemeral `127.0.0.1:0` port, stops isolating `FAMP_HOME`/
//!    `--socket` per process inside a tempdir, or stops using the
//!    `cross_machine` fixture certs — any of which would make the E2E
//!    leak processes, race other CI tests, or depend on a developer's
//!    live `~/.famp/` daemon.

#![allow(unused_crate_dependencies)]

use std::path::PathBuf;

/// The exact `#[test]` fn name of the Phase 9 E2E this guard protects.
const E2E_TEST_FN: &str = "gw01_gw02_gw03_two_process_cross_host_delivery";

/// Read `tests/e2e_cross_host_delivery.rs`'s source as a `String`, from
/// this same crate (`CARGO_MANIFEST_DIR` = `crates/famp-gateway`), so no
/// cross-package binary/path resolution is needed.
fn e2e_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("e2e_cross_host_delivery.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "TEST-02 CI-gate guard: could not read the guarded E2E at {}: {e}. \
             The signed cross-host E2E (e2e_cross_host_delivery.rs) must exist \
             and stay in `crates/famp-gateway/tests/` for TEST-02 to hold.",
            path.display()
        )
    })
}

/// The Rust "ignore this test" attribute, built at runtime (not embedded
/// as a literal `#[ignore]` token in this file's own source) so a
/// future repo-wide grep for that exact bracketed attribute does not
/// false-trip on this guard file itself.
fn ignore_attribute_needle() -> String {
    format!("#{o}ignore{c}", o = '[', c = ']')
}

#[test]
fn e2e_cross_host_delivery_is_present_and_not_ignored() {
    let source = e2e_source();

    assert!(
        !source.trim().is_empty(),
        "TEST-02 CI-gate guard: e2e_cross_host_delivery.rs exists but is empty. \
         TEST-02 requires the live two-process signed cross-host E2E to run on \
         every `just ci` — restore its content."
    );

    assert!(
        source.contains(E2E_TEST_FN),
        "TEST-02 CI-gate guard: e2e_cross_host_delivery.rs no longer defines \
         `{E2E_TEST_FN}`. TEST-02 requires this exact E2E to stay in the default \
         `cargo nextest run --workspace` set — do not delete or rename it away; \
         if it was intentionally renamed, update this guard's E2E_TEST_FN \
         constant in the same commit."
    );

    let ignore_needle = ignore_attribute_needle();
    assert!(
        !source.contains(&ignore_needle),
        "TEST-02 CI-gate guard: e2e_cross_host_delivery.rs carries the ignore \
         test attribute. Per D-04, TEST-02 must NEVER be satisfied via \
         `#[ignore]` or a manual recipe — the E2E must run on every default \
         `cargo nextest run --workspace` (= `just ci`) invocation. Remove the \
         ignore attribute; if the test is genuinely broken, fix it rather than \
         disabling it."
    );
}

#[test]
fn e2e_cross_host_delivery_stays_hermetic_and_ci_safe() {
    let source = e2e_source();

    assert!(
        source.contains("ChildGuard") && source.contains("child_guard"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer references \
         ChildGuard / the child_guard module. Every spawned broker/gateway/\
         register child MUST be RAII-reaped via ChildGuard (project ChildGuard \
         convention) so a panicking run doesn't leak processes or respawn \
         tmp-socket brokers. Fix the E2E, do not weaken this guard."
    );

    assert!(
        source.contains("127.0.0.1:0"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer binds an \
         ephemeral `127.0.0.1:0` port. The E2E must use OS-assigned ephemeral \
         ports, never a fixed listen port, so it can run unattended and in \
         parallel in CI without colliding. Fix the E2E, do not weaken this \
         guard."
    );

    assert!(
        source.contains("FAMP_HOME") && source.contains("TempDir"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer isolates \
         FAMP_HOME inside a tempfile::TempDir. Each side's broker/gateway \
         process MUST run with its own tempdir-scoped FAMP_HOME — never a \
         developer's real `~/.famp/` — so the E2E is CI-safe and stays \
         hermetic. Fix the E2E, do not weaken this guard."
    );

    assert!(
        source.contains("--socket"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer passes an \
         isolated `--socket` per side. Each broker/gateway process MUST bind \
         its own tempdir-scoped bus socket — never a developer's real bus — so \
         the E2E is CI-safe and stays hermetic. Fix the E2E, do not weaken \
         this guard."
    );

    assert!(
        source.contains("cross_machine") && source.contains(".crt") && source.contains(".key"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer references \
         the `crates/famp/tests/fixtures/cross_machine/` fixture cert pair \
         (alice/bob .crt/.key). The E2E MUST use the committed fixture certs, \
         not developer-machine or self-generated certs, so it is reproducible \
         and CI-safe. Fix the E2E, do not weaken this guard."
    );

    let daemon_default_needle = format!("{home}{path}", home = "~", path = "/.famp/bus.sock");
    assert!(
        !source.contains(&daemon_default_needle),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs references the \
         developer daemon's default bus socket path. The E2E MUST NEVER rely \
         on a developer's live `~/.famp/` daemon — it must stay fully \
         isolated per side via tempdir-scoped FAMP_HOME/--socket. Fix the E2E, \
         do not weaken this guard."
    );
}
