---
phase: 02-daemon-inbox
verified: 2026-04-14T21:10:00Z
status: gaps_found
score: 5/5 ROADMAP success criteria verified; 7/11 plan-claimed requirements satisfied per REQUIREMENTS.md wording
overrides_applied: 0
gaps:
  - truth: "INBOX-01 requires a structured JSONL schema with fields {received_at, task_id, from_principal, message_class, envelope_bytes_b64, body_json}"
    status: partial
    reason: "Plan 02-01 claims INBOX-01 but writes the raw envelope JSON bytes verbatim (one envelope per line). Schema wrapping fields are absent. The plan explicitly chose byte-exact raw-bytes to avoid a typed-decode-then-reencode step (preserves signature integrity per P3). This is a defensible technical decision that CONTRADICTS the REQUIREMENTS.md wording. Either the requirement must be rewritten to match reality, or Phase 3 must add a wrapper line format that carries the raw bytes inside a b64 field."
    artifacts:
      - path: "crates/famp-inbox/src/append.rs"
        issue: "append() takes envelope_bytes: &[u8] and writes them + \\n; no wrapper object, no received_at, no message_class discriminator on disk"
    missing:
      - "Either: update REQUIREMENTS.md INBOX-01 to match the raw-bytes-per-line reality (documenting the P3 byte-exact rationale)"
      - "Or: add a Phase 3 wrapper that persists {received_at, task_id, from_principal, message_class, envelope_bytes_b64, body_json} around each envelope"
  - truth: "INBOX-02 requires a sidecar inbox.cursor file tracking byte offset of last-read entry"
    status: failed
    reason: "Plan 02-01 claims INBOX-02 but implements fsync-before-return durability instead. No inbox.cursor file exists anywhere in the codebase. The plans confused INBOX-02 with DAEMON-05. The actual cursor semantics are needed by `famp await` / `famp inbox --unread --mark-read` — all Phase 3 surfaces — so the functionality has nowhere to land."
    artifacts:
      - path: "crates/famp-inbox/src/"
        issue: "No cursor module, no inbox.cursor file operations, no byte-offset tracking"
    missing:
      - "Phase 3 must introduce inbox.cursor file handling, OR REQUIREMENTS.md INBOX-02 must be re-mapped to Phase 3 and plan 02-01 frontmatter corrected"
  - truth: "INBOX-03 requires `famp await` to block-with-timeout by polling the inbox file (default 250ms)"
    status: failed
    reason: "Plan 02-02 claims INBOX-03 but the subcommand `famp await` does not exist in this phase. What plan 02-02 actually ships is the bind-collision / PortInUse guard — which matches REQUIREMENTS.md DAEMON-04, not INBOX-03. Pure labeling error in the plan frontmatter."
    artifacts:
      - path: "crates/famp/src/cli/listen/mod.rs"
        issue: "No await subcommand, no polling loop, no timeout semantics — this file wires `famp listen`, not `famp await`"
    missing:
      - "Move INBOX-03 to Phase 3 ROADMAP requirements list; correct plan 02-02 frontmatter to not claim it"
  - truth: "INBOX-05 requires an inbox.lock advisory file preventing concurrent readers from double-consuming"
    status: failed
    reason: "Plans 02-01/02-03 claim INBOX-05 but implement truncated-tail tolerance (which is INBOX-04). No inbox.lock file, no advisory-lock acquisition path, no reader coordination logic. Another pure labeling conflation — INBOX-04 and INBOX-05 were treated as one concept."
    artifacts:
      - path: "crates/famp-inbox/src/read.rs"
        issue: "read_all is a pure stateless function — no lock acquisition, no .lock sidecar file management"
    missing:
      - "Move INBOX-05 to Phase 3 ROADMAP requirements list; phase 3 reader must acquire inbox.lock before advancing the cursor"
  - truth: "Plan frontmatter DAEMON-03 / DAEMON-04 labels are swapped relative to REQUIREMENTS.md wording"
    status: partial
    reason: "REQUIREMENTS.md DAEMON-03 = SIGINT/SIGTERM graceful shutdown; DAEMON-04 = single-instance bind gate. Plan 02-02 frontmatter swaps them: claims DAEMON-03 for 'second listen → PortInUse' and DAEMON-04 for 'SIGINT → exit 0'. Both BEHAVIORS are implemented and tested correctly — this is a label/id mismatch only, not a functional gap. But the audit trail is wrong."
    artifacts:
      - path: ".planning/milestones/v0.8-phases/02-daemon-inbox/02-02-PLAN.md"
        issue: "requirements frontmatter uses DAEMON-03 / DAEMON-04 labels that do not match REQUIREMENTS.md definitions"
    missing:
      - "Correct the plan frontmatter / SUMMARY requirement-to-test map so DAEMON-03 points to listen_shutdown and DAEMON-04 points to listen_bind_collision"
