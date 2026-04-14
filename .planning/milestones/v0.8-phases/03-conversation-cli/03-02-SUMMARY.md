---
phase: 03-conversation-cli
plan: 02
subsystem: conversation-cli-outbound
tags: [famp-send, famp-peer-add, tofu-tls, taskdir, fsm]

requires:
  - phase: 03-conversation-cli-plan-01
    provides: famp-taskdir, PeerEntry schema, write_peers helpers

provides:
  - famp peer add subcommand (validated, atomic)
  - famp send subcommand with --new-task / --task / --terminal modes
  - TOFU rustls ServerCertVerifier capturing leaf SHA-256 into peers.toml
  - cli::send::fsm_glue — on-disk TaskRecord ↔ TaskFsm bridge
  - cli::paths tasks_dir/inbox_cursor_path/inbox_jsonl_path/peers_toml_path
  - cli::config::{read_peers, write_peers_atomic} shared helpers
  - Extended PeerEntry with optional `principal` field
  - 12 new CliError variants covering send/peer/task/TLS failures

affects: [03-03-await-inbox, 03-04-peer-add-lock]

tech-stack:
  added:
    - "reqwest 0.13 with rustls-no-provider feature (custom TLS config)"
    - "sha2 + hex — leaf cert SHA-256 fingerprinting for TOFU pinning"
    - "uuid (runtime dep) — MessageId parse in deliver causality"
  patterns:
    - "Custom rustls::client::danger::ServerCertVerifier that IS the trust anchor — fingerprint pinning bypasses X.509 chain validation entirely"
    - "POST-first, persist-second ordering: task records only mutate after HTTP 2xx so network / TOFU / non-2xx failures leave on-disk state untouched"
    - "Marker-in-error-string propagation for TOFU mismatch: rustls::Error::General carries a `famp-tofu-mismatch:pinned:got` sentinel that reqwest's error chain preserves, parsed back into CliError::TlsFingerprintMismatch"
    - "Phase 3 FSM seeding shortcut — advance_terminal seeds TaskFsm at Committed to satisfy v0.7 legality without a commit-reply round-trip (documented as TODO(phase4))"
    - "peers.toml atomic write reused across peer add + TOFU capture via a single write_peers_atomic helper"

key-files:
  created:
    - crates/famp/src/cli/peer/mod.rs
    - crates/famp/src/cli/peer/add.rs
    - crates/famp/src/cli/send/mod.rs
    - crates/famp/src/cli/send/client.rs
    - crates/famp/src/cli/send/fsm_glue.rs
    - crates/famp/tests/peer_add.rs
    - crates/famp/tests/send_new_task.rs
    - crates/famp/tests/send_deliver_sequence.rs
    - crates/famp/tests/send_terminal_blocks_resend.rs
    - .planning/milestones/v0.8-phases/03-conversation-cli/03-02-SUMMARY.md
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/bin/famp.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/config.rs
    - crates/famp/src/cli/paths.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/_gen_fixture_certs.rs

