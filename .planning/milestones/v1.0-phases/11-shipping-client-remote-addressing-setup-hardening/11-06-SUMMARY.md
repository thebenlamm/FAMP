---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 06
status: complete
requirement: UAT-01
gate: blocking-human
verdict: PASS
completed: 2026-07-29T02:05:00Z
tags: [uat, dogfood, two-machine, federation, gateway, v1.0.0-rc.1]

# Dependency graph
requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "plan 03's remote-addressed `famp send`; plan 02's single-source own-domain; plan 07's broker from-binding + egress own-domain check + ready-line-after-init; plan 08's ingress destination validation and gateway-owned federation fields; plan 05's corrected GATEWAY-SETUP.md; plan 04's cert recipe"
provides:
  - "11-HUMAN-UAT.md — the live two-machine dogfood record and PASS verdict"
  - "empirical proof that a shipping FAMP client can address a remote principal cross-host and drive the task FSM to a terminal state on both hosts"
affects: [v1.0.0-rc.1 tagging]

key-files:
  created:
    - .planning/phases/11-shipping-client-remote-addressing-setup-hardening/11-HUMAN-UAT.md
  modified: []
---

# Plan 11-06 — UAT-01 Two-Machine Dogfood

## Outcome

**PASS.** The final human gate of phase 11 is satisfied. Task
`019fab97-d3e0-7d63-92ba-39f1ce171b83` reached **COMPLETED on both machines**,
opened by the real shipping `famp send` with no hand-written injector.

This closes the v1.0 blocker "no shipping client can address a remote principal"
(`project_v10_no_shipping_client_addresses_remote`). The wire was already proven
in Gate A; what was missing was a driveable client. It is now driveable.

Full evidence — topology, binary hashes, key fingerprints, every envelope, both
FSM views — is in `11-HUMAN-UAT.md`. Not duplicated here.

## Gate handling

The plan marks Task 2 `gate="blocking-human"` with an explicit "do not
auto-approve" prohibition, and the orchestrator was running under `--auto`.
The gate was **not** auto-approved: execution halted, the gate was presented to
Ben with its cost and blast radius, and he explicitly directed the run. That is
a human decision, not an automated one.

## Task 1 — binary freshness

`just install-all` on the Mac; the equivalent raw `cargo install --path …
--locked --force` pair on devbox (`just` is not installed there). Both hosts
built from source commit `0184f01`. SHA-256 of all four binaries recorded per
host in the UAT doc, satisfying the plan's "record hash + commit on BOTH hosts"
requirement (added from the review MEDIUM about stale binaries).

Code reached devbox over the tailnet via `git bundle`. **Nothing was pushed to
`origin`** — a dogfood does not require publishing, and no push was requested.

## Deviations

1. **Isolated `FAMP_HOME` instead of the production broker.** Ran both hosts
   under `~/famp-uat11` with a dedicated bus socket. Ben's live mesh (broker pid
   743 + three `famp mcp` sessions) was running on the Mac and installing fresh
   binaries requires a broker restart, which would have dropped those holders.
   Same deployed binaries, same real network path, smaller blast radius. The
   production broker was never restarted.

2. **Gateway A run from `target/release/famp-gateway`.** The macOS firewall was
   enabled, `sudo` needed an unavailable interactive password, and the existing
   allow-rule covered the `target/release` path but not `~/.cargo/bin`. Confirmed
   the two binaries are **byte-identical** (`4544499d…`) and ran the
   already-authorized path — so this is the deployed binary, and freshness holds.
   Inbound to the Mac was then proven empirically by B's reply legs arriving.

3. **`just ci` substituted.** The plan's Task 1 verification specifies `just ci`,
   which is unusable on this machine (cargo-nextest hangs in `--list`; a full
   `cargo test --workspace` also blew a 900s timeout). Substituted `just lint`
   plus the three targeted suites, all green. **The full workspace suite was not
   run to completion — CI remains the real gate.** Recorded as finding F-C.

## Findings

- **F-A (minor, non-blocking):** an *opening* `request` is not task-indexed until
  a threaded reply arrives — the index keys on `causality.ref`, which a task-root
  request has none of. So a pending, unanswered remote task is invisible to
  `famp inspect tasks`. Established with a two-pole control (local send indexes;
  remote opening send does not), which also refuted the initial hypothesis that
  the cause was the `audit_log`→`request` class change. Delivery, signature
  verification and the FSM are unaffected; the gate's requirement is still met.
  Follow-up candidate: index the request root on receipt.
- **F-B (informational):** macOS firewall allow-rules are path-bound, so an
  approval granted to a `target/release` build does not survive the move to
  `~/.cargo/bin`. Worth one sentence in GATEWAY-SETUP.md §1.
- **F-C (informational):** `just ci` cannot serve as a verification gate on this
  machine; future plans should name targeted substitutes.

## Validation of this phase's own doc work

`docs/GATEWAY-SETUP.md` as corrected by plan 11-05 was followed **literally** and
the setup worked on the first attempt — including the two semantic inversions
that broke the Gate A dogfood (gateway backs the REMOTE principal; export under
the sender AGENT principal, not `/gateway`). That is independent confirmation
that 11-05's corrections were right, not just internally consistent.

## What this unblocks

**v1.0.0-rc.1** can be tagged. `v1.0.0` itself still requires design review C's
§16 nine-item checklist per the phase decision record.
