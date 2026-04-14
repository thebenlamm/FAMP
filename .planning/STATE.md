---
gsd_state_version: 1.0
milestone: v0.8
milestone_name: Usable from Claude Code
status: executing
last_updated: "2026-04-14T20:34:40.055Z"
last_activity: 2026-04-14
---

# STATE: FAMP — v0.8 Usable from Claude Code

**Last Updated:** 2026-04-14 (v0.8 roadmap defined; 4 phases, 37 requirements, 100% coverage)

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-14 with v0.8 Current Milestone section)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** Phase 02 — daemon-inbox

## Current Position

Phase: 02 (daemon-inbox) — EXECUTING
Plan: 2 of 3 (Plan 01 shipped: famp-inbox library)
Status: Executing Phase 02
Last activity: 2026-04-14 -- Plan 02-01 shipped: famp-inbox (8/8 crate, 292/292 workspace)

```
v0.8 Progress: [░░░░░░░░░░░░░░░░░░░░] 0% (0/4 phases)
```

## Last Shipped

- **v0.7 Personal Runtime** (2026-04-14) — 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests green. `famp-envelope`, `famp-fsm`, `famp-transport` + `MemoryTransport`, `famp-keyring` (TOFU), `famp-transport-http` (axum + rustls + reqwest), two finish-line examples, 3×2 adversarial matrix. `cargo tree -i openssl` empty.

## Accumulated Context

### Key Decisions Logged (carried forward)

- **Language: Rust** — compiler-checked INV-5 via exhaustive enum `match`
- **Personal Profile before Federation Profile** (adopted 2026-04-12) — v0.6 + v0.7 are the solo-dev finish line; v0.8+ stacks federation semantics on top without substrate churn
- **v0.5.1 spec fork is authority** — all implementation bytes hash against `FAMP-v0.5.1-spec.md`
- **SEED-001 RESOLVED 2026-04-13:** keep `serde_jcs 0.2.0` — 12/12 RFC 8785 conformance gate green; fallback plan on disk as insurance. Evidence in `.planning/SEED-001.md`.
- **`verify_strict`-only public surface** — raw `verify` unreachable from `famp-crypto` public API
- **Domain separation prefix prepended internally** — callers never assemble signing input by hand; `canonicalize_for_signature` is the only sanctioned path
- **Narrow, phase-appropriate error enums** — not one god enum (repeated pattern in Plans 01-01 D-16 and 02-01)
- **15-category flat `ProtocolErrorKind` + exhaustive consumer stub under `#![deny(unreachable_patterns)]`** — new error categories become compile errors in downstream crates
- **`AuthorityScope` hand-written 5×5 `satisfies()` truth table, no `Ord` derive** — authority is a ladder, not a total order
- **v0.7 TOFU keyring stays local-file** — `HashMap<Principal, VerifyingKey>`, principal = raw Ed25519 pubkey, loaded from file or `--peer` CLI flag. Agent Cards defer to v0.9.
- **v0.7 adversarial matrix = 3 cases × 2 transports, not 18** — CONF-05/06/07 own the three cases; Phase 4 extended the same matrix to HTTP without new CONF-0x requirements.
- **ENV-09 and ENV-12 are intentionally narrowed for v0.7** — ENV-09 ships with no capability-snapshot binding; ENV-12 ships cancel-only. Wider forms defer to Federation Profile.
- **D-B5 + D-D1 resolved (2026-04-13):** `TerminalStatus` lifted to `famp-core` alongside `MessageClass`; `famp-fsm` depends only on `famp-core`, never on `famp-envelope`.
- **relation field dropped from TaskTransitionInput (2026-04-13):** D-B3 resolved — no v0.7 legal arrow needs relation inspection.
- **FSM step() and accessors are const fn** — All transition arms operate on Copy enums.
- **v0.8 phase shape:** 4 phases derived from the requirement dependency graph — Phase 1 (identity + CLI scaffold), Phase 2 (daemon + inbox), Phase 3 (conversation CLI + task records), Phase 4 (MCP + E2E). Dependency chain: v0.7 → P1 → P2 → P3 → P4.
- **Plan 02-01 (2026-04-14):** `famp-inbox` takes raw `&[u8]`, not a typed `SignedEnvelope`, to preserve byte-exactness (P3) and keep the crate decoupled from `famp-envelope`. Append path is fsync-sealed via `tokio::fs::File::sync_data` under `Arc<Mutex<File>>`; 16-task concurrent test locks the serialization contract; `read_all` tail-tolerance swallows the final truncated line but surfaces mid-file corruption as a hard `CorruptLine { line_no }` error. Covers INBOX-01/02/04/05.

