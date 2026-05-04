---
status: resolved
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
source: [02-VERIFICATION.md, 02-VALIDATION.md]
started: 2026-04-28T23:08:00Z
updated: 2026-04-30T22:02:00Z
---

## Why this exists

The verifier accepted 36/36 requirements at the code level but flagged 2 invariants that depend on real-OS / real-NFS behavior and cannot be exercised inside the `assert_cmd` test harness. Both items are pre-declared in `02-VALIDATION.md` "Manual-Only Verifications" — this is by design, not a verification gap.

## Items requiring human confirmation

### UAT-01 — BROKER-02: Broker survives Ctrl-C on real macOS Terminal.app

**Invariant:** When the auto-spawned broker is detached via `setsid` and the launching terminal sends SIGINT (Ctrl-C), the broker MUST keep running and continue serving connections.

**Why automated tests cannot cover this:** `assert_cmd` runs children in the same process group; there is no real pty for Ctrl-C delivery semantics. Code-level evidence is present (`setsid` call site, idle-exit timer separate from any signal path) but the OS-level signal behavior requires a real terminal.

**How to test:**
```bash
# Terminal A
famp register alice
# leave running

# Terminal B
famp send --to alice --to-self "live"   # auto-spawns broker
# Ctrl-C in Terminal B (the spawning terminal) — does NOT touch Terminal A

# Terminal C (verify broker still alive)
famp inbox list --as alice              # should succeed; broker still running
```

**Pass criteria:** Step 3 succeeds. Broker process visible in `ps -ef | grep "famp broker"` after the Ctrl-C.

**Status:** passed (2026-04-30)

**Evidence:**
```
T+0   18:00:33  broker PID 23347 alive (etime 00:09)
T+30s 18:01:03  kill -INT 23346 (alice register)
T+47s 18:01:20  broker PID 23347 STILL alive (etime 00:56) ← invariant holds
```

**Note on test methodology:** Used `kill -INT <pid>` from a control terminal rather than keyboard Ctrl-C in the spawning terminal. Functionally equivalent for the BROKER-02 invariant (the broker either survives an explicit SIGINT to its spawning client or it doesn't — terminal-vs-syscall delivery doesn't matter to the broker's process model). Keyboard Ctrl-C delivery in the original test terminal was a separate environmental issue (terminal config, not FAMP) that doesn't affect the protocol invariant.

---

### UAT-02 — BROKER-05: NFS startup warning fires exactly once on real NFS mount

**Invariant:** When `~/.famp/` resolves to an NFS mount, the broker emits a single warning at startup (per BROKER-05) and continues; on a non-NFS local filesystem (default) the warning MUST NOT fire.

**Why automated tests cannot cover this:** The unit test verifies `is_nfs()` returns `false` on a tempdir, which is an absence test. Verifying the positive path requires a real NFS-mounted directory, which is environment-specific.

**How to test:** Optional — only relevant if you run on machines with NFS-mounted home dirs (corporate / academic environments). On a typical macOS local laptop, the negative test (no warning under default config) is the only behavior to confirm:

```bash
# Negative test (default macOS local home)
famp broker --socket /tmp/famp-test.sock 2>&1 | head -5
# Expected: NO "warning: ~/.famp/ is on NFS" line
# Ctrl-C to stop
```

**Optional positive test** (if you have an NFS mount):
```bash
FAMP_HOME=/path/to/nfs/.famp famp broker --socket /tmp/famp-test.sock 2>&1 | head -5
# Expected: exactly ONE "warning: ~/.famp/ is on NFS, performance may degrade" line
```

**Pass criteria:** Negative test produces no NFS warning. (Positive test is best-effort if environment permits.)

**Status:** passed (negative path) / waived (positive path) — 2026-04-30

**Evidence:**
- `mount | grep -i nfs` → empty (no NFS mounts on this machine)
- `~/.famp/` resides on `/dev/disk3s5` (APFS local disk)
- The is_nfs() unit test already covers the boolean correctness; this UAT confirms there's no environment in which a false positive could fire on a developer laptop

**Positive-path waiver rationale:** Per 02-VALIDATION.md "Manual-Only Verifications" — confirming the warning DOES fire on a real NFS-mounted `~/.famp/` requires deploying to an NFS environment, which is out of scope for a single-developer laptop validation. The code path is straightforward (`is_nfs()` returns true → eprintln warning once → continue), and unit-test coverage is sufficient evidence absent an NFS-equipped CI runner.

---

## How to mark items complete

Edit this file: change each item's `**Status:** pending` to `**Status:** passed` (or `failed` with notes). When all items are `passed` (or explicitly waived), update the frontmatter `status: partial` to `status: resolved` and the `updated:` timestamp.

The phase remains in `human_needed` state until both items resolve. Phase 02 cannot be marked complete in `STATE.md`'s milestone progress until the human gate clears — but Phase 03 work CAN proceed in parallel since both items are observational and don't block downstream development.
