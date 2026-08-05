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
//! This file does NOT modify that E2E. It only pins already-true properties
//! of it, by reading source text and asserting on the content, so a future
//! edit cannot silently regress any of them.
//!
//! Scope note (Phase 11): the presence/enablement guard reads the E2E file
//! alone, since it is about that file's test fn and attributes. The hermetic
//! guard reads the E2E **plus** the `common/gateway_harness.rs` module it
//! `#[path]`-includes, because plan 04 extracted the two-host rig there — the
//! guarded properties moved with the code. A third guard pins that include so
//! the widened read stays honest.
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

/// The shared two-host rig the E2E `#[path]`-includes. Phase 11 plan 04
/// mechanically extracted `Side`/`spawn_*`/`wait_for_*` out of the E2E into
/// this harness so a second gateway E2E could reuse it. The hermetic
/// properties guarded below (ChildGuard reaping, ephemeral `127.0.0.1:0`,
/// tempdir `FAMP_HOME`/`--socket`, fixture certs) moved with that code.
const HARNESS_REL: &str = "common/gateway_harness.rs";

fn read_guarded(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "TEST-02 CI-gate guard: could not read the guarded source at {}: {e}. \
             The signed cross-host E2E (e2e_cross_host_delivery.rs) and the \
             harness it includes ({HARNESS_REL}) must both exist and stay in \
             `crates/famp-gateway/tests/` for TEST-02 to hold.",
            path.display()
        )
    })
}

/// Read only the E2E file itself. Used by the presence/enablement guard,
/// which is about THAT file's test fn and attributes — not the harness.
fn e2e_source() -> String {
    read_guarded("e2e_cross_host_delivery.rs")
}

/// The **effective** source surface the E2E compiles from: the E2E file plus
/// the harness module it `#[path]`-includes.
///
/// The hermetic guard below must read both. Before Phase 11 the rig lived
/// inline in the E2E, so grepping one file was sufficient; after plan 04's
/// extraction the same invariants live in the harness, and a file-scoped grep
/// false-trips on a refactor that weakened nothing.
///
/// This widens the guard's VIEW, it does not weaken its ASSERTIONS — every
/// property below is still required to be present, and deleting e.g.
/// `ChildGuard` from the harness still trips it. The linkage assertion in
/// `harness_is_actually_included_by_the_e2e` keeps the widening honest: the
/// harness only counts because the E2E really includes it.
fn e2e_effective_source() -> String {
    format!("{}\n{}", e2e_source(), read_guarded(HARNESS_REL))
}

/// The widened hermetic guard is only sound while the E2E genuinely compiles
/// the harness in. If that `#[path]` include is ever dropped, the harness's
/// `ChildGuard`/tempdir/ephemeral-port code stops applying to this E2E and
/// reading it would be a lie — so pin the linkage itself.
#[test]
fn harness_is_actually_included_by_the_e2e() {
    let source = e2e_source();
    assert!(
        source.contains(HARNESS_REL) && source.contains("mod gateway_harness"),
        "D-05 hermetic guard: e2e_cross_host_delivery.rs no longer \
         `#[path]`-includes `{HARNESS_REL}`. The hermetic guard reads that \
         harness as part of the E2E's effective source; without the include, \
         the harness's ChildGuard/tempdir/ephemeral-port guarantees no longer \
         apply to this E2E. Either restore the include, or move those \
         properties back inline AND narrow this guard in the same commit."
    );
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
    // Effective surface = the E2E plus the harness it `#[path]`-includes.
    // Plan 11-04 moved the rig into the harness; the properties asserted
    // below are unchanged, only where they physically live.
    let source = e2e_effective_source();

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

    assert!(
        !source.contains("TcpStream::connect") && source.contains("wait_for_https"),
        "D-05 hermetic guard (TLS readiness): the gateway E2E harness must prove listener \
         readiness with a COMPLETED trusted-TLS request, not a plaintext TcpStream connect. \
         A TCP connect succeeds as soon as the socket reaches LISTEN state — it proves \
         nothing about the rustls config loading, the TLS accept loop running, or the axum \
         router being mounted. That gap is load-bearing for a documented reason (see \
         `wait_for_https`): `run_egress`'s `Await` drain ADVANCES the mailbox read cursor \
         even when the relay POST fails, with no re-queue on error, so an envelope drained \
         before the peer can actually serve HTTPS is silently LOST rather than retried. \
         Strengthen the probe; do not weaken this guard."
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
