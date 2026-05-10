# Phase 1: Broker Diagnosis & Identity Inspection - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-10
**Phase:** 01-broker-diagnosis-identity-inspection
**Areas discussed:** Client cwd tracking, inspect tasks/messages stubs, ORPHAN_HOLDER pid discovery, broker started_at tracking

---

## Client cwd tracking

| Option | Description | Selected |
|--------|-------------|----------|
| A: Register wire extension | Add `cwd: Option<String>` to `Register { name, pid }` with `#[serde(default)]`. Client sends its own cwd; broker stores it. | ✓ |
| B: Server-side proc derivation | Broker reads `/proc/<pid>/cwd` (Linux) or `proc_pidinfo` (macOS) from the client's pid at register time. | |

**User's choice:** Spin up Zed and Matt to decide → both chose A unanimously.
**Notes:** Option B is racy (process may have `chdir`'d or exited), platform-forked, and fails exactly when you need cwd most (orphan/zombie holders). Capture at register time, never refresh; document that the field reflects where the agent was born.

---

## inspect tasks/messages CLI stubs

| Option | Description | Selected |
|--------|-------------|----------|
| Absent (Phase 1) | `famp inspect tasks` returns unrecognized subcommand until Phase 2 adds it. Proto variants are internal seams only. | ✓ |
| Visible stubs | Surface `famp inspect tasks` and `famp inspect messages` in Phase 1 `--help`, returning "not yet implemented". | |

**User's choice:** Spin up Zed and Matt to decide → both chose absent unanimously.
**Notes:** Stubs calcify CLI surface and flag shape before the server is designed. Users script against them; removing them later is breaking. Internal `NotYetImplemented` variants are fine as forward-compat seams — don't leak to the user-facing CLI.

---

## ORPHAN_HOLDER pid discovery

| Option | Description | Selected |
|--------|-------------|----------|
| A: lsof/ss subprocess | Shell out to `lsof -U <sock>` (macOS) or `ss -lx` / `/proc/net/unix` (Linux). Parse PID from output. | |
| B: SO_PEERCRED only | `SO_PEERCRED` (Linux) / `LOCAL_PEERPID` (macOS) socket option — zero subprocess, atomic with connection. | |
| C: SO_PEERCRED + fallback | Platform socket option first; lsof/ss fallback when socket option fails or returns 0. | ✓ |

**User's choice:** Spin up Zed and Matt to decide → both chose C unanimously.
**Notes:** SO_PEERCRED covers the connected-orphan case cleanly; lsof fallback handles the stale-inode-orphan case where nothing is accepting. Matt added: expose discovery method as `pid_source: peercred|lsof|unknown` in evidence row so operators know whether to trust the PID. Zed noted macOS requires `getsockopt(SOL_LOCAL, LOCAL_PEERPID)` returning `pid_t`, not the Linux `ucred` struct.

---

## broker started_at tracking

| Option | Description | Selected |
|--------|-------------|----------|
| A: `started_at: SystemTime` in BrokerState | Add field to struct; set `SystemTime::now()` in constructor at broker startup. | ✓ |
| B: Derive from socket file mtime | Read socket file mtime as a proxy for broker start time. | |

**User's choice:** Spin up Zed and Matt to decide → both chose A unanimously.
**Notes:** Socket mtime lies in three ways: restart-with-reused-socket reports old start time; `touch` from external tools corrupts the answer; some filesystems don't update mtime predictably. One field in the constructor is correct by construction. The broker's HEALTHY handler runs server-side and has direct BrokerState access — no need for mtime approximation.

---

## Claude's Discretion

- Concrete `cargo tree` assertion form for `check-inspect-version-aligned` — deferred to plan-phase per SPEC.
- Whether `BrokerState` moves to explicit `new()` or keeps `derive(Default)` with custom impl — both satisfy D-07; planner picks the cleanest shape.

## Deferred Ideas

None — discussion stayed within phase scope.