### Open TODOs

- None carried.

### Known Blockers

- **None.**

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260414-cme | Remove obsolete wave2_impl feature gate from famp-canonical | 2026-04-14 | a77cfe1 | [260414-cme-remove-obsolete-wave2-impl-feature-gate-](./quick/260414-cme-remove-obsolete-wave2-impl-feature-gate-/) |
| 260414-ecp | Wire UnsupportedVersion error on envelope decode (PR #2) | 2026-04-14 | 8d14341 | [260414-ecp-wire-unsupportedversion-error-on-envelop](./quick/260414-ecp-wire-unsupportedversion-error-on-envelop/) |
| 260414-esi | Seal famp field visibility + cover adversarial gaps (PR #2.1) | 2026-04-14 | 2e9cf92, bf4c70a | [260414-esi-seal-famp-field-visibility-and-cover-adv](./quick/260414-esi-seal-famp-field-visibility-and-cover-adv/) |
| 260414-f4i | famp-crypto rustdoc + README "How FAMP Signs a Message" + CONTRIBUTING.md (PR #3) | 2026-04-14 | c0c5311, 243fc19, 1b432c5 | [260414-f4i-docs-pr-famp-crypto-rustdoc-readme-overv](./quick/260414-f4i-docs-pr-famp-crypto-rustdoc-readme-overv/) |
| 260414-fjo | PR #4 architectural cleanup: drop Signer/Verifier traits, remove 5 stub crates, add famp umbrella re-exports | 2026-04-14 | 9e5426f, 08c442a, e8ecf9f | [260414-fjo-pr-4-architectural-cleanup-drop-signer-v](./quick/260414-fjo-pr-4-architectural-cleanup-drop-signer-v/) |
| 260414-g32 | PR #4.1 adversarial review followups: reword WeakKey doc, delete dead InvalidSigningInput variant, add is_weak() gate to CONTRIBUTING "Do Not Touch" list | 2026-04-14 | 278cb83 | [260414-g32-pr-4-1-fix-weakkey-docstring-drop-dead-v](./quick/260414-g32-pr-4-1-fix-weakkey-docstring-drop-dead-v/) |

## Session Continuity

### Recent Activity

- **2026-04-14:** **Plan 02-01 shipped** — `famp-inbox` library crate with durable append (fsync-before-return) + tail-tolerant read. 8/8 crate tests, 292/292 workspace tests, `cargo tree -i openssl` empty. Commits `b7ca9bb` (feat) + `071b781` (test). INBOX-01/02/04/05 complete.
- **2026-04-14:** **v0.8 roadmap created.** 4 phases, 37 requirements, 100% coverage. Phase 1 (Identity & CLI Foundation) queued for `/gsd:plan-phase 1`.
- **2026-04-14:** **v0.7 Personal Runtime shipped.** 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests. Archived to `.planning/milestones/v0.7-*.md`.
- **2026-04-14:** Completed quick task 260414-g32: PR #4.1 adversarial review followups. `just ci` green. 261/261 workspace tests.
- **2026-04-14:** Completed quick task 260414-fjo: PR #4 architectural cleanup. Drop Signer/Verifier traits, remove 5 stub crates, add famp umbrella re-exports. `just ci` green. 261/261 workspace tests.

---
*2026-04-14 — v0.8 roadmap defined. 4 phases: (1) Identity & CLI Foundation — CLI-01/07, IDENT-01..06; (2) Daemon & Inbox — CLI-02, DAEMON-01..05, INBOX-01..05; (3) Conversation CLI — CLI-03..06, CONV-01..05; (4) MCP Server & Same-Laptop E2E — MCP-01..06, E2E-01..03. 37/37 requirements mapped. Next: `/gsd:plan-phase 1`.*
