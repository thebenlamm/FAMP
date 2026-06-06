---
phase: 06-onboarding-cross-platform-docs
plan: 03
status: complete
requirements: [DOC-01, DOC-02, DOC-03]
---

# 06-03 Summary: Accuracy gate — README verified against the live binary

## What was done

The hard D-07 accuracy gate: every documented command verified against the
actual installed binary (`~/.cargo/bin/famp`), side-effect-free quotes
automatically (Task 1) and the side-effecting lifecycle + fresh-clone Claude+Codex
E2E by human verification (Task 2).

### Task 1 (auto) — side-effect-free quote verification
Ran `famp -V`, `famp daemon --help`, `famp daemon status --help`,
`famp broker --help` and grepped the EPERM source-of-truth string. All README
quotes matched the binary EXCEPT one drift, corrected:

- **Status exit-code drift:** the `famp daemon status` CLI row read
  `exits 0 / 2 / 1` against states `(not-installed / installed-down / running)` —
  positionally wrong. Binary truth: **0 running / 1 not-installed /
  2 installed-but-down**. Reworded the row to pair each code with its state
  unambiguously. (commit `docs(06-03): accuracy gate Task 1`)

No exit-code/CI-verified idempotency overclaim; idempotency framed only as
"re-running is safe" per `--help`.

### Task 2 (human-verify, BLOCKING) — live lifecycle + E2E
Ben ran the side-effecting lifecycle on macOS. Results:

| Step | Result |
|---|---|
| install + idempotency | ✅ (after fix below) — install ×2/×3 all exit 0, one registration, no broker restart |
| restart + status RUNNING | ✅ status render matches README form verbatim |
| fresh-clone Claude+Codex E2E | ✅ sandboxed Codex (`bob`) received `alice`'s message with no hand-started broker — daemon served the sandboxed client (DOC-01 acceptance) |
| uninstall ×2 → NOT_INSTALLED | ✅ both exit 0; `status` → NOT_INSTALLED (exit 1); LaunchAgent removed |

## The blocker found and resolved (the gate did its job)

Initial idempotency check **failed**: a 2nd `famp daemon install` errored with
`Bootstrap failed: 5: Input/output error` / `launchctl failed with exit code 5`
— contradicting the README + `--help` "re-running is safe" claim (the one item
Phase 5 left behaviorally UNCERTAIN).

**Root cause: stale installed binary, NOT a code or docs bug.**
- The source `load_macos` (install.rs:224-235) guards idempotency via a
  registration probe (`launchctl_is_registered`) and no-ops if already loaded.
- The installed binary was built **Jun 4 16:00**; the idempotency fix
  `fe3cea4 "make launchd install idempotent against real launchctl exit 5"`
  landed **Jun 4 17:14** — ~74 min later. The installed binary still had the
  old "tolerate exit 37" logic, so launchctl's exit 5 fell through to an error.
- `famp -V` (0.11.0) and `--help` were unchanged across that hour — only the
  idempotency *logic* changed — so the version string looked current but wasn't
  for this behavior.

**Fix:** `just install` (rebuild + reinstall from current source). After that,
`famp daemon install` ×2/×3 no-op cleanly (rc=0), pid unchanged (no restart),
exactly one `com.famp.broker` registration. The README's idempotency claim is
now TRUE against the actual installed binary — **no docs reword, no code change**
(the fix was already committed in source; the binary was simply stale).

## Residual observations (non-blocking, no doc change)

- `famp daemon status` immediately after a fresh `install`/reinstall can
  transiently report `INSTALLED_DOWN (no_socket_file)` for ~1-3s before the
  broker binds its socket; it self-corrects to RUNNING. The documented
  first-install path showed RUNNING immediately; the race only surfaced after a
  rapid uninstall→reinstall cycle. Noted, not documented as a quickstart caveat.
- Codex does **not** auto-wake on inbound (listen-mode auto-wake is a Claude Code
  Stop-hook feature; `install-codex` writes only the MCP server entry). The E2E
  flow is: send → prompt the Codex window to check its inbox → `famp_inbox`.
  Delivery (the DOC-01 claim) is unaffected. Considered a possible future
  README clarity nit; not changed this phase (out of the three DOC requirements'
  scope, and the quickstart does not claim Codex auto-wakes).

## Key files

- `README.md` (modified) — status exit-code row corrected (Task 1).

## Verification

- Task 1 automated gate: PASS (famp -V == famp 0.11.0; daemon --help lists
  install/uninstall/status/restart; broker --help has --no-idle-exit; EPERM
  string in spawn.rs:28; no idempotency overclaim).
- Task 2 human-verify: all acceptance criteria met against the live binary after
  `just install`.

DOC-01 / DOC-02 / DOC-03 confirmed accurate against shipped Phase 4/5 behavior.
