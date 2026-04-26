---
phase: quick-260425-pc7
reviewed: 2026-04-25T00:00:00Z
depth: deep
diff_range: f4d9aed..HEAD (7 commits)
files_reviewed: 13
files_reviewed_list:
  - crates/famp-envelope/src/body/request.rs
  - crates/famp-envelope/tests/scope_more_coming_round_trip.rs
  - crates/famp/src/cli/send/mod.rs
  - crates/famp/src/cli/mcp/server.rs
  - crates/famp/src/cli/mcp/tools/send.rs
  - crates/famp/src/cli/inbox/list.rs
  - crates/famp/tests/common/conversation_harness.rs
  - crates/famp/tests/e2e_two_daemons.rs
  - crates/famp/tests/send_deliver_sequence.rs
  - crates/famp/tests/send_new_task.rs
  - crates/famp/tests/send_new_task_scope_instructions.rs
  - crates/famp/tests/send_principal_fallback.rs
  - crates/famp/tests/send_terminal_advance_error_surfaces.rs
findings:
  blocker: 3
  warning: 6
  total: 9
status: issues_found
---

# Quick 260425-pc7 — Adversarial Code Review

**Reviewed:** 2026-04-25
**Depth:** deep (cross-file: envelope library ↔ CLI ↔ MCP tool ↔ inbox-list)
**Status:** issues_found — 3 BLOCKERs, 6 WARNINGs

## Summary

The change does what it says — adds an opt-in `scope.more_coming: true` JSON convention to `request` envelopes — and the byte-exact backwards-compat property is genuinely preserved in practice (verified by the unmodified `decode_provisional_scope_instructions_vector` fixture test still passing).

But the implementor's "must_haves" verification is **not** what they claim it is. The new `scope_more_coming_round_trip` test does not prove backwards compat with pre-flag signed envelopes; it proves a tautology. Two surfaces (CLI clap `requires`, MCP type validation) leak intent silently. The `inbox list` change emits a misleading field on every entry. Several constants in the new test file are dead, wrong, or both.

Severity rationale: BLOCKER for behaviors that silently drop user intent or violate an established cross-cutting convention; WARNING for misleading docs / dead code / unproven test claims.

---

## BLOCKER

### BL-01 — `--more-coming` with `--task` is silently accepted; user intent is dropped

**File:** `crates/famp/src/cli/send/mod.rs:54-65`, dispatch at `crates/famp/src/cli/send/mod.rs:124-189`

**Issue:**
`SendArgs::more_coming` is declared `#[arg(long, requires = "new_task")]`. The `requires` constraint is supposed to reject `--more-coming` when `--new-task` is absent. Reproduction on the freshly-built binary:

```
$ ./target/debug/famp send --to nobody --task 019f0000-0000-7000-8000-000000000001 --more-coming
peer not found: nobody
exit=1
```

Clap accepts `--more-coming --task <uuid>` (no `--new-task`) without firing `requires`. (Diagnosis: clap-derive does not enforce `requires` against an arg that is itself `conflicts_with` another already-present arg.)

The dispatch in `run_at_structured` then routes to `SendMode::DeliverNonTerminal`, which calls `build_deliver_envelope(...)` — and `args.more_coming` is **never read** on that path. The user thinks they signaled "more briefing follows"; the wire envelope carries no such flag.

For a protocol whose entire point is precise sender intent across federation boundaries, silently dropping a sender-set flag is the wrong default.

**Why it matters:**
- Worst-case: orchestrator agent calls `famp send --task <id> --more-coming` thinking it's pausing the receiver. Receiver receives a normal `deliver`, treats the task as committable, races the wrong way. This is exactly the Gap G4 scenario this task is supposed to fix.
- This is a regression risk for the very Lampert-deck cycle the SUMMARY cites as motivation.

**Fix:**
- Don't rely solely on clap's `requires`. Add an explicit guard in `run_at_structured` (or in the `SendMode` match block at line 124-133):
  ```rust
  if args.more_coming && !matches!(mode, SendMode::NewTask) {
      return Err(CliError::SendArgsInvalid {
          reason: "--more-coming is only valid with --new-task".to_string(),
      });
  }
  ```
- Add a regression test `send_args_invalid_more_coming_without_new_task`.

---

### BL-02 — MCP `more_coming` silently coerces non-bool input to `false`, violating the project's established malformed-input convention

**File:** `crates/famp/src/cli/mcp/tools/send.rs:52`

