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
    fn task_id_uses_id_only_for_new_task_event() {
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

    /// Replicates inspect-server's `envelope_task_id` (~443-473) verbatim, so
    /// the property test below pins `view.task_id()` to the exact derivation
    /// the wave-2 migration must preserve.
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
}