deferred: []
---

# Phase 2: Daemon & Inbox — Verification Report

**Phase Goal:** A running `famp listen` process accepts inbound signed messages over HTTPS, persists each one durably to a JSONL inbox, and shuts down cleanly — all without any change to the v0.7 wire protocol or transport code.
**Verified:** 2026-04-14T21:10:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp listen` starts, prints bound address to stderr, accepts `POST /famp/v0.5.1/inbox/{principal}` using on-disk key + TLS cert, no manual flag wiring | VERIFIED | `crates/famp/src/cli/listen/mod.rs:78` emits `eprintln!("listening on https://{bound}")`; `load_identity` wires Phase 1 files; integration test `listen_smoke.rs::smoke_post_delivers_to_inbox` exercises end-to-end |
| 2 | Signed message appears as JSONL line in `inbox.jsonl` within HTTP response cycle; SIGKILL immediately after 200 leaves line intact on disk | VERIFIED | `crates/famp-inbox/src/append.rs:109` calls `sync_data().await` before `Ok(())`; `listen_durability.rs::sigkill_after_200_leaves_line_intact` subprocess SIGKILLs the daemon after the 200 and asserts exactly one value via `read_all` |
| 3 | Second `famp listen` on same port exits non-zero with typed error (no random-port fallback, no hang) | VERIFIED | `crates/famp/src/cli/listen/mod.rs:61` maps `AddrInUse` → `CliError::PortInUse`; `listen_bind_collision.rs::second_listen_on_same_port_errors_port_in_use` asserts non-zero exit + "already bound" in stderr |
| 4 | SIGINT / SIGTERM causes stop-accepting, flush, exit 0 within few seconds | VERIFIED | `signal::shutdown_signal` uses `tokio::signal::unix` for both signals; `listen_shutdown.rs::sigint_causes_exit_0_within_5s` delivers SIGINT via `/bin/kill -INT`, asserts `ExitStatus::success()` within 5s |
| 5 | Truncated / malformed JSONL line does not prevent `read_all` from returning the lines that precede it | VERIFIED | `crates/famp-inbox/src/read.rs` tail-tolerant `read_all`; unit tests `tail_tolerant_*` + integration `listen_truncated_tail.rs` |

**Score:** 5/5 ROADMAP success criteria verified.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-inbox/Cargo.toml` | workspace-member crate | VERIFIED | Present; in root `Cargo.toml [workspace.members]` |
| `crates/famp-inbox/src/append.rs` | `Inbox::open` + `append` with fsync | VERIFIED | `sync_data` on line 109; `mode(0o600)` on line 47; `EmbeddedNewline` guard on line 93 |
| `crates/famp-inbox/src/read.rs` | tail-tolerant `read_all` | VERIFIED | present; unit + integration tests green |
| `crates/famp-inbox/src/error.rs` | `InboxError { Io, CorruptLine, EmbeddedNewline }` | VERIFIED | shipped |
| `crates/famp/src/cli/error.rs` | new variants `PortInUse`, `Inbox`, `Tls` | VERIFIED | lines 62/65/68 |
| `crates/famp/src/cli/listen/mod.rs` | `run` + `run_on_listener` async entry points | VERIFIED | implements bind, identity load, router build, select! shutdown |
| `crates/famp/src/cli/listen/router.rs` | reuses `FampSigVerifyLayer`; handler calls `inbox.append` | VERIFIED | `router.rs:58` layers `FampSigVerifyLayer::new(keyring)`; line 74 `inbox.append(&body).await` before returning 200 |
| `crates/famp/src/cli/listen/signal.rs` | SIGINT + SIGTERM future | VERIFIED | `tokio::signal::unix` select! |
| `crates/famp/tests/listen_*.rs` | 5 integration tests | VERIFIED | `listen_smoke`, `listen_durability`, `listen_bind_collision`, `listen_shutdown`, `listen_truncated_tail` all green under nextest |

