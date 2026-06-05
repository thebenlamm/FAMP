---
phase: 06-onboarding-cross-platform-docs
plan: 02
status: complete
requirements: [DOC-01, DOC-03]
---

# 06-02 Summary: Reconcile five downstream sections + v0.11 refresh

## What was built

Reconciled every downstream README reference against plan 01's finalized
three-tier block (D-04), plus the v0.9→v0.11 version-framing refresh (D-05), so
no section still contradicts the daemon-first lead.

### Task 1 — CLI table + Onboarding path + version refresh
- **CLI table:** added the four `famp daemon` lifecycle rows
  (install / status / restart / uninstall) and the `famp broker --no-idle-exit`
  bridge row. Reworded the `register` row off the universal-auto-spawn overclaim
  ("start the broker if needed") to the demoted-but-live framing.
- **Onboarding (recommended path):** step 1 now installs the broker service first
  and points back to Quick Start, instead of leading with a bare
  `install-claude-code` (the second hidden quickstart that contradicted D-01).
- **Version refresh (lead header + intro only):** `v0.9 Local-First Bus` →
  `v0.11 Broker Daemon`; blockquote names the v0.11 broker daemon and corrects
  the stale `0.1.0` crate-version note to the unified `0.11.0` (VER-02); intro
  prose reads "v0.11 runtime path" and "Local (v0.11)". The v0.5.1 protocol-spec
  framing and the v1.0 federation framing are untouched; historical milestone
  sections ("Not Shipped Yet", "Current Milestones") left as-is per the D-05
  boundary.

### Task 2 — Integrity reword + Upgrading + Troubleshooting
- **"When NOT to Use FAMP" (INTEGRITY TRAP):** replaced "There is no autonomous
  daemon servicing scheduled work" (which now collides with `famp daemon
  install`) with the reword that preserves the load-bearing boundary — the daemon
  restores broker *presence*, not agent *attendance*; delivery still requires an
  open agent window. The "survives a closed laptop" rule of thumb and the "needs
  a real backend" list (incl. "while no one is at a keyboard") survive verbatim.
  No background/production/unattended overclaim introduced.
- **Upgrading:** added `famp daemon restart` after `cargo install` + a VER-01 note
  (a client hitting a not-yet-restarted long-lived daemon gets a ProtocolMismatch
  error naming the restart remedy).
- **Troubleshooting:** "stuck after a binary upgrade" now also restarts the
  daemon; new "is the broker up?" bullet runs `famp daemon status`.

## Key files

- `README.md` (modified) — header/intro version refresh, CLI table, Onboarding
  step 1, "When NOT to Use" paragraph, Upgrading, Troubleshooting.

## Verification

All plan acceptance greps pass:
- T1 PASS: four daemon subcommands + `--no-idle-exit` in the CLI table; register
  overclaim string removed; Onboarding references `famp daemon install`; header no
  longer `v0.9 Local-First Bus`; `0.11.0` present, stale `0.1.0` gone.
- T2 PASS: reword line present; old colliding line gone; "survives a closed
  laptop" + "while no one is at a keyboard" preserved; `famp daemon restart` in
  Upgrading; `famp daemon status` + `famp daemon restart` in Troubleshooting.

Commands quoted from the live binary (verified by the orchestrator: `famp 0.11.0`;
daemon install/uninstall/status/restart; `--no-idle-exit`).

## Deviations

- Version refresh deliberately bounded to the lead header + intro lines (25, 28)
  per D-05; the "Not Shipped Yet" `v0.9 — Local-First Bus (shipping now)` framing
  and the "Current Milestones" list were left untouched (out of D-05 scope). Plan
  03's accuracy gate may flag the "shipping now" label as stale — noted, but not
  changed here to honor the plan's surgical scope.

## Notes for downstream (plan 03)

The README is now internally consistent and daemon-first. Plan 03 is the hard
accuracy gate: Task 1 (auto) greps every side-effect-free command quote against
the live binary; Task 2 (human-verify, BLOCKING) drives the live launchctl
lifecycle + fresh-clone Claude+Codex E2E.