**Issue:**
```rust
let more_coming = input["more_coming"].as_bool().unwrap_or(false);
```
A caller sending `"more_coming": "true"`, `"more_coming": 1`, `"more_coming": null`, or `"more_coming": {}` gets silent coercion to `false`. The MCP envelope is sent without the flag, the caller has no way to know.

This **directly violates** the precedent set by `famp_inbox_list_rejects_non_bool_include_terminal` (`crates/famp/tests/mcp_malformed_input.rs:134-174`), which is an explicit assertion that boolean MCP fields **must** error with both the field name and the expected type when type-mismatched. The whole reason `include_terminal` rejects non-bool is so MCP clients can self-correct — the same reasoning applies verbatim to `more_coming`.

**Why it matters:**
- MCP callers (LLM agents) construct JSON dynamically. String-vs-bool errors are common. Silent `false` is exactly the failure mode the project already decided to reject elsewhere.
- Inconsistent error semantics across the same MCP tool surface are user-hostile.

**Fix:**
- Require the value to be either absent or a real bool:
  ```rust
  let more_coming = match input.get("more_coming") {
      None | Some(Value::Null) => false,
      Some(Value::Bool(b)) => *b,
      Some(other) => return Err(CliError::SendArgsInvalid {
          reason: format!(
              "famp_send: 'more_coming' must be a boolean (got {})",
              type_name(other)
          ),
      }),
  };
  ```
- Also reject `more_coming` set on `mode != "new_task"` (currently silently ignored — see the comment at `tools/send.rs:49-51`). Silent-ignore on a documented "ignored elsewhere" field is the same anti-pattern.
- Add the symmetric test `famp_send_rejects_non_bool_more_coming` in `mcp_malformed_input.rs`.

---

### BL-03 — Implementor's "must_haves" claim of backwards-compat proof is wrong; the new test is a tautology

**File:** `crates/famp-envelope/tests/scope_more_coming_round_trip.rs:131-153`, SUMMARY line 76-78

**Issue:**
SUMMARY claims:
> [x] Existing signed envelopes (no `more_coming` key) still decode + `verify_strict` cleanly — proven by `more_coming_default_false_is_byte_exact_with_legacy`

That test compares `build_envelope_bytes(None)` to `build_envelope_bytes(Some(false))`. Both invocations take the **same code path** (the `if more_coming == Some(true)` branch is false for both, so `scope_map` insertion is skipped on both). The test therefore proves: "this deterministic builder is deterministic." It does **not** prove anything about envelopes signed by code that pre-dated the `more_coming` constant.

The actual backwards-compat proof exists only **incidentally** in the unmodified `decode_provisional_scope_instructions_vector` test, which loads an on-disk fixture (`tests/fixtures/provisional/request-scope-instructions.json`) that pre-dates pc7 and was not regenerated. That test's continued passing is the real proof.

**Why it matters:**
- This is a protocol crate where the byte-exactness claim is load-bearing. Documenting a tautology as the proof is a trap for the next reviewer / for a future regression: someone changes the on-disk fixture or the field-ordering in `RequestBody`, the named "backwards-compat" test still passes, and the actual breakage hides until the next interop run.
- Worse: the implementor closed the task with `[x]` against this claim, which means the GSD trail now contains a false post-condition.

