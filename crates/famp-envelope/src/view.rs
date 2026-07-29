//! Pre-verification structural envelope view.
//!
//! `famp-envelope` defines typed envelopes, but the *read* path (inspect-server,
//! CLI/MCP surfaces, runtime glue) never gets to use them: the full
//! [`crate::SignedEnvelope::decode`] requires a `TrustedVerifyingKey` *up-front*
//! (you need the sender to look up the key — the two-phase decode documented in
//! [`crate::peek`]). The local bus deliberately skips that keyring lookup, so
//! every reader fell back to raw `value.get("from").and_then(as_str)` poking —
//! re-encoding the wire field names and parse rules at 22 sites across 9 files.
//!
//! [`EnvelopeView`] is the single source of truth for that pre-verification
//! read path. It is a **borrowing** wrapper over an already-parsed
//! `&serde_json::Value` — zero-copy, so the dominant cluster
//! (`famp-inspect-server`, which already threads `&Value` through
//! `message_row` / `envelope_task_id` / `derive_fsm_state`) swaps in without a
//! clone. Byte-holders (`peek_sender`, the transport sig-verify middleware)
//! reach it through [`OwnedEnvelopeView::parse`], which performs the strict,
//! duplicate-key-rejecting `from_slice_strict` parse once.
//!
//! HARD INVARIANTS:
//! - **No signature verification.** This is a *structural* view; it never takes
//!   a `TrustedVerifyingKey`. The whole point is reading fields before/without
//!   the keyring lookup the local path skips.
//! - **Parse-only, never re-encode.** Accessors borrow out of the parsed Value;
//!   nothing here serializes (matches `famp-inbox`'s bytes-signed = bytes-stored
//!   invariant).

use crate::error::EnvelopeDecodeError;
use famp_canonical::from_slice_strict;
use famp_core::Principal;
use serde_json::Value;
use std::str::FromStr;

/// New-task marker `body.event` value used by the `task_id` derivation.
const NEW_TASK_EVENT: &str = "famp.send.new_task";

/// A borrowing, pre-verification structural view over a parsed envelope.
///
/// Wraps a `&serde_json::Value` (an already-parsed envelope object) and exposes
/// typed accessors for the wire fields readers poke across the codebase. Holds
/// no owned state and performs **no** signature verification.
///
/// Construct directly from a borrowed Value with [`EnvelopeView::new`] (the
/// inspect-server path, which already holds `&Value`), or from wire bytes via
/// [`OwnedEnvelopeView::parse`] then [`OwnedEnvelopeView::view`] (the
/// byte-holder path).
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeView<'a> {
    value: &'a Value,
}

impl<'a> EnvelopeView<'a> {
    /// Wrap an already-parsed envelope Value. No validation, no verification.
    #[must_use]
    pub const fn new(value: &'a Value) -> Self {
        Self { value }
    }