### Key Link Verification

| From | To | Via | Status |
|------|-----|-----|--------|
| `append.rs` | `tokio::fs::File` | `sync_data()` called before Ok | WIRED (line 109) |
| Cargo workspace | `crates/famp-inbox` | `members` entry | WIRED |
| `listen::run` | `init::load_identity` | loads (signing_key, tls_cert, config) | WIRED |
| `listen::router::handler` | `famp_inbox::Inbox::append` | `append(&body).await` before 200 | WIRED (router.rs:74) |
| `listen::router` | `FampSigVerifyLayer` | tower layer reused unmodified | WIRED (router.rs:58) |
| `listen::run` | `TcpListener::bind` | `AddrInUse` → `PortInUse` | WIRED (mod.rs:61) |

### Data-Flow Trace

| Artifact | Data | Source | Real Data? | Status |
|----------|------|--------|------------|--------|
| `inbox.jsonl` line | raw signed envelope bytes | HTTP request body post sig-verify | YES — `listen_durability.rs` asserts line on disk after SIGKILL via real signed envelope | FLOWING |
| `eprintln!("listening on ...")` | bound SocketAddr | `listener.local_addr()` | YES — `listen_durability.rs` parses beacon from child stderr | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Workspace tests pass | `cargo nextest run --workspace` | 298 passed, 1 skipped | PASS |
| Clippy clean (all targets) | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings | PASS |
| No OpenSSL in dep tree | `cargo tree -i openssl` | `package ID specification openssl did not match any packages` | PASS |
| `famp init` smoke | `FAMP_HOME=$tmp cargo run -q -p famp -- init` | creates key.ed25519 0600, pub.ed25519, tls.cert.pem, tls.key.pem, config.toml, peers.toml | PASS |
| Live listen smoke | covered by `listen_durability.rs` subprocess which: spawns `famp listen`, parses stderr beacon, POSTs signed envelope, SIGKILLs, asserts durable line | PASS (via integration test, equivalent to manual smoke) | PASS |

### Requirements Coverage

