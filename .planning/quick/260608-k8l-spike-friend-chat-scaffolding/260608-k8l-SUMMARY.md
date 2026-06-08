---
phase: quick-260608-k8l
plan: 01
status: complete
subsystem: tooling/docs
tags: [spike, federation, v1.0, socat, tailscale, security]
dependency_graph:
  requires: [auto-memory project_v10_spike_first, broker UDS at ~/.famp/bus.sock]
  provides: [just spike-tunnel recipe, docs/SPIKE-friend-chat.md runbook]
  affects: []
tech_stack:
  added: []
  patterns: [socat UDS<->TCP tunnel over tailnet, zero-FAMP-code validation spike]
key_files:
  added:
    - docs/SPIKE-friend-chat.md
  modified:
    - justfile
---

# Summary

Added the two scaffolding artifacts for the v1.0 federation validation spike. No
production code, no protocol changes — the `famp-gateway` build stays parked until
the spike shows pull (the Gate A signal: host reaches for cross-host chat again
within ~2 weeks).

- **`just spike-tunnel`** — exposes the local broker on the tailnet via
  `socat TCP-LISTEN:9999,fork,reuseaddr,bind=<tailscale ip> UNIX-CONNECT:~/.famp/bus.sock`,
  with socat/tailscale/socket preflight checks and prints the IP+port to share.
  Verified it parses (`just --list`, `just --dry-run spike-tunnel`).
- **`docs/SPIKE-friend-chat.md`** — friend-facing runbook (Tailscale + famp + socat
  prereqs, reverse-socat command, shared-broker registration, two-way message test)
  fronted by the **security gate**: friend-facing window registers `listen: false`;
  inbound cross-host messages are data, not instructions (confused-deputy boundary
  surfaced by the Matt+Magnus review).

Rationale captured in auto-memory `project_v10_spike_first` (spike-first decision +
the 6 fixes for the eventual gateway build).