    /// The raw `from` field as a string slice, if present and a string.
    ///
    /// Mirrors the inspect-server `.get("from").and_then(as_str)` extraction
    /// exactly: a present-but-non-string or absent field yields `None`. Use
    /// this where the raw on-wire string is wanted even when it is not a valid
    /// [`Principal`].
    #[must_use]
    pub fn from_str(&self) -> Option<&'a str> {
        self.value.get("from").and_then(Value::as_str)
    }

    /// The raw `to` field as a string slice, if present and a string.
    #[must_use]
    pub fn to_str(&self) -> Option<&'a str> {
        self.value.get("to").and_then(Value::as_str)
    }

    /// The `from` field parsed as a [`Principal`].
    ///
    /// `None` if the field is absent, not a string, or not a parseable
    /// principal. This is the accessor `peek_sender` delegates to.
    #[must_use]
    pub fn from(&self) -> Option<Principal> {
        self.from_str().and_then(|s| Principal::from_str(s).ok())
    }

    /// The `to` field parsed as a [`Principal`].
    #[must_use]
    pub fn to(&self) -> Option<Principal> {
        self.to_str().and_then(|s| Principal::from_str(s).ok())
    }

    /// The `class` field (message kind) as a string slice, if present.
    ///
    /// NOTE: the wire field is `class`, not `kind`. The refactoring review
    /// referred to this loosely as `kind()`; naming the accessor after the
    /// actual field avoids reintroducing the name/field indirection this view
    /// exists to eliminate. The `body.event` sub-kind read by `inbox.rs` stays
    /// reachable through [`EnvelopeView::body`].
    #[must_use]
    pub fn class(&self) -> Option<&'a str> {
        self.value.get("class").and_then(Value::as_str)
    }

    /// The `body` field as a raw Value, if present.
    ///
    /// Returned untyped because `body` is polymorphic on the wire: some
    /// surfaces read it as a string (`register.rs`), others as an object with
    /// `event` / `details` (`inbox.rs`, inspect-server). Callers project
    /// further from here.
    #[must_use]
    pub fn body(&self) -> Option<&'a Value> {
        self.value.get("body")
    }

    /// Derive the task id this envelope refers to.
    ///
    /// Mirrors `famp-inspect-server`'s `envelope_task_id` derivation exactly so
    /// the wave-2 migration is byte-for-byte equivalent. Resolution order:
    /// 1. `causality.ref`
    /// 2. `body.details.task`
    /// 3. the envelope `id`, iff `body.event == "famp.send.new_task"`
    /// 4. `None`
    #[must_use]
    pub fn task_id(&self) -> Option<String> {
        if let Some(task_id) = self
            .value
            .get("causality")
            .and_then(|c| c.get("ref"))
            .and_then(Value::as_str)
        {
            return Some(task_id.to_string());
        }
        if let Some(task_id) = self
            .value
            .get("body")
            .and_then(|b| b.get("details"))
            .and_then(|d| d.get("task"))
            .and_then(Value::as_str)
        {
            return Some(task_id.to_string());
        }
        if self
            .value
            .get("body")
            .and_then(|b| b.get("event"))
            .and_then(Value::as_str)
            == Some(NEW_TASK_EVENT)
        {
            return self
                .value
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        // 4. A task-root `request` IS the task: its own `id` is the task id.
        //
        // Branch 3 only fires for the LOCAL send shape, whose body carries the
        // `famp.send.new_task` event marker. A remote `--new-task` (Phase 11
        // ADDR-02) emits a typed `RequestBody` instead — `{bounds,
        // natural_language_summary, scope}` — with no `event` key, so branches
        // 1-3 all miss and an OPEN remote task was invisible to
        // `famp inspect tasks` until a threaded reply arrived to supply
        // `causality.ref`.
        //
        // Resolving a `request` to its own `id` is not a guess: the replies
        // that continue the task set `causality.ref` to exactly that `id`
        // (verified end-to-end in the UAT-01 two-machine dogfood), so this
        // branch and branch 1 agree on the same task id by construction.
        if self.class() == Some("request") {
            return self
                .value
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        None
    }

    /// Escape hatch: the underlying parsed Value, for fields not yet promoted
    /// to a typed accessor (e.g. `ts`, `causality`, `authority`).
    #[must_use]
    pub const fn value(&self) -> &'a Value {
        self.value
    }
}

/// An owned holder for the strict, duplicate-key-rejecting bytes → Value parse.
///
/// Byte-holders (`peek_sender`, the transport sig-verify middleware) parse wire
/// bytes once here, then borrow an [`EnvelopeView`] via [`OwnedEnvelopeView::view`].
/// This is the only place the bytes → Value step (and thus duplicate-key
/// rejection, a property of the parse, not of the accessors) lives.
#[derive(Debug, Clone)]
pub struct OwnedEnvelopeView {
    value: Value,
}

impl OwnedEnvelopeView {
    /// Strictly parse wire bytes (duplicate-key-rejecting per `famp-canonical`).
    /// Performs NO signature verification.
    ///
    /// # Errors
    /// [`EnvelopeDecodeError::MalformedJson`] if the bytes are not a valid,
    /// duplicate-key-free JSON document.
    pub fn parse(bytes: &[u8]) -> Result<Self, EnvelopeDecodeError> {
        // from_slice_strict returns famp_canonical::CanonicalError on failure;
        // EnvelopeDecodeError::MalformedJson wraps that via #[from].
        let value: Value = from_slice_strict(bytes)?;
        Ok(Self { value })
    }

