# Phase 5: Daemon Service Management & Version Safety - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-04
**Phase:** 5-daemon-service-management-version-safety
**Areas discussed:** Version compatibility policy, Version source of truth, Linux persistence UX

**Method:** User declined the default research flow and asked for a 2-agent panel
(matt-essentialist + magnus-fixer) on all three gray areas. Both agents returned
decisive, fully converging recommendations. User locked all three.

---

## A. Version compatibility policy (VER-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Protocol-version-only | Refuse only on `bus_proto` integer mismatch (real wire break); build skew logged, not refused | ✓ |
| Exact build-version match | Refuse on any version mismatch | |
| Semver-major | Refuse only on major bumps | |

**User's choice:** Protocol-version-only (panel-recommended).
**Notes:** Both agents named exact-build-match the single highest-regret mistake —
`KeepAlive=true` means a new client meets the old daemon on every connect after
`cargo install` until restart, so build-strictness walls every routine upgrade.
Refusal error MUST name `famp daemon restart`. Handshake logs both versions
non-fatally. Client is the refusing party.

---

## B. Version source of truth (VER-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Unify on `CARGO_PKG_VERSION` | One number for -V/banner/handshake | |
| Separate protocol constant | `bus_proto` = wire authority + one honest display version | ✓ |

**Display version sub-decision:**

| Option | Description | Selected |
|--------|-------------|----------|
| 0.11.0 (milestone-aligned) | -V/banner track the git-tagged milestone | ✓ |
| 0.5.2 (spec-aligned) | -V/banner track the FAMP spec version | |
| Decide at planning | Lock principle, defer number | |

**User's choice:** Separate protocol constant; display version = **0.11.0**.
**Notes:** `BUS_PROTO_VERSION: u32 = 1` already exists and is already on the wire
— this decision was partly confirmation of the de-facto architecture. Stays = 1
(no wire change). Needs a doc comment: bump only on wire change, never auto-wired
to `CARGO_PKG_VERSION`. Three distinct axes kept separate: display version
(0.11.0), `bus_proto` (1), `FAMP_SPEC_VERSION` (0.5.2, federation).

---

## C. Linux persistence UX (DAEMON-06)

| Option | Description | Selected |
|--------|-------------|----------|
| Detect-and-instruct | Print exact `loginctl enable-linger <user>`, proceed, don't run it | ✓ |
| Proactively enable linger | install runs `loginctl enable-linger` for the user | |

**User's choice:** Detect-and-instruct (panel-recommended).
**Notes:** Proactive linger = silent privilege escalation (processes persist with
no active session) — banned on some managed hosts, and asymmetric with the locked
macOS LaunchAgent. `famp daemon status` must also report linger state so the loop
closes. systemd-absent path already locked by DAEMON-06 acceptance (fail + manual
fallback).

## Claude's Discretion

- `launchctl restart` invocation (`kickstart -k` vs `bootout`+`bootstrap`).
- Version-exchange wire placement (extend existing `Hello`/`HelloOk` vs new frame).
- BOOT-02 sandbox refusal reuses the Phase 4 `SandboxEperm` probe.

## Deferred Ideas

- Two-brokers-bind-same-socket steal race (magnus-fixer) — NOT in Phase 5;
  ROADMAP already defers the `bind_exclusive` spawn-lock to its own track. Full
  failure analysis recorded in CONTEXT.md `<deferred>` for that future track.
- Planning note (matt-essentialist): grep for hardcoded `0.1`/`0.5` version
  assertions before the `-V` → `0.11.0` bump.
