---
phase: 19
slug: auto-wake-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-03
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via `cargo test`; Tokio only in `famp` integration tests |
| **Config file** | Workspace `Cargo.toml` and crate manifests |
| **Quick run command** | `cargo test -p famp-bus --lib auto_wake && cargo test -p famp --test auto_wake_gate` |
| **Full suite command** | `cargo test --workspace && just lint && cargo fmt --all -- --check && just check-no-tokio-in-bus` |
| **Estimated runtime** | Measure during Wave 0 and record before sign-off |

---

## Sampling Rate

- **After every task commit:** Run the task's focused test target plus `just check-no-tokio-in-bus`
- **After every plan wave:** Run `cargo test -p famp-bus && cargo test -p famp --test auto_wake_gate --test quarantine_surfaces --test pair_cli && just lint && cargo fmt --all -- --check`
- **Before `$gsd-verify-work`:** Run `cargo test --workspace && just lint && cargo fmt --all -- --check && just check-no-tokio-in-bus`; the full suite must be green
- **Max feedback latency:** Record measured focused-test runtime during Wave 0; keep each task's default feedback command focused rather than workspace-wide

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 19-01-01 | 01 | 1 | QUAR-12, QUAR-13, QUAR-14 | T-19-01, T-19-02, T-19-03 | Gateway and Unknown cannot select or drain into Await; Local wakes; remote-then-local does not starve | broker actor unit | `cargo test -p famp-bus --lib auto_wake` | ❌ W0 cases in existing `handle/tests.rs` | ⬜ pending |
| 19-01-02 | 01 | 1 | QUAR-12, QUAR-13 | T-19-02, T-19-03 | Repeated fixtures preserve production Local stamps without erasing legacy controls | property regression | `cargo test -p famp-bus --test prop01_dm_fanin_order --test prop02_channel_fanout --test prop04_drain_completeness` | ✅ existing files; fixture repair pending | ⬜ pending |
| 19-02-01 | 02 | 2 | QUAR-12, QUAR-13, QUAR-14 | T-19-05, T-19-06 | One real socket proves remote held, Local wakes, and explicit Inbox retains remote | integration | `cargo test -p famp --test auto_wake_gate` | ❌ W0 new file | ⬜ pending |
| 19-02-02 | 02 | 2 | QUAR-12, QUAR-14 | T-19-05 | Conflicting Phase 14 Gateway-Await-wakes expectation is removed while rendering controls stay green | integration regression | `cargo test -p famp --test quarantine_surfaces` | ✅ existing file; expectation replacement pending | ⬜ pending |
| 19-03-01 | 03 | 3 | QUAR-12, QUAR-13, QUAR-14, QUAR-15 | T-19-08, T-19-10 | Live MCP descriptor and all active guides state Local-only Await/listen-mode wake, explicit remote Inbox recovery, and every residual limitation | descriptor/docs assertions + focused Rust integration + install | <code>set -e; contract='Only Local-origin records satisfy a parked `famp await`; Gateway- and Unknown-origin records remain available through explicit Inbox reads.'; for doc in docs/QUARANTINE.md docs/CONFIGURATION.md README.md CLAUDE.md docs/CLAUDE-CODE-CONTEXT-GUIDE.md docs/CHANNEL-DISCUSSION-GUIDE.md docs/HOST-WAKE-ADAPTERS.md; do rg -Fq "$contract" "$doc"; done; rg -Fq 'Block until one or more new Local-origin messages arrive.' crates/famp/src/cli/mcp/server.rs; rg -Fq 'Only Local-origin records satisfy parked famp_await; Gateway- and Unknown-origin records remain available through explicit famp_inbox reads.' crates/famp/src/cli/mcp/server.rs; rg -Fq 'Agents auto-wake on Local-origin inbound messages without an explicit flag:' CLAUDE.md; rg -Fq 'When a Local-origin message arrives, Claude wakes automatically and receives:' CLAUDE.md; rg -Fq '  → Local-origin message arrives' docs/CLAUDE-CODE-CONTEXT-GUIDE.md; rg -Fq '`famp_await` delivers one eligible Local-origin message at a time.' docs/CHANNEL-DISCUSSION-GUIDE.md; rg -Fq 'fires only on **Local-origin channel messages**' docs/CHANNEL-DISCUSSION-GUIDE.md; rg -Fq 'On each Local-origin batch it prints **one scrubbed stdout line**' docs/HOST-WAKE-ADAPTERS.md; rg -Fq 'On a Local-origin message, emits' docs/HOST-WAKE-ADAPTERS.md; ! rg -Fq 'Block until one or more new messages arrive.' crates/famp/src/cli/mcp/server.rs; ! rg -Fq 'famp_await delivers every message as it arrives' crates/famp/src/cli/mcp/server.rs; ! rg -Fq 'auto-wake on inbound messages' CLAUDE.md; ! rg -Fq 'When a message arrives, Claude wakes automatically and receives:' CLAUDE.md; ! rg -Fq '  → message arrives' docs/CLAUDE-CODE-CONTEXT-GUIDE.md; ! rg -Fq '`famp_await` delivers one message at a time.' docs/CHANNEL-DISCUSSION-GUIDE.md; ! rg -Fq 'fires on **every channel message**' docs/CHANNEL-DISCUSSION-GUIDE.md; ! rg -Fq 'On each inbound batch' docs/HOST-WAKE-ADAPTERS.md; ! rg -Fq 'On message, emits' docs/HOST-WAKE-ADAPTERS.md; for doc in docs/QUARANTINE.md docs/CONFIGURATION.md README.md; do for forbidden_claim in 'blocks remote mailbox ingress' 'remote ingress is blocked' 'remote mailbox ingress is blocked' 'blocks remote content' 'remote content is safe' 'all remote content is safe' 'comprehensive safety' 'provides comprehensive safety' 'prevents remote steering' 'remote agents cannot steer' 'steering is prevented' 'prevents provenance laundering' 'origin cannot be laundered' 'laundering is prevented' 'prevents mailbox growth' 'prevents remote mailbox growth' 'remote traffic cannot grow the mailbox' 'mailbox growth is prevented' 'prevents host re-entry' 'prevents host UI/model re-entry' 'host re-entry is prevented'; do if rg -Fqi "$forbidden_claim" "$doc"; then exit 1; fi; done; done; cargo test -p famp --lib tool_descriptors_has_exactly_twelve_named_tools && cargo test -p famp --test auto_wake_gate && cargo test -p famp --test quarantine_surfaces && cargo test -p famp --test pair_cli consent_warning_matches_quarantine_doc && just check-no-tokio-in-bus && just install</code> | ✅ eight existing descriptor/guide surfaces; truth update pending | ⬜ pending |
| 19-03-02 | 03 | 3 | QUAR-15 | T-19-09 | Consent warning precedes pairing code and matches quarantine documentation; phase gate measured | integration regression + phase gate | `cargo test -p famp --test pair_cli consent_warning_matches_quarantine_doc && cargo test -p famp --test pair_cli artifact_code_offset_greater_than_consent_and_install_lines` | ✅ existing tests | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Repair broker test fixtures so local clients register `Origin::Local`, origin-specific fixtures are explicit, and mailbox helpers stamp `Out::AppendMailbox` exactly as production does; retain intentional absent-origin compatibility cases.
- [ ] Add focused actor cases in `crates/famp-bus/src/broker/handle/tests.rs` for Gateway, Unknown, Local, preexisting remote drain, remote-then-local ordering, and the channel path.
- [ ] Add `crates/famp/tests/auto_wake_gate.rs` using the established broker/register/`BusClient` harness.
- [ ] Replace `quarantine_surfaces.rs::await_marks_gateway_origin`, whose expected behavior conflicts with Phase 19.
- [ ] Confirm the existing QUAR-15 `pair_cli` regressions remain green.
- [ ] Measure focused and full validation runtimes and replace the provisional latency entries above.

---

## Manual-Only Verifications

All phase behaviors have automated verification. Documentation wording is asserted where a stable exact string is required and reviewed as part of the plan acceptance criteria.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency is measured and bounded
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