The plans claim 11 requirement IDs: `CLI-02, DAEMON-01..05, INBOX-01..05`. Scoring each against the REQUIREMENTS.md wording (not against the plan's re-interpretation):

| Req | REQUIREMENTS.md definition | Source plan | Status | Evidence |
|-----|---------------------------|-------------|--------|----------|
| CLI-02 | `famp listen` runs daemon, foreground, stderr logs | 02-02 | SATISFIED | `cli/listen/mod.rs`; foreground behavior; `eprintln!` beacon |
| DAEMON-01 | Wraps v0.7 HTTP server + sig-verify middleware, no protocol changes | 02-02 | SATISFIED | `router.rs` reuses `FampSigVerifyLayer` unmodified; `famp-transport-http` untouched |
| DAEMON-02 | Verified messages appended; failed ones dropped | 02-02 | SATISFIED | sig-verify middleware runs before handler; handler calls `inbox.append`; middleware drops failures (v0.7 behavior preserved) |
| DAEMON-03 | SIGINT/SIGTERM graceful shutdown + flush + exit 0 | 02-02 (labeled as "port-in-use") | SATISFIED (label mismatch) | `signal::shutdown_signal` + `listen_shutdown.rs` — functional, but plan frontmatter calls this DAEMON-04 |
| DAEMON-04 | Single-instance gate (bind refusal) | 02-02 (labeled as "SIGINT") | SATISFIED (label mismatch) | `AddrInUse` → `CliError::PortInUse` + `listen_bind_collision.rs` — functional, but plan frontmatter calls this DAEMON-03 |
| DAEMON-05 | Inbox write is fsync'd before 200 | 02-03 (labeled as "integration coverage") | SATISFIED | `append.rs:109 sync_data`; `listen_durability.rs` SIGKILL test |
| INBOX-01 | JSONL with {received_at, task_id, from_principal, message_class, envelope_bytes_b64, body_json} | 02-01 | **NOT SATISFIED** | Implementation writes raw envelope bytes verbatim — no wrapper object. Byte-exact choice was deliberate (see gaps) but REQUIREMENTS.md wording is not met |
| INBOX-02 | Sidecar `inbox.cursor` file with last-read byte offset | 02-01 (labeled as "fsync") | **NOT SATISFIED** | No cursor file exists. Plan confused this with DAEMON-05 fsync |
| INBOX-03 | `famp await` block-with-timeout polling (default 250ms) | 02-02 (labeled as "bind-collision") | **NOT SATISFIED** | No `famp await` subcommand exists in Phase 2 |
| INBOX-04 | Truncated/malformed line tolerance in reader | 02-01 + 02-03 | SATISFIED | `read_all` tail-tolerant; 3 unit tests + 1 integration test |
| INBOX-05 | Advisory `inbox.lock` preventing double-consume | 02-01 (labeled as "mid-file corruption") | **NOT SATISFIED** | No lock file, no advisory-lock path |

**Requirement-label satisfaction:** 7/11 satisfied per REQUIREMENTS.md wording, 4 misclaimed.

**Functional satisfaction of ROADMAP success criteria:** 5/5 (the ROADMAP SC list does not include cursor/lock/`famp await`, which belong to Phase 3's `famp await` surface).

### Orphaned Requirements

INBOX-02, INBOX-03, INBOX-05 are currently mapped to Phase 2 in REQUIREMENTS.md (§Traceability table, lines 130, 131, 133) but are NOT mentioned in Phase 3's ROADMAP requirements list (`CLI-03..06, CONV-01..05`). If left uncorrected, they will be orphaned at milestone audit.

### Anti-Patterns Found

None flagged. Sampled files: `append.rs`, `read.rs`, `listen/mod.rs`, `listen/router.rs`, `listen/signal.rs`. No TODOs, no empty implementations, no placeholder returns, no hardcoded empty data in live code paths, no `console.log`/`println!` stubs.

### Human Verification Required

None. All 5 ROADMAP success criteria are covered by automated integration tests that run under `cargo nextest` and exercise the real OS-process + signal + fsync + bind-collision paths. The `listen_durability.rs` subprocess test is a strictly stronger form of the manual "start daemon, POST, SIGKILL, check disk" smoke described in the context.

### Gaps Summary

The phase fully delivers the **ROADMAP success criteria** — all 5 are observably true in the code and locked by integration tests. The workspace is green (298/298), clippy is clean, `cargo tree -i openssl` is empty, and a live `famp init` smoke creates a correctly-permissioned identity tree.

However, the phase plans' `requirements_addressed` frontmatter makes **four misaligned claims** against REQUIREMENTS.md:

1. **INBOX-01** is partially satisfied: the format IS JSONL, but the per-line structure is a raw envelope rather than the wrapper object REQUIREMENTS.md specifies. This is a defensible technical choice (byte-exact preservation for signature re-verification) — but one of the two documents must change.
2. **INBOX-02** is claimed but implements fsync, not a cursor sidecar. INBOX-02 (cursor) is Phase 3 territory and must move.
3. **INBOX-03** is claimed but implements the port-bind guard, not `famp await` polling. INBOX-03 belongs with `famp await` in Phase 3.
4. **INBOX-05** is claimed but implements truncation tolerance (INBOX-04). INBOX-05 (advisory lock) belongs with the Phase 3 reader.
5. **DAEMON-03 / DAEMON-04** are swapped in plan frontmatter relative to REQUIREMENTS.md (both behaviors are correctly implemented — it's a label swap only).

**Recommended closure:**

Option A (documentation-only fix): rewrite REQUIREMENTS.md INBOX-01 to describe the raw-bytes-per-line format and document the byte-exact rationale; move INBOX-02/03/05 into Phase 3's requirement list and add them to Phase 3 ROADMAP success criteria; fix the DAEMON-03 ↔ DAEMON-04 label swap in plan 02-02 frontmatter and SUMMARY. **No code changes.** This is the cheap, honest path because the ROADMAP success criteria — the actual contract — are all green.

Option B (code changes): add a wrapper line format, cursor file handling, and lock file handling inside famp-inbox in Phase 2. **Not recommended** — cursor + lock only become meaningful when `famp await` exists in Phase 3, and the wrapper format would break the byte-exact signature preservation the plan was explicitly built around.

---

*Verified: 2026-04-14T21:10:00Z*
*Verifier: Claude (gsd-verifier)*
