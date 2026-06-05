---
phase: 06-onboarding-cross-platform-docs
plan: 01
status: complete
requirements: [DOC-01, DOC-02, DOC-03]
---

# 06-01 Summary: Daemon-first quickstart + bridge + platform support

## What was built

The README's new getting-started spine — the D-06 three-tier block, built as one
unit so plan 02's downstream edits reconcile against a finalized block:

1. **Quick Start (daemon-first)** — now leads with `famp daemon install` as the
   one command that ends broker-babysitting. The fence enumerates every step
   (Rust → `cargo install famp` → `famp daemon install` → `famp install-claude-code`
   + `famp install-codex` → register Claude/Codex). `install-claude-code` and
   `install-codex` are kept distinct from `daemon install` (orthogonal: broker
   presence vs MCP wiring, D-01). Idempotency stated only as "re-running is safe"
   (no exit-code / CI-verified claim); must run from an unsandboxed shell, with a
   pointer to the bridge for users who can't.
2. **`### No-install bridge`** — peer first-contact choice directly under the
   quickstart (`famp broker --no-idle-exit`), with the persistence tradeoff by
   mechanism: daemon survives reboot/logout (`RunAtLoad` + `KeepAlive`); bridge
   dies on terminal-close/logout.
3. **`## Platform support`** — placed immediately after the bridge. Names covered
   (macOS launchd `com.famp.broker`; Linux systemd `--user` ≥ 240 + `enable-linger`)
   and uncovered (minimal distros, containers, WSL, headless-without-linger, any
   other platform) configs, each pointing down to the bridge. Frames the boundary
   as the enforced refuse-to-install error path (DAEMON-06), not a fragile promise.
   No "works for both" overclaim.

**Broker lifecycle** rewritten: auto-spawn kept LIVE but demoted to no-install
behavior for unsandboxed clients (works for Claude Code, cannot serve sandboxed
Codex, idle-exits after 300 s). `~/.famp/` (`bus.sock`, `broker.log`) diagnostics
preserved.

## Key files

- `README.md` (modified) — Quick Start, new `### No-install bridge`, new
  `## Platform support`, rewritten `## Broker lifecycle`.

## Verification

All plan acceptance greps pass:
- T1 PASS: `famp daemon install` / `famp broker --no-idle-exit` / `install-codex` /
  `RunAtLoad` all present.
- T2 PASS: `## Platform support` / `WSL` / `enable-linger` present; `launchd`,
  `systemd`, `container`, `minimal distro` named; bridge string occurs after the
  Platform support heading.
- Negatives clean: no `exit 37`, no `CI-verified`, no `works for both`, no
  `works on both platforms`.

Commands were quoted from the live installed binary (`famp 0.11.0`; daemon
subcommands install/uninstall/status/restart; `--no-idle-exit` present), verified
by the orchestrator before execution.

## Deviations

- The CLI table (inside Quick Start) was left unchanged — D-04 item 1 assigns the
  daemon-row additions to plan 02. Kept 06-01 surgical to the three-tier block.
- The Quick Start intro's `v0.9 local-first path` label was rewritten to a
  version-neutral daemon-first framing; the full v0.9→v0.11 refresh (header L7 +
  intro prose) remains plan 02's D-05 scope.

## Notes for downstream (plan 02)

The finalized three-tier block is in place. Plan 02 must reconcile the five D-04
sections against it: CLI table (add daemon rows + `--no-idle-exit`, fix the
`register` auto-spawn framing), "Onboarding (recommended path)", "When NOT to Use"
(integrity reword — daemon keeps the BROKER reachable, does not service work
autonomously), "Upgrading" (add `famp daemon restart`), "Troubleshooting" (add
`famp daemon status`/`restart`), plus the v0.9→v0.11 header/intro refresh.