**Fix:**
1. Either delete `more_coming_default_false_is_byte_exact_with_legacy` (it's not testing what its name says) or re-purpose it to load the on-disk legacy fixture and re-verify it still decodes — the `scope_more_coming_round_trip` test should additionally read `tests/fixtures/provisional/request-scope-instructions.json`, decode it under the same trust anchor, and assert `scope.more_coming == None`.
2. Update SUMMARY's must_have-1 to cite `decode_provisional_scope_instructions_vector` as the actual proof.
3. Consider committing a second on-disk fixture: a request WITH `more_coming: true`, signed once and pinned, so a future canonicalization-library change can't silently break the wire format.

---

## WARNING

### WA-01 — `inbox list` emits `more_coming: false` on every non-request entry, polluting the JSON shape

**File:** `crates/famp/src/cli/inbox/list.rs:108-122`

**Issue:**
The hoist logic only inspects `scope.more_coming` when `class == "request"`, but `more_coming` is always added to the `shaped` JSON regardless of class:
```rust
let more_coming = if class == "request" { ... } else { false };
let shaped = json!({ ..., "more_coming": more_coming, "body": body, });
```

Result: every `deliver`, `ack`, `commit` entry now carries `"more_coming": false` in its JSON. This is misleading (the field has no meaning on non-request classes — a deliver envelope can't carry `more_coming`) and a needless schema expansion for downstream consumers.

**Fix:**
Emit `more_coming` only when it's meaningfully `true`, or only on `request` entries:
```rust
let mut shaped = serde_json::Map::new();
shaped.insert("offset".into(), json!(end_offset));
// ... other fields
if class == "request" {
    let mc = body.pointer("/scope/more_coming")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    shaped.insert("more_coming".into(), json!(mc));
}
shaped.insert("body".into(), body);
```
Or, if a stable schema across classes is preferred, document explicitly that `more_coming` is *always* `false` on non-request and add a test pinning that.

Either way: add a test asserting (a) a request entry with `scope.more_coming: true` surfaces as top-level `"more_coming": true`, and (b) the chosen behavior for non-request entries. Neither exists today.

---

### WA-02 — MCP tool description contains a logical contradiction ("repeat new_task with more_coming=false to terminate briefing")

**File:** `crates/famp/src/cli/mcp/server.rs:48`

**Issue:**
The description says:
> Send subsequent context via famp_send mode=deliver, then a final mode=deliver (or repeat new_task with more_coming=false) when the briefing is complete.

"Repeat new_task with more_coming=false" would create a **new task** (new `task_id`, new conversation), not terminate the previous briefing. The "or" branch is wrong and will mislead the LLM agent reading the tool schema.

**Why it matters:**
The MCP tool description is the agent's API contract. A logical contradiction here costs real tokens / real wrong sends in production.

**Fix:**
Remove the "(or repeat new_task with more_coming=false)" clause. The correct termination signal is "send a deliver envelope without the more_coming flag (or with `interim: false` for terminal)" — exactly what mode=deliver/terminal already does. Re-read the description end-to-end to ensure it tells the agent the *right* thing to do.

---

### WA-03 — Doc-comment claim that `more_coming` "mirrors" `body.interim` is misleading; the wire shapes are not analogous

**File:** `crates/famp-envelope/src/body/request.rs:24-25` (and echoed in `cli/send/mod.rs:61-62`, `mcp/server.rs:48`, the test file header)

**Issue:**
The docstring says `more_coming` "mirrors the `body.interim` flag on `deliver` envelopes." It doesn't:

- `interim` on `DeliverBody` is `pub interim: bool` (no `skip_serializing_if`) — **always** present in canonical bytes; both `true` and `false` are part of the signed payload.
- `more_coming` on `RequestBody.scope` is a JSON-level convention, **omitted** when `false`. `false` is indistinguishable from "not set" on the wire.

These have very different semantics for upgradability and for receivers that do byte-equality on canonical forms. Calling them "mirrors" sells the analogy too hard.

**Fix:**
Soften the doc-comments to: "Semantically the request-side counterpart to `body.interim` on deliver envelopes — but encoded as an opt-in JSON-level convention (omitted when false), not a struct field." Also note in the constant doc that the omit-when-false rule is sender-side convention only — the decoder accepts `false` if explicitly set, but those bytes will not match the canonical "absent" form.

---

### WA-04 — Receiver-side validation gap: `scope.more_coming` accepts non-bool values without error

**File:** `crates/famp-envelope/src/body/request.rs:39-44` (RequestBody::scope is `serde_json::Value`)

**Issue:**
`RequestBody::scope` is typed as `serde_json::Value`, so `#[serde(deny_unknown_fields)]` doesn't apply inside `scope`. A peer can send `"scope": {"more_coming": "yes"}` or `"more_coming": 1` or `"more_coming": {"nested": true}`, and the envelope decodes cleanly. The inbox-list hoist (`as_bool().unwrap_or(false)`) silently treats all of these as `false`.

This means: a buggy or hostile peer can claim "more is coming" without the local agent ever surfacing the discrepancy. The signed bytes carry the malformed value; the local view shows `false`.

**Why it matters:**
This is a smaller version of BL-02 — silent type coercion on signed data. Lower severity here because it's receiver-side observability, not an outgoing protocol violation, but it should be documented at minimum.

**Fix:**
Two reasonable options:
1. Add a `RequestBody::validate()` rule: if `scope.more_coming` exists, it MUST be a bool. Reject the envelope on decode otherwise. (Strict; possibly too strict for a provisional flag.)
2. Add a test (`scope_more_coming_non_bool_does_not_silently_become_false`) and emit a `tracing::warn!` in `inbox list` when the key exists but is not a bool, so observability surfaces malformed-but-signed data.

---

### WA-05 — Dead and invalid constant `FIXED_MESSAGE_ID` in the new test file, suppressed via `let _ =`

**File:** `crates/famp-envelope/tests/scope_more_coming_round_trip.rs:50, 128`

**Issue:**
```rust
const FIXED_MESSAGE_ID: &str = "019f0000-0000-7000-8000-00000000pc70";
```
This string is **not** a valid UUIDv7 — `pc70` contains non-hex characters. If anyone tried to `.parse::<MessageId>()` it, it would panic. The function actually uses a different hardcoded UUID at line 58: `"019f0000-0000-7000-8000-000000000001"`. The `let _ = FIXED_MESSAGE_ID;` at line 128 exists solely to suppress a dead-code warning, with a comment claiming it "documents the intended fixed-id origin even if not used directly."

This is cargo-cult code: the constant is dead, wrong, and suppressed. The "documentation" rationale is false — the fixed-id origin is the literal at line 58, not this dead const. Compare to the well-formed `FIXED_MESSAGE_ID` in the sister `provisional_scope_instructions_vector.rs` test (which IS used and IS valid).

**Fix:**
Delete `FIXED_MESSAGE_ID` and its `let _ =` suppressor. If the intent was to document the task-tag in the UUID, do it via a comment, not an unparseable const masquerading as a UUID.

---

### WA-06 — No e2e test proves the wire envelope or the inbox-list output actually carries `more_coming`

**Files:** all of `crates/famp/tests/*` (no test exercises the CLI path with `--more-coming` end-to-end; no test exercises the MCP path with `more_coming: true` and inspects the resulting envelope; no test seeds an inbox with a `more_coming: true` envelope and asserts `inbox list` output)

**Issue:**
The only round-trip coverage is the in-process `scope_more_coming_round_trip` test that builds and decodes in the same process. Three of the five "must_haves" in SUMMARY are unverified:

- "famp send --new-task --more-coming accepts the flag and `requires=new_task` gates it correctly" — partial: BL-01 shows the gate is broken in one direction. There is no test asserting the wire envelope after `--more-coming` carries `scope.more_coming: true`.
- "famp_send MCP tool accepts more_coming in new_task mode" — there is no test asserting an MCP `tools/call` with `"more_coming": true` results in an envelope on the wire whose `scope.more_coming` is `true`. (The conversation harness sets `more_coming: false` in every call.)
- "famp inbox list exposes more_coming as top-level JSON" — there is no test seeding an inbox file with a `more_coming: true` request and asserting the JSONL output contains `"more_coming":true`.

The implementor's confidence rests on visual inspection; the tests don't carry that load.

**Fix:**
Add three integration tests:
1. `send_new_task_with_more_coming_emits_scope_key`: spawn a local listener, run `send_run_at(home, SendArgs { more_coming: true, ... })`, then read the listener's inbox JSONL and assert the request envelope's `body.scope.more_coming == true`.
2. `mcp_send_passes_more_coming_through_to_envelope`: same but driven through `cli::mcp::tools::send::call` with JSON input containing `"more_coming": true`.
3. `inbox_list_surfaces_more_coming_for_request_entries`: seed an inbox JSONL with one `more_coming: true` request and one normal request, run `run_list`, parse the captured stdout, assert the boolean values.

---

## What I checked and found OK

- **Canonical-byte ordering:** `serde_jcs::to_vec` in `famp-canonical/src/canonical.rs:23-25` always sorts keys per RFC 8785; `serde_json::Map` insertion order does not matter. Adding `more_coming` cannot reorder existing keys. Backwards-compat at the JCS layer is preserved.
- **Pre-existing on-disk fixture:** `tests/fixtures/provisional/request-scope-instructions.json` is unchanged and still verifies via `decode_provisional_scope_instructions_vector`. **This** is the real backwards-compat proof, even though SUMMARY misattributes it (see BL-03).
- **All `SendArgs { ... }` literal sites in `*.rs`:** every one (16 sites in tests, 3 in MCP) is updated with `more_coming`. No examples / benches / doctests exist that construct `SendArgs`. The Python script the implementor used did not miss any sites.
- **`--more-coming` + `--new-task` + `--task` combo:** clap correctly rejects (the `conflicts_with` between `new_task` and `task` fires first). The broken case is BL-01.
- **`build_request_envelope` regression risk:** the rewrite from `body.map_or_else` to a mutable `scope_map` preserves the empty-map shape when neither body nor more_coming is present (tested implicitly by `decode_provisional_scope_instructions_vector` and explicitly by `send_new_task_creates_record_and_hits_daemon`).
- **No snapshot tests / no fixed-schema consumers** of `inbox list` JSON exist that would break on the new field. (WA-01 is about misleading shape, not breaking change.)

---

_Reviewed: 2026-04-25_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
