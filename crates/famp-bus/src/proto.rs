//! Bus protocol messages and replies.

use std::{fmt, sync::LazyLock};

use regex::Regex;
use serde::{
    de::{Error as DeError, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::BusErrorKind;

const CHANNEL_PATTERN: &str = "^#[a-z0-9][a-z0-9_-]{0,31}$";
static CHANNEL_RE: LazyLock<Regex> = LazyLock::new(|| match Regex::new(CHANNEL_PATTERN) {
    Ok(regex) => regex,
    Err(err) => panic!("channel regex failed to compile: {err}"),
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClientId(pub u64);

impl From<u64> for ClientId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AwaitFilter {
    Any,
    Task(uuid::Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Agent { name: String },
    Channel { name: String },
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Kind,
            Name,
        }

        struct TargetVisitor;

        impl<'de> Visitor<'de> for TargetVisitor {
            type Value = Target;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a target object with kind and name")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut kind: Option<String> = None;
                let mut name: Option<String> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Kind => {
                            if kind.is_some() {
                                return Err(DeError::duplicate_field("kind"));
                            }
                            kind = Some(map.next_value()?);
                        }
                        Field::Name => {
                            if name.is_some() {
                                return Err(DeError::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                    }
                }
                let kind = kind.ok_or_else(|| DeError::missing_field("kind"))?;
                let name = name.ok_or_else(|| DeError::missing_field("name"))?;
                match kind.as_str() {
                    "agent" => Ok(Target::Agent { name }),
                    "channel" if CHANNEL_RE.is_match(&name) => Ok(Target::Channel { name }),
                    "channel" => Err(DeError::custom(format!(
                        "channel name must match {CHANNEL_PATTERN}"
                    ))),
                    _ => Err(DeError::unknown_variant(&kind, &["agent", "channel"])),
                }
            }
        }

        deserializer.deserialize_map(TargetVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BusMessage {
    Hello {
        bus_proto: u32,
        client: String,
        // D-10: optional proxy binding. `Some(name)` = this connection
        // acts as a read/write-through proxy to the canonical live
        // registered holder of `name`. `None` = normal unbound connection
        // (must `Register` before identity-required ops).
        // `skip_serializing_if = Option::is_none` + `default` preserves
        // BUS-02 byte-exact round-trip when the field is None.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bind_as: Option<String>,
    },
    Register {
        name: String,
        pid: u32,
    },
    Send {
        to: Target,
        envelope: serde_json::Value,
    },
    Inbox {
        // BL-04: `default + skip_serializing_if` together preserves
        // BUS-02 byte-exact round-trip when the field is None AND
        // accepts a wire form that omits the field. Match the locked
        // pattern used by `Hello.bind_as` (see comment on that field).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_terminal: Option<bool>,
    },
    Await {
        timeout_ms: u64,
        // BL-04: see Inbox above.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task: Option<uuid::Uuid>,
    },
    Join {
        channel: String,
    },
    Leave {
        channel: String,
    },
    Sessions {},
    Whoami {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BusReply {
    HelloOk {
        bus_proto: u32,
    },
    HelloErr {
        kind: BusErrorKind,
        message: String,
    },
    // D-09: `drained` is `Vec<serde_json::Value>` on the wire to preserve
    // BUS-02/BUS-03 canonical-JSON round-trip, but the broker MUST
    // type-validate each line via `AnyBusEnvelope::decode` before inserting
    // into this Vec. Decode failure emits `Err{EnvelopeInvalid}` and aborts
    // cursor advance for that drain.
    RegisterOk {
        active: String,
        drained: Vec<serde_json::Value>,
        peers: Vec<String>,
    },
    SendOk {
        task_id: uuid::Uuid,
        delivered: Vec<Delivered>,
    },
    InboxOk {
        envelopes: Vec<serde_json::Value>,
        next_offset: u64,
    },
    AwaitOk {
        envelope: serde_json::Value,
    },
    AwaitTimeout {},
    JoinOk {
        channel: String,
        members: Vec<String>,
        drained: Vec<serde_json::Value>,
    },
    LeaveOk {
        channel: String,
    },
    SessionsOk {
        rows: Vec<SessionRow>,
    },
    WhoamiOk {
        // BL-04: see BusMessage::Inbox above.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active: Option<String>,
        joined: Vec<String>,
    },
    Err {
        kind: BusErrorKind,
        message: String,
    },
}

/// Per-target delivery row in [`BusReply::SendOk`].
///
/// - `ok` — the broker accepted the bytes for this target's mailbox
///   (i.e. `AppendMailbox` succeeded). It does NOT mean the
///   recipient observed the message.
/// - `woken` — at the moment the message landed, a `famp_await`
///   was parked for this target and was woken with `AwaitOk`.
///   `false` means the message is sitting in the mailbox awaiting
///   a future `Inbox`/`Await` from the recipient (offline /
///   crashed / not currently listening).
///
/// Wire compat: `woken` is `#[serde(default)]` so frames produced
/// by pre-`woken` peers deserialize with `woken = false`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered { // woken field is serde-defaulted for wire compatibility.
    pub to: Target,
    pub ok: bool,
    #[serde(default)]
    pub woken: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRow {
    pub name: String,
    pub pid: u32,
    pub joined: Vec<String>,
}

// #[serde(deny_unknown_fields)] for Target agent variant.
// #[serde(deny_unknown_fields)] for Target channel variant.
// #[serde(deny_unknown_fields)] for BusMessage variant payloads.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{BusMessage, BusReply, Delivered, SessionRow, Target};
    use crate::BusErrorKind;
    use serde_json::json;

    #[test]
    fn roundtrip_busmessage() {
        let v = BusMessage::Send {
            to: Target::Channel {
                name: "#good".into(),
            },
            envelope: json!({"body": "hello"}),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_busreply() {
        let v = BusReply::SendOk {
            task_id: uuid::Uuid::nil(),
            delivered: vec![Delivered { // woken set below.
                to: Target::Agent {
                    name: "alice".into(),
                },
                ok: true,
                woken: false,
            }],
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn delivered_back_compat_no_woken_field_deserializes() {
        let bytes = br#"{"ok":true,"to":{"kind":"agent","name":"alice"}}"#;
        let decoded: Delivered = famp_canonical::from_slice_strict(bytes).unwrap();
        assert_eq!(
            decoded,
            Delivered { // woken defaults false when omitted on the wire.
                to: Target::Agent {
                    name: "alice".into()
                },
                ok: true,
                woken: false,
            }
        );
    }

    #[test]
    fn delivered_with_woken_round_trips() {
        let delivered = Delivered { // woken set below.
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: true,
        };
        let bytes = famp_canonical::canonicalize(&delivered).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            r#"{"ok":true,"to":{"kind":"agent","name":"alice"},"woken":true}"#
        );
        let decoded: Delivered = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(delivered, decoded);
    }

    #[test]
    fn channel_regex_accepts_good() {
        let target: Target = serde_json::from_value(json!({
            "kind": "channel",
            "name": "#good_1"
        }))
        .unwrap();
        assert_eq!(
            target,
            Target::Channel {
                name: "#good_1".into()
            }
        );
    }

    #[test]
    fn channel_regex_rejects_bad_caps() {
        let err = serde_json::from_value::<Target>(json!({
            "kind": "channel",
            "name": "BadCaps"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("channel name must match"));
    }

    #[test]
    fn channel_regex_rejects_missing_hash() {
        let err = serde_json::from_value::<Target>(json!({
            "kind": "channel",
            "name": "good"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("channel name must match"));
    }

    #[test]
    fn channel_regex_rejects_overlong() {
        let err = serde_json::from_value::<Target>(json!({
            "kind": "channel",
            "name": "#abcdefghijklmnopqrstuvwxyzabcdefg"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("channel name must match"));
    }

    #[test]
    fn busreply_sessions_roundtrips() {
        let v = BusReply::SessionsOk {
            rows: vec![SessionRow {
                name: "alice".into(),
                pid: 1234,
                joined: vec!["#good".into()],
            }],
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn error_reply_roundtrips() {
        let v = BusReply::Err {
            kind: BusErrorKind::Internal,
            message: "boom".into(),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    /// D-10: `Hello { bind_as: None }` serializes byte-identical to the
    /// pre-D-10 `Hello { bus_proto, client }` shape via
    /// `skip_serializing_if = Option::is_none`. This pins the
    /// BUS-02 round-trip property so a wire frame produced by a v0.5.2
    /// agent (no `bind_as` field) round-trips through a v0.5.2+D-10
    /// implementation byte-for-byte.
    #[test]
    fn hello_bind_as_none_byte_identical_to_pre_d10() {
        let with_field = BusMessage::Hello {
            bus_proto: 1,
            client: "alice".into(),
            bind_as: None,
        };
        let bytes = famp_canonical::canonicalize(&with_field).unwrap();
        // Pre-D-10 shape would canonicalize identically since the missing
        // optional field is skipped on serialize. Expected canonical form:
        // {"bus_proto":1,"client":"alice","op":"hello"}
        let expected = br#"{"bus_proto":1,"client":"alice","op":"hello"}"#;
        assert_eq!(bytes.as_slice(), &expected[..]);
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(with_field, decoded);
    }

    /// D-10: `Hello { bind_as: Some(name) }` round-trips with the new
    /// field present in canonical form (alphabetical key order).
    #[test]
    fn hello_bind_as_some_round_trips() {
        let v = BusMessage::Hello {
            bus_proto: 1,
            client: "alice".into(),
            bind_as: Some("bob".into()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        // Canonical (RFC 8785) JSON sorts keys alphabetically:
        // bind_as < bus_proto < client < op
        let expected = br#"{"bind_as":"bob","bus_proto":1,"client":"alice","op":"hello"}"#;
        assert_eq!(bytes.as_slice(), &expected[..]);
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    /// D-10: a v0.5.2 frame with no `bind_as` field still deserializes
    /// (via `serde(default)`) to `Hello { bind_as: None }`.
    #[test]
    fn hello_pre_d10_frame_deserializes_with_default_none() {
        let pre_d10 = br#"{"bus_proto":1,"client":"alice","op":"hello"}"#;
        let decoded: BusMessage = famp_canonical::from_slice_strict(pre_d10).unwrap();
        assert_eq!(
            decoded,
            BusMessage::Hello {
                bus_proto: 1,
                client: "alice".into(),
                bind_as: None,
            }
        );
    }
}
