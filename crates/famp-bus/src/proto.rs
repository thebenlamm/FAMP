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
        #[serde(skip_serializing_if = "Option::is_none")]
        since: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        include_terminal: Option<bool>,
    },
    Await {
        timeout_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
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
        #[serde(skip_serializing_if = "Option::is_none")]
        active: Option<String>,
        joined: Vec<String>,
    },
    Err {
        kind: BusErrorKind,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered {
    pub to: Target,
    pub ok: bool,
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
            delivered: vec![Delivered {
                to: Target::Agent {
                    name: "alice".into(),
                },
                ok: true,
            }],
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
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
}