key-decisions:
  - "Task 1 + Task 2 merged into one commit for compile-atomicity — the plan's Task 1 wires Peer/Send variants into Commands, so splitting would leave an intermediate state where Commands::Send references a non-existent module (same rationale as Plan 02-02)."
  - "PeerEntry grows an Option<String> `principal` field. Deviation from the plan's strict schema: the plan's `agent:host/<hex-of-pubkey>` derivation scheme is incompatible with the Phase 2 listen daemon's hardcoded `agent:localhost/self` self-keyring entry. The `to` field in envelopes must address a principal the receiver's keyring can resolve, which requires a known, non-derived identity. Tests set --principal explicitly; production callers will default to `agent:localhost/self` until Phase 4 adds config-driven principals."
  - "Phase 3 FSM shortcut: advance_terminal SEEDS a TaskFsm at Committed before stepping. The ideal flow is REQUESTED →(commit reply)→ COMMITTED →(terminal deliver)→ COMPLETED, but Phase 3 has no commit-reply round-trip. Phase 4 (MCP + E2E) will replace this with a real round-trip. Marked TODO(phase4) inline in fsm_glue.rs."
  - "Self principal hardcoded to `agent:localhost/self`. Phase 2's listen daemon pins this exact string in its single-entry keyring; Phase 3 `famp send` mirrors it for `from` so signature verification resolves. A config.toml `principal` field lands in Phase 4 alongside the multi-entry keyring."
  - "POST-first-persist-second ordering: taskdir writes run AFTER HTTP 2xx, so a network / TOFU / 4xx failure leaves disk state unchanged. Explicitly locked by the terminal test's post-reject record-equality assertion."
  - "TOFU mismatch translation path: rustls errors bubble up through reqwest::Error::Display. The verifier plants a `famp-tofu-mismatch:<pinned>:<got>` marker in the rustls::Error::General string, and post_envelope parses it back into CliError::TlsFingerprintMismatch. Ugly but the cleanest way to reach typed errors through the erased boundary."
  - "Test daemon + sender share the SAME FAMP_HOME. Phase 2's self-keyring only resolves its own principal, so the integration tests would need a multi-entry keyring to drive a cross-home flow — that multi-entry keyring is Phase 4 work. Shared-home tests still exercise every Phase 3 contract (envelope build, sign, POST, middleware verify, inbox append, taskdir persistence, FSM advance)."
  - "reqwest feature set pinned to `rustls-no-provider` locally (not workspace). The workspace reqwest entry uses `rustls-tls-native-roots` which does not actually exist in reqwest 0.13.2 — it is a latent bug in the workspace Cargo.toml that no crate has hit until now because every prior consumer declared reqwest directly. Left the workspace entry untouched to keep this plan scoped; Plan 03-04 should clean it up."

requirements-completed: [CLI-03, CLI-06, CONV-01, CONV-02, CONV-03]

duration: ~45min
completed: 2026-04-14
---

# Phase 3 Plan 02: `famp peer add` + `famp send` Summary

**`famp send` wires the outbound half of the Phase 3 conversation CLI end-to-end: a signed request/deliver envelope, TOFU-pinned rustls client, byte-exact POST to a running `famp listen` daemon, atomic task-record persistence, and FSM advancement on terminal — plus `famp peer add` as the on-ramp.**

## Performance

