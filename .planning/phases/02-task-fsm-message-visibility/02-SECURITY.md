---
phase: 02
slug: task-fsm-message-visibility
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-11
---

# Phase 02 - Security

Per-phase security contract: threat register, accepted risks, and audit trail.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| client (any UDS connector on host) -> broker handler | Untrusted local RPC arguments cross into inspector handlers. | `InspectTasksRequest.id`, `InspectMessagesRequest.to`, `InspectMessagesRequest.tail` |
| async event loop <-> blocking thread pool | Inspect I/O crosses into `spawn_blocking`; captured data must be owned/Send and read-only. | `BrokerStateView`, cursor offsets, bus paths, inspect kind |
| broker process <-> filesystem | Taskdir and mailbox JSONL files are read inside the blocking closure. | `~/.famp/tasks/`, `~/.famp/mailboxes/` records |
| operator shell -> `famp inspect` CLI process | CLI args are parsed before broker RPC construction. | `--id`, `--full`, `--orphans`, `--to`, `--tail`, `--json` |
| `famp inspect` CLI -> broker UDS | Local UDS boundary inherited from Phase 1; kernel-enforced local socket access. | enum-tagged inspect requests and replies |
| integration test fixture -> broker subprocess | Test harness owns broker lifetime and tempdir state. | subprocess, tempdir, socket path |

---

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-02-01 | I | `inspect_messages` handler returning body content | mitigate | `MessageRow` exposes sender, recipient, task id, class, state, timestamp, `body_bytes`, and `body_sha256_prefix`; no body field crosses the wire. `metadata_only_no_body` asserts a literal body is absent from stdout. | closed |
| T-02-02 | I | `--full` output exposing envelopes from other recipients | mitigate | `inspect_tasks` detail/full filters observed envelopes by task id before rendering; unobserved federation legs are represented by schema support for `bytes: None` / `reason`, not by broadening scope. | closed |
| T-02-03 | T | Reply enum wire form drift | mitigate | `InspectTasksReply` and `InspectMessagesReply` use `#[serde(tag = "kind", rename_all = "snake_case")]`; roundtrip/tag tests and `canonicalize_roundtrip` cover the locked JSON/JCS shape. | closed |
| T-02-04 | I | Path traversal via `to: Option<String>` | mitigate | Handler performs a `BTreeMap` lookup against `ctx.message_data.by_recipient`; broker populates the map only from registered `BrokerStateView.clients`. The handler does no path construction from `to`. | closed |
| T-02-05 | E | Read-only invariant violation by future contributor | mitigate | Inspector dispatch takes `&BrokerStateView` / `&BrokerCtx`; broker captures a cloned state view and cursor map into `spawn_blocking`, never `&mut Broker`. | closed |
| T-02-06 | D | Pathological `tail` value | accept | `tail` conversion falls back to `usize::MAX`, slicing is bounded by existing entry count, and the 500ms RPC budget caps worst-case latency. | closed |
| T-02-07 | I | Disclosure of taskdir path structure to any local UDS connector | accept | Local UDS access is the existing v0.9 trust boundary; inspector remains read-only. Gateway-auth boundary is deferred to v1.0. | closed |
| T-02-08 | D | Broker event loop blocked by slow taskdir walk | mitigate | Broker wraps taskdir/mailbox I/O and dispatch in `tokio::task::spawn_blocking` under `tokio::time::timeout(Duration::from_millis(500))`, returning `budget_exceeded`. | closed |
| T-02-09 | D | Blocking pool exhaustion under 1000 concurrent inspect calls | mitigate | Runtime builder sets `.max_blocking_threads(1024)`; `inspect_cancel_1000` runs under a serialized nextest group. | closed |
| T-02-10 | D | FD/lock/allocation leak on cancelled handler | mitigate | Timeout stops awaiting the join handle; file handles are stack-local in the blocking closure. `one_thousand_cancel_no_leak` asserts completed calls and FD delta `< 10`. | closed |
| T-02-11 | I | Privilege boundary breach from `spawn_blocking` closure | mitigate | Closure captures owned `BrokerStateView`, cursor offsets, paths, and cloned inspect kind; no mutable broker reference enters the blocking thread. | closed |
| T-02-12 | T | TOCTOU on taskdir between walk start and dispatch | accept | Inspector is a read-only snapshot command; a mid-walk task can be missed until the next invocation but cannot corrupt state. | closed |
| T-02-13 | I | Timeout elapsed value revealing taskdir size | accept | `elapsed_ms` is bounded by the 500ms budget and local UDS users can already inspect local task files under the v0.9 trust model. | closed |
| T-02-14 | I | Body content leaking into `inspect messages` CLI output | mitigate | CLI table headers are metadata-only (`BODY_BYTES`, `SHA256_PREFIX`) and tests assert no `BODY` column or body literal in stdout. | closed |
| T-02-15 | I | `--full` revealing envelopes from an unobserved federation peer | mitigate | Message snapshots are keyed by registered local recipients; the handler can only render locally observed envelopes for the requested task id. | closed |
| T-02-16 | D | 1000-cancel test exhausting FD limits and crashing nextest worker | mitigate | The test is serialized in `inspect-subprocess` (`max-threads = 1`) and asserts bounded FD growth after 1000 inspect processes. | closed |
| T-02-17 | T | Test fixture leaking secrets into temp dirs after failures | accept | Fixtures use `tempfile::TempDir`; broker subprocess cleanup is in harness code, and fixture payloads are non-secret literals. | closed |
| T-02-18 | I | Malformed `--id` causing broad task scans | mitigate | CLI parses `--id` as `Option<uuid::Uuid>`, rejecting malformed IDs before RPC construction; budget still caps worst-case valid scans. | closed |
| T-02-19 | E | CLI accidentally enabling write surface | mitigate | `InspectTasksArgs` / `InspectMessagesArgs` declare only read-flavored fields, and `InspectKind` has no write variants. | closed |
| T-02-20 | T | Silent corruption of FAILED/CANCELLED FSM states | mitigate | `derive_fsm_state` explicitly matches `completed`, `failed`, and `cancelled`; dedicated unit tests cover failed and cancelled mappings. | closed |

Status: open / closed. Disposition: mitigate / accept / transfer.

---

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-02-01 | T-02-06 | Pathological `tail` does not amplify allocation beyond existing entries and is constrained by the 500ms budget. | Codex security audit | 2026-05-11 |
| AR-02-02 | T-02-07 | Local UDS trust is the current product boundary; this phase preserves read-only behavior and defers stronger auth to the gateway boundary. | Codex security audit | 2026-05-11 |
| AR-02-03 | T-02-12 | Snapshot staleness is acceptable for an operator inspection command and cannot mutate broker/task state. | Codex security audit | 2026-05-11 |
| AR-02-04 | T-02-13 | Bounded timeout metadata adds no meaningful disclosure beyond existing local filesystem access. | Codex security audit | 2026-05-11 |
| AR-02-05 | T-02-17 | Test fixture data is non-secret and tempdir-scoped; cleanup is best-effort through normal test harness ownership. | Codex security audit | 2026-05-11 |

Accepted risks do not resurface in future audit runs unless the relevant trust boundary changes.

---

## Security Audit 2026-05-11

| Metric | Count |
|--------|-------|
| Threats found | 20 |
| Closed | 20 |
| Open | 0 |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-11 | 20 | 20 | 0 | Codex |

---

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

Approval: verified 2026-05-11