    /// Borrow a structural view over the parsed envelope.
    #[must_use]
    pub const fn view(&self) -> EnvelopeView<'_> {
        EnvelopeView::new(&self.value)
    }

    /// The owned parsed Value, for callers that need to thread it onward.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- accessor: from / to (raw + parsed) ---

    #[test]
    fn from_and_to_parse_as_principals() {
        let v = json!({"from": "agent:local/alice", "to": "agent:local/bob"});
        let view = EnvelopeView::new(&v);
        assert_eq!(view.from_str(), Some("agent:local/alice"));
        assert_eq!(view.to_str(), Some("agent:local/bob"));
        assert_eq!(view.from().unwrap().to_string(), "agent:local/alice");
        assert_eq!(view.to().unwrap().to_string(), "agent:local/bob");
    }

    #[test]
    fn missing_from_and_to_yield_none() {
        let v = json!({"class": "ack"});
        let view = EnvelopeView::new(&v);
        assert_eq!(view.from_str(), None);
        assert_eq!(view.to_str(), None);
        assert_eq!(view.from(), None);
        assert_eq!(view.to(), None);
    }

    #[test]
    fn non_string_from_yields_none() {
        let v = json!({"from": 42});
        let view = EnvelopeView::new(&v);
        assert_eq!(view.from_str(), None);
        assert_eq!(view.from(), None);
    }

    #[test]
    fn unparseable_principal_keeps_raw_str_but_none_typed() {
        // Behaviour-preserving for inspect-server, which shows the raw string
        // even when it is not a valid principal (`.as_str().unwrap_or("")`).
        let v = json!({"from": "not a principal"});
        let view = EnvelopeView::new(&v);
        assert_eq!(view.from_str(), Some("not a principal"));
        assert_eq!(view.from(), None);
    }

    // --- accessor: class ---

    #[test]
    fn class_reads_wire_class_field() {
        let v = json!({"class": "deliver"});
        assert_eq!(EnvelopeView::new(&v).class(), Some("deliver"));
        let v2 = json!({"from": "agent:local/a"});
        assert_eq!(EnvelopeView::new(&v2).class(), None);
    }

    // --- accessor: body ---

    #[test]
    fn body_returns_raw_value_object_or_string() {
        let v = json!({"body": {"event": "famp.send.new_task"}});
        assert_eq!(
            EnvelopeView::new(&v).body(),
            Some(&json!({"event": "famp.send.new_task"}))
        );
        let v2 = json!({"body": "a plain string body"});
        assert_eq!(
            EnvelopeView::new(&v2).body(),
            Some(&json!("a plain string body"))
        );
        let v3 = json!({"from": "agent:local/a"});
        assert_eq!(EnvelopeView::new(&v3).body(), None);
    }

    // --- accessor: task_id (the load-bearing derivation) ---

    #[test]
    fn task_id_prefers_causality_ref() {
        let v = json!({
            "causality": {"ref": "task-from-causality"},
            "body": {"details": {"task": "task-from-details"}, "event": "famp.send.new_task"},
            "id": "the-id",
        });
        assert_eq!(
            EnvelopeView::new(&v).task_id(),
            Some("task-from-causality".to_string())
        );
    }

    #[test]
    fn task_id_falls_back_to_body_details_task() {
        let v = json!({
            "body": {"details": {"task": "task-from-details"}, "event": "famp.send.new_task"},
            "id": "the-id",
        });
        assert_eq!(
            EnvelopeView::new(&v).task_id(),
            Some("task-from-details".to_string())
        );
    }

    #[test]
    fn task_id_uses_id_for_new_task_event_but_not_other_events() {
        // NB: these fixtures carry no `class`, so the Phase 11 task-root
        // `request` branch is deliberately not in play here — this test is
        // scoped to the `body.event` branch alone. Request-class resolution
        // is covered by `remote_request_root_resolves_task_id_from_its_own_id`.
        let new_task = json!({"body": {"event": "famp.send.new_task"}, "id": "the-id"});
        assert_eq!(
            EnvelopeView::new(&new_task).task_id(),
            Some("the-id".to_string())
        );

        let other_event = json!({"body": {"event": "famp.send.reply"}, "id": "the-id"});
        assert_eq!(EnvelopeView::new(&other_event).task_id(), None);
    }

    #[test]
    fn task_id_none_when_nothing_matches() {
        let v = json!({"from": "agent:local/a", "body": {"foo": "bar"}});
        assert_eq!(EnvelopeView::new(&v).task_id(), None);
    }

    // --- OwnedEnvelopeView: strict parse entry ---

    #[test]
    fn owned_parse_then_view_round_trips_accessors() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob","class":"ack"}"#;
        let owned = OwnedEnvelopeView::parse(bytes).expect("parse");
        let view = owned.view();
        assert_eq!(view.from().unwrap().to_string(), "agent:local/alice");
        assert_eq!(view.to().unwrap().to_string(), "agent:local/bob");
        assert_eq!(view.class(), Some("ack"));
    }

    #[test]
    fn owned_parse_rejects_malformed_json() {
        let err = OwnedEnvelopeView::parse(br#"{"from": }"#).unwrap_err();
        assert!(matches!(err, EnvelopeDecodeError::MalformedJson(_)));
    }

    #[test]
    fn owned_parse_rejects_duplicate_keys() {
        // Duplicate-key rejection is a property of the bytes -> Value strict
        // parse, mirrored from peek.rs's existing test.
        let bytes = br#"{"from":"agent:local/alice","from":"agent:local/eve"}"#;
        let err = OwnedEnvelopeView::parse(bytes).unwrap_err();
        assert!(matches!(err, EnvelopeDecodeError::MalformedJson(_)));
    }

    // --- equivalence: prove the view matches the raw extractors it replaces ---

    /// Independent re-implementation of the task-id derivation, so the
    /// property test below pins `view.task_id()` to a spelled-out reference
    /// rather than to itself.
    ///
    /// Originally a verbatim copy of inspect-server's `envelope_task_id`
    /// (~443-473) to pin the wave-2 migration. Phase 11 (F-A) added the
    /// task-root `request` branch to the real implementation; this reference
    /// is kept in lockstep deliberately — if it is allowed to drift, the
    /// equivalence test below silently degrades into "the old logic still
    /// works on inputs the old logic handled", which is exactly the false
    /// confidence it exists to prevent.
    fn raw_envelope_task_id(env: &Value) -> Option<String> {
        if let Some(task_id) = env
            .get("causality")
            .and_then(|c| c.get("ref"))
            .and_then(Value::as_str)
        {
            return Some(task_id.to_string());
        }
        if let Some(task_id) = env
            .get("body")
            .and_then(|b| b.get("details"))
            .and_then(|d| d.get("task"))
            .and_then(Value::as_str)
        {
            return Some(task_id.to_string());
        }
        if env
            .get("body")
            .and_then(|body| body.get("event"))
            .and_then(Value::as_str)
            == Some("famp.send.new_task")
        {
            return env.get("id").and_then(Value::as_str).map(str::to_string);
        }
        if env.get("class").and_then(Value::as_str) == Some("request") {
            return env.get("id").and_then(Value::as_str).map(str::to_string);
        }
        None
    }

    #[test]
    fn task_id_equivalent_to_raw_inspect_derivation() {
        let corpus = vec![
            json!({"causality": {"ref": "t1"}}),
            json!({"body": {"details": {"task": "t2"}}}),
            json!({"body": {"event": "famp.send.new_task"}, "id": "t3"}),
            json!({"body": {"event": "famp.send.reply"}, "id": "t4"}),
            json!({"from": "agent:local/a"}),
            json!({
                "causality": {"ref": "win"},
                "body": {"details": {"task": "lose"}, "event": "famp.send.new_task"},
                "id": "also-lose"
            }),
            json!({"causality": {"ref": 7}, "body": {"details": {"task": "t6"}}}),
            // Phase 11 F-A: task-root request resolves to its own id...
            json!({"class": "request", "id": "t7"}),
            // ...but causality.ref still outranks it...
            json!({"class": "request", "id": "lose", "causality": {"ref": "win"}}),
            // ...and a non-request with a bare id still resolves to None.
            json!({"class": "deliver", "id": "t8"}),
        ];
        for env in &corpus {
            assert_eq!(
                EnvelopeView::new(env).task_id(),
                raw_envelope_task_id(env),
                "task_id mismatch for {env}"
            );
        }
    }

    #[test]
    fn from_to_equivalent_to_raw_get_as_str() {
        let corpus = vec![
            json!({"from": "agent:local/alice", "to": "agent:local/bob"}),
            json!({"from": "not-a-principal", "to": 42}),
            json!({"class": "ack"}),
            json!({"from": null, "to": "agent:local/bob"}),
        ];
        for env in &corpus {
            let view = EnvelopeView::new(env);
            // Raw extraction the view replaces.
            let raw_from = env.get("from").and_then(Value::as_str);
            let raw_to = env.get("to").and_then(Value::as_str);
            assert_eq!(view.from_str(), raw_from, "from_str mismatch for {env}");
            assert_eq!(view.to_str(), raw_to, "to_str mismatch for {env}");
            // Typed accessor matches raw -> Principal::from_str.
            assert_eq!(
                view.from(),
                raw_from.and_then(|s| Principal::from_str(s).ok()),
                "from mismatch for {env}"
            );
            assert_eq!(
                view.to(),
                raw_to.and_then(|s| Principal::from_str(s).ok()),
                "to mismatch for {env}"
            );
        }
    }

    /// F-A (UAT-01 finding): a task-root `request` resolves to its own `id`.
    ///
    /// This is the VERBATIM envelope captured from bob's mailbox on the Linux
    /// host during the UAT-01 two-machine dogfood — a remote `--new-task` sent
    /// by the shipping `famp send`. Note the typed `RequestBody` shape: no
    /// `causality`, no `body.details.task`, and no `body.event` marker, which
    /// is why every pre-existing branch missed it.
    #[test]
    fn remote_request_root_resolves_task_id_from_its_own_id() {
        let env = json!({
            "authority": "advisory",
            "body": {
                "bounds": {"budget": {"amount": "0", "unit": "usd"}, "hop_limit": 8},
                "natural_language_summary": "UAT-01 phase 11 two-machine dogfood",
                "scope": {}
            },
            "class": "request",
            "famp": "0.5.2",
            "from": "agent:mac.famp/alice",
            "id": "019fab97-d3e0-7d63-92ba-39f1ce171b83",
            "scope": "standalone",
            "to": "agent:devbox.famp/bob",
            "ts": "2026-07-29T01:58:01Z"
        });
        assert_eq!(
            EnvelopeView::new(&env).task_id().as_deref(),
            Some("019fab97-d3e0-7d63-92ba-39f1ce171b83"),
            "an open remote request must be task-indexed by its own id, or \
             `famp inspect tasks` cannot see a pending task until it is answered"
        );
    }

    /// The reply that continues the task must agree with the branch above:
    /// its `causality.ref` is the request's `id`. If these two ever disagree,
    /// one task would show up under two ids.
    #[test]
    fn threaded_reply_causality_ref_matches_the_request_root_id() {
        let root = "019fab97-d3e0-7d63-92ba-39f1ce171b83";
        let request = json!({
            "class": "request", "id": root,
            "body": {"natural_language_summary": "x", "scope": {}}
        });
        let commit = json!({
            "class": "commit", "id": "019fab99-52bf-7770-a36a-cc28c156bc18",
            "causality": {"ref": root, "rel": "commits"}
        });
        assert_eq!(
            EnvelopeView::new(&request).task_id(),
            EnvelopeView::new(&commit).task_id(),
            "request root and its threaded reply must resolve to the SAME task id"
        );
    }

    /// Precedence guard: branch 1 still wins. A `request` that carries an
    /// explicit `causality.ref` must resolve to the ref, NOT to its own id --
    /// otherwise a request threaded into an existing task would fork a second.
    #[test]
    fn request_with_causality_ref_prefers_the_ref_over_its_own_id() {
        let env = json!({
            "class": "request",
            "id": "019fab99-0000-7000-8000-000000000000",
            "causality": {"ref": "019fab97-d3e0-7d63-92ba-39f1ce171b83"}
        });
        assert_eq!(
            EnvelopeView::new(&env).task_id().as_deref(),
            Some("019fab97-d3e0-7d63-92ba-39f1ce171b83")
        );
    }

    /// Negative: the new branch is scoped to `request`. A class that does not
    /// open a task and carries no correlation still resolves to None, so this
    /// change cannot make unrelated envelopes masquerade as tasks.
    #[test]
    fn non_request_without_correlation_still_resolves_to_none() {
        for class in ["audit_log", "ack", "deliver", "commit"] {
            let env = json!({"class": class, "id": "019fab99-1111-7000-8000-000000000000"});
            assert_eq!(
                EnvelopeView::new(&env).task_id(),
                None,
                "class {class} must not self-resolve a task id"
            );
        }
    }
}