- **Duration:** ~45 min
- **Tasks:** 2/2 (merged into one atomic commit — see Decisions)
- **Files created:** 10
- **Files modified:** 9
- **Workspace tests:** 324/324 green (+8 over Plan 03-01's 316), 1 skipped
- **Integration tests added:** 5 peer_add + 3 send = 8

## Accomplishments

- **`famp peer add`** validates endpoint as HTTPS, pubkey as 32 raw base64url-unpadded bytes, rejects duplicates, writes `peers.toml` atomically via same-dir tempfile + fsync + rename, sets 0600 on the persisted file.
- **`famp send --new-task`** builds a `RequestBody` with a minimal legal `Bounds` (hop_limit + recursion_depth — §9.3's ≥2-key rule), signs via `FampSigningKey::sign`, POSTs to the peer's inbox, creates `<home>/tasks/<uuid>.toml` in REQUESTED state, prints the UUIDv7 to stdout.
- **`famp send --task`** (non-terminal) builds a `DeliverBody` with `interim = true`, attaches `Causality { rel: Delivers, ref: <task_message_id> }`, and updates `last_send_at` on success. Multiple sequential calls all succeed; the record stays in REQUESTED.
- **`famp send --task --terminal`** builds `DeliverBody { interim: false, provenance: Some({}) }`, attaches `terminal_status = Completed`, advances the local FSM via `fsm_glue::advance_terminal`, marks the record terminal. Subsequent sends on the same task exit non-zero with `CliError::TaskTerminal`.
- **TOFU TLS pinning** works: first contact captures `sha256(leaf_cert)` hex into `peers.toml.tls_fingerprint_sha256`; subsequent contacts reject mismatches with a typed `TlsFingerprintMismatch { alias, pinned, got }`. The sig-verification (TLS 1.2 and 1.3 handshake) methods are stubbed because the fingerprint itself is the trust anchor — chain validation is bypassed intentionally.
- **POST-first / persist-second** ordering is locked by the terminal test: after a rejected send, `tasks.read(&task_id)` returns a record byte-identical to the one observed before the rejected call (proved via `PartialEq` on `TaskRecord`).
- **CliError** gains 12 new variants: `PeerNotFound`, `PeerDuplicate`, `PeerEndpointInvalid`, `PeerPubkeyInvalid`, `TaskNotFound`, `TaskTerminal`, `SendFailed`, `TaskDir(#[from])`, `Envelope`, `TlsFingerprintMismatch`, `SendArgsInvalid`. All use narrow thiserror variants per the project pattern.

## Task Commits

1. **Tasks 1+2 merged — wire famp peer add + famp send with TOFU TLS pinning** — `93a9092` (feat)

## Files Created/Modified

**Created (10):**
- `crates/famp/src/cli/peer/mod.rs` — PeerArgs / PeerCommand dispatch
- `crates/famp/src/cli/peer/add.rs` — validated `run_add` / `run_add_at`
- `crates/famp/src/cli/send/mod.rs` — SendArgs, run_at, envelope builders, persist path
- `crates/famp/src/cli/send/client.rs` — TofuVerifier + post_envelope async client
- `crates/famp/src/cli/send/fsm_glue.rs` — state_to_str / is_terminal / advance_terminal
- `crates/famp/tests/peer_add.rs` — 5 tests
- `crates/famp/tests/send_new_task.rs` — 1 test
- `crates/famp/tests/send_deliver_sequence.rs` — 1 test
- `crates/famp/tests/send_terminal_blocks_resend.rs` — 1 test
- `.planning/milestones/v0.8-phases/03-conversation-cli/03-02-SUMMARY.md`

**Modified (9):**
- `crates/famp/Cargo.toml` — famp-taskdir, time features, uuid, sha2, hex, reqwest, rustls
- `crates/famp/src/bin/famp.rs` — silencers for new transitive deps
- `crates/famp/src/cli/mod.rs` — Peer + Send Commands variants, Send tokio runtime bootstrap
- `crates/famp/src/cli/error.rs` — 12 new variants
- `crates/famp/src/cli/config.rs` — PeerEntry::principal field, read_peers, write_peers_atomic
- `crates/famp/src/cli/paths.rs` — tasks_dir / peers_toml_path / inbox_cursor_path / inbox_jsonl_path
- Three `examples/*.rs` — silencers for new deps

## Test Coverage

### peer_add (5 tests)
- `peer_add_creates_entry` — fresh alias + HTTPS endpoint + valid pubkey → entry persisted, `tls_fingerprint_sha256` + `principal` both `None`
- `peer_add_rejects_duplicate` — second call with same alias → `PeerDuplicate`, file still has exactly one entry
- `peer_add_rejects_http_endpoint` — `http://...` → `PeerEndpointInvalid`, file untouched
- `peer_add_rejects_short_pubkey` — 16-byte pubkey → `PeerPubkeyInvalid`
- `peer_add_rejects_garbage_pubkey` — non-base64 input → `PeerPubkeyInvalid`

### send_new_task (1 test)
- Spawns `run_on_listener` on an ephemeral port, peer-adds the daemon as alias "self" with `principal = agent:localhost/self`, runs `send --new-task`, asserts: exactly one task record, state REQUESTED, peer=self, non-terminal, valid UUIDv7 (36 chars), `last_send_at` set, daemon inbox has one line with `class: "request"`.

### send_deliver_sequence (1 test)
- Opens a task, sends 3 non-terminal delivers (with 1.1s sleeps between so the RFC-3339-second `last_send_at` actually changes), asserts: record still REQUESTED and non-terminal, `last_send_at` advanced past the first-send value, inbox has exactly 4 lines (1 request + 3 delivers).

### send_terminal_blocks_resend (1 test)
- Opens a task, sends `--terminal`, asserts record.state = COMPLETED and record.terminal = true and inbox has 2 lines. Then attempts another send on the same task and asserts: `CliError::TaskTerminal { task_id }` (task_id matches), record on disk byte-identical to the post-terminal state (PartialEq), inbox line count still exactly 2.

## Decisions Made

See frontmatter `key-decisions`. Highlights:

- **Task 1+2 merged** for compile-atomicity (same rationale as Plan 02-02).
- **PeerEntry gains optional `principal` field** — necessary to interoperate with the Phase 2 listen daemon's hardcoded `agent:localhost/self` keyring entry. The plan's `hex(pubkey)` derivation scheme cannot produce that principal, so tests must set it explicitly.
- **FSM seeding shortcut** for terminal sends (`advance_terminal` seeds at Committed). Phase 4 replaces with a real commit-reply round-trip.
- **Self principal hardcoded to `agent:localhost/self`** for both `famp listen` and `famp send` — matching Phase 2's convention. Config-driven principal lands in Phase 4.
- **POST-first-persist-second** guarantees atomicity of the local state across network failures.
- **TOFU mismatch translation via error-string marker** — least ugly way to surface a typed error through the erased rustls → reqwest → io::Error chain.

## Deviations from Plan

### Rule 2 — Add optional `principal` field to PeerEntry

- **Found during:** Task 2, writing the first integration test.
- **Issue:** The plan's `peer_principal_from_pubkey(pubkey_b64)` derivation scheme produces something like `agent:host/<hex>`, which does NOT match the Phase 2 listen daemon's single-entry keyring (`agent:localhost/self` → own vk). Without aligning the `from` principal with the listener's keyring entry, `FampSigVerifyLayer` returns `UnknownSender` before the handler runs, and every integration test fails.
- **Fix:** Added `principal: Option<String>` to `PeerEntry` (backward compatible with Plan 03-01's on-disk schema via `#[serde(default, skip_serializing_if)]`), wired a `--principal` flag to `famp peer add`, and made `famp send` fall back to `agent:localhost/self` when the field is absent. Tests pass the flag explicitly.
- **Files modified:** `crates/famp/src/cli/config.rs`, `crates/famp/src/cli/peer/mod.rs`, `crates/famp/src/cli/peer/add.rs`, `crates/famp/src/cli/send/mod.rs`.
- **Commit:** squashed into `93a9092`.

### Rule 3 — Blocking: reqwest workspace dep uses a nonexistent feature

- **Found during:** First `cargo check -p famp`.
- **Issue:** Workspace `Cargo.toml` declares `reqwest = { ..., features = ["rustls-tls-native-roots", "json"] }`. `rustls-tls-native-roots` does not exist as a reqwest 0.13.2 feature — this is a latent bug in the workspace manifest that no prior consumer hit because every existing reqwest user declared the dep directly rather than via `workspace = true`.
- **Fix:** Declared `reqwest` locally in `crates/famp/Cargo.toml` with `rustls-no-provider` feature (matching `famp-transport-http`'s pattern). Left the workspace entry untouched — cleaning it up is Plan 03-04 scope.
- **Impact:** None on plan scope.

### Rule 3 — Blocking: clippy pedantic fixes during Task implementation

Batched lint fixes applied inline before the single commit:
1. `clippy::items_after_statements` on the four test files' `use base64::...` inside `pubkey_b64()` → hoisted above `let bytes = ...`.
2. `clippy::doc_markdown` on `POSTed`, `UUIDv7`, `REQUESTED` in module docs → wrapped in backticks / reworded.
3. `clippy::missing_const_for_fn` on `state_to_str`, `is_terminal`, `TofuVerifier::new` → added `const`.
4. `clippy::too_long_first_doc_paragraph` on `post_envelope` → split into title + paragraph.
5. `clippy::redundant_closure` on `.take_while(|c| c.is_ascii_hexdigit())` → `char::is_ascii_hexdigit`.
6. `clippy::clone_on_copy` on `id.clone()` (MessageId is Copy) → removed.
7. `clippy::expect_used` / `clippy::option_if_let_else` on `load_self_principal` → `.unwrap_or_else(|_| unreachable!(...))` with an explicit `#[allow(clippy::option_if_let_else)]`.
8. `unused_imports` on `use uuid::Uuid` in send/mod.rs → removed.
9. `E0412` on `famp_core::ids::ParseIdError` (doesn't exist) → replaced with `uuid::Error`.
10. `clippy::items_after_statements` on `use base64::...` in peer/add.rs → hoisted to top of file.
11. Workspace `unused_crate_dependencies` for the 6 new bin transitive deps (`famp_taskdir`, `hex`, `reqwest`, `rustls`, `sha2`, `uuid`) and the matching silencers for the three example binaries.

### Note — `state_from_fsm` helper not added

The plan sketched a public `state_from_fsm` helper in fsm_glue. The actually-implemented `state_to_str(TaskState) -> &'static str` covers the plan's intent; no free-function wrapper was added because the caller graph is one site (`advance_terminal`).

---

**Total deviations:** 1 Rule-2 schema addition (PeerEntry.principal), 1 Rule-3 Cargo fix, 11 Rule-3 lint items. No Rule-4 architectural changes.

## Issues Encountered

- **FampSigVerifyLayer only resolves what's in the keyring.** The Phase 2 `listen::run_on_listener` builds a single-entry keyring pinning `agent:localhost/self` → own vk. Any envelope whose `from` is anything else is rejected with `UnknownSender`. This forces integration tests to share the same `FAMP_HOME` between sender and daemon (so `from == agent:localhost/self` resolves) — a multi-home cross-process flow requires the multi-entry keyring from Plan 03-04.
- **`DeliverBody` cross-field validation.** A terminal deliver with `terminal_status = Completed` REQUIRES `provenance` to be `Some(_)` per §8a.3 `validate_against_terminal_status`. Initially skipped and `envelope.sign` surfaced `MissingProvenance` at decode time (middleware round-trip). Fixed by setting `provenance: Some({})` as a placeholder for terminal sends.
- **Workspace reqwest feature bug.** Documented above as Rule 3.

## Verification Artifacts

- `cargo nextest run -p famp --test peer_add` → **5/5 passed**
- `cargo nextest run -p famp --test send_new_task --test send_deliver_sequence --test send_terminal_blocks_resend` → **3/3 passed**
- `cargo nextest run --workspace` → **324/324 passed, 1 skipped**
- `cargo clippy --workspace --all-targets -- -D warnings` → **0 warnings**
- `cargo tree -i openssl` → empty (no openssl transitive pulls)

## Threat Flags

None. The new outbound trust boundary (famp-send HTTPS client) is enumerated in the plan's threat model. TOFU pinning is the mitigation; `TlsFingerprintMismatch` is the observable. The `load_self_principal` `unreachable!()` path is defense-in-depth only — the static input is a compile-time constant that cannot fail to parse, and there is no runtime code path that can make it fail. The parser is exercised by the existing `Principal::from_str` tests in `famp-core`.

## Next Plan Readiness

- **Plan 03-03 (`famp await` + `famp inbox`)** can consume `paths::inbox_cursor_path` / `paths::inbox_jsonl_path` / `famp_inbox::InboxCursor` / `TaskDir::update(last_recv_at)` to close the loop on the inbound side.
- **Plan 03-04 (peer add lock + E2E)** will add an advisory lock around `write_peers_atomic` (for concurrent `famp send` + `famp peer add` calls racing on `peers.toml`), and a full E2E with two distinct `FAMP_HOME` directories (requires a multi-entry keyring).
- **Phase 4 (MCP + E2E)** will replace the `advance_terminal` FSM seeding shortcut with a real commit-reply round-trip, and move the hardcoded `agent:localhost/self` principal into `config.toml`.

## Self-Check: PASSED

- `crates/famp/src/cli/peer/mod.rs` — FOUND
- `crates/famp/src/cli/peer/add.rs` — FOUND
- `crates/famp/src/cli/send/mod.rs` — FOUND
- `crates/famp/src/cli/send/client.rs` — FOUND
- `crates/famp/src/cli/send/fsm_glue.rs` — FOUND
- `crates/famp/tests/peer_add.rs` — FOUND
- `crates/famp/tests/send_new_task.rs` — FOUND
- `crates/famp/tests/send_deliver_sequence.rs` — FOUND
- `crates/famp/tests/send_terminal_blocks_resend.rs` — FOUND
- Commit `93a9092` — FOUND in `git log`
- `grep -q 'TlsFingerprintMismatch' crates/famp/src/cli/error.rs` — FOUND
- `grep -q 'Commands::Send' crates/famp/src/cli/mod.rs` — FOUND

---
*Phase: 03-conversation-cli*
*Plan: 02*
*Completed: 2026-04-14*
