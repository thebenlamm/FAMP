//! Bus protocol messages and replies.

use std::{fmt, sync::LazyLock};

use regex::Regex;
use serde::{
    de::{Error as DeError, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::origin::{Origin, StampedEnvelope};
use crate::{BusErrorKind, MailboxName};

/// Current local bus protocol version.
///
/// **Bump this ONLY when the wire frame changes** (new message field,
/// removed field, changed semantics). Never bump automatically. Never
/// wire this to `CARGO_PKG_VERSION` — these are separate axes.
///
/// Bumped 1 -> 2 in Phase 14 (QUAR-10, D-09) for the v1.1 quarantine
/// record shape: `Register` gains an `origin` field and
/// `BusReply::InboxOk.envelopes` changes from `Vec<serde_json::Value>`
/// to `Vec<StampedEnvelope>`. Proto-1 clients are rejected at Hello BY
/// DESIGN (not a bug to work around) — an old client cannot render
/// provenance, so serving it anyway would hand unmarked remote content
/// to a client blind to it, which is exactly the fail-open hole QUAR-09
/// exists to close. See `hello()` in `broker/handle.rs` for the reject
/// path and its actionable error message.
///
/// Bumped 2 -> 3 in quick task 260810-hac (D1, native wake ping) for the
/// new `BusMessage::SetWakeAddr` / `BusReply::SetWakeAddrOk` frame pair,
/// which records the Claude Code host session's `SendMessage` address on
/// the canonical holder. A new message variant is a wire frame change per
/// the mandate above. Proto-2 clients are rejected at Hello BY DESIGN —
/// `deny_unknown_fields` on `BusMessage` means a proto-2 peer cannot even
/// parse the new frame, so serving it would produce a decode error at an
/// arbitrary later point instead of an actionable handshake refusal. After
/// `just install`, restart the daemon; every live window re-registers.
/// Mailboxes are durable per name, so nothing queued is lost.
pub const BUS_PROTO_VERSION: u32 = 3;

const CHANNEL_PATTERN: &str = "^#[a-z0-9][a-z0-9_-]{0,31}$";
static CHANNEL_RE: LazyLock<Regex> = LazyLock::new(|| match Regex::new(CHANNEL_PATTERN) {
    Ok(regex) => regex,
    Err(err) => panic!("channel regex failed to compile: {err}"),
});

/// Wake-address shape accepted by the broker's `SetWakeAddr` handler.
///
/// D1/D3 (spec `2026-08-10-native-wake-ping-design.md`): the value is a
/// peer-controlled string — ANY bus client can send this frame — so it is
/// pinned to the exact Claude Code cross-session socket shape and nothing
/// else. Anchored at both ends; `regex` has no implicit multiline mode, so
/// `^`/`$` match string start/end and an embedded newline cannot smuggle a
/// second value past the check.
const WAKE_ADDR_PATTERN: &str = r"^uds:/tmp/cc-socks/[0-9]{1,10}\.sock$";
static WAKE_ADDR_RE: LazyLock<Regex> = LazyLock::new(|| match Regex::new(WAKE_ADDR_PATTERN) {
    Ok(regex) => regex,
    Err(err) => panic!("wake addr regex failed to compile: {err}"),
});

/// True iff `candidate` matches [`WAKE_ADDR_PATTERN`].
///
/// Exposed so the broker's `SetWakeAddr` handler validates through the
/// same single definition the wire type documents. Validation is
/// BROKER-side on purpose: only the broker sees every client's frames.
#[must_use]
pub fn wake_addr_valid(candidate: &str) -> bool {
    WAKE_ADDR_RE.is_match(candidate)
}

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
        /// D-01: client's working directory at registration time.
        /// Captured once; never refreshed (D-02). Optional with
        /// `#[serde(default, skip_serializing_if = "Option::is_none")]`
        /// so pre-v0.10 senders that omit the field continue to
        /// serialize/deserialize byte-exactly under BUS-02.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        /// listen-mode flag from `famp_register({listen: true})`. Default
        /// false. `#[serde(default)]` keeps the field omittable for
        /// pre-v0.10 senders. NOT `skip_serializing_if=false` because
        /// `false` is the wire-default value; ALWAYS serializing
        /// would change the canonical form for pre-v0.10 round-trips.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        listen: bool,
        /// D-01/D-17 (Phase 14): declared provenance for this connection.
        /// `None` means the sender did not declare an origin, and the
        /// broker MUST resolve it to [`Origin::Unknown`] — NEVER
        /// [`Origin::Local`] (D-01 fail-closed polarity). Additive field
        /// following the exact `cwd`/`listen` precedent above: pre-Phase-14
        /// senders that omit this field continue to serialize/deserialize
        /// byte-exactly under BUS-02.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Origin>,
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
        /// Optional self-declared role for this member (e.g. "judge", "peer").
        /// `skip_serializing_if = Option::is_none` preserves byte-exact
        /// round-trip for pre-role senders that omit the field.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
    },
    Leave {
        channel: String,
    },
    Sessions {},
    Whoami {},
    /// v0.10 inspector RPC dispatch entry. `kind` carries the
    /// per-operation Request type. The broker forwards `kind` to
    /// `famp_inspect_server::dispatch(&state, kind, ...)` (Wave 2);
    /// the reply rides back as `BusReply::InspectOk { payload }`.
    ///
    /// Wire shape: `{"kind":{"op":"broker"},"op":"inspect"}` after JCS.
    ///
    /// `deny_unknown_fields` on the parent enum means pre-v0.10 peers
    /// REJECT this frame on receive (the documented failure mode).
    Inspect {
        kind: famp_inspect_proto::InspectKind,
    },
    /// Fix 1 (2026-05-12): update the canonical holder's listen-mode
    /// flag without re-registering. Reply is `BusReply::SetListenOk`.
    ///
    /// Proxy (`bind_as`) connections MUST NOT issue `SetListen` — slot
    /// ownership is canonical-holder-only, mirroring the Register
    /// rejection at `handle::register`. Broker replies `Err{NotRegistered}`.
    SetListen {
        listen: bool,
    },
    /// D1 (260810-hac): record the Claude Code host session's
    /// `SendMessage` address on the canonical holder. Issued by the
    /// `famp_register` MCP tool immediately after a successful
    /// `Register`, and ONLY when the computed socket path exists on
    /// disk. Reply is `BusReply::SetWakeAddrOk`.
    ///
    /// Carried as a separate frame rather than a `Register` field on
    /// purpose: `Register` has ~48 construction sites including the
    /// gateway's remote-principal registration, and a field there would
    /// create a slot the gateway path must remember never to populate.
    /// A distinct frame means only the MCP register tool ever sends one.
    ///
    /// Proxy (`bind_as`) connections MUST NOT issue `SetWakeAddr` — slot
    /// ownership is canonical-holder-only, mirroring the `SetListen`
    /// rejection. Broker replies `Err{NotRegistered}`.
    ///
    /// `wake_addr` is peer-controlled and validated BROKER-side against
    /// [`wake_addr_valid`]; a non-matching value stores nothing and the
    /// reply echoes `None` (fail-open to no-ping, never an error that
    /// would break registration).
    SetWakeAddr {
        wake_addr: Option<String>,
    },
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
    // D-09/D-17 (Phase 14 plan 14-02): `drained` carries `StampedEnvelope`
    // elements, not bare `serde_json::Value`s — each element's `envelope`
    // field is what preserves the BUS-02/BUS-03 canonical-JSON round-trip
    // property the bare-`Value` shape used to carry directly. The broker
    // still type-validates the INNER value via `AnyBusEnvelope::decode`
    // before inserting into this Vec (see `decode_line` in
    // `broker/handle.rs`); decode failure skips that one line
    // (head-of-line resilience, fix 260611) rather than aborting the
    // whole drain.
    RegisterOk {
        active: String,
        drained: Vec<StampedEnvelope>,
        peers: Vec<String>,
    },
    SendOk {
        task_id: uuid::Uuid,
        delivered: Vec<Delivered>,
    },
    // D-17 (Phase 14): `envelopes`/`drained` changed from
    // `Vec<serde_json::Value>` to `Vec<StampedEnvelope>` on all four reply
    // variants that carry drained mailbox content (`InboxOk` in plan
    // 14-01; `AwaitOk`/`RegisterOk`/`JoinOk` in plan 14-02) — every reply
    // that can carry received content now carries the fail-closed
    // provenance stamp per element, closing the gap the original
    // five-surface hand-curated list missed (D-04/D-05).
    InboxOk {
        envelopes: Vec<StampedEnvelope>,
        next_offset: u64,
    },
    AwaitOk {
        envelopes: Vec<StampedEnvelope>,
        mailbox: MailboxName,
        next_offset: u64,
    },
    AwaitTimeout {},
    JoinOk {
        channel: String,
        members: Vec<MemberInfo>,
        drained: Vec<StampedEnvelope>,
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
    /// v0.10 inspector RPC reply. Carried as a `serde_json::Value` to
    /// avoid coupling famp-bus to famp-inspect-proto's reply types
    /// at the `BusReply` layer (the dispatch crate handles the typed
    /// reply on both sides; this layer just shuttles the JSON).
    InspectOk {
        payload: serde_json::Value,
    },
    /// Reply to `BusMessage::SetListen`. Echoes the post-mutation flag
    /// so the client can confirm the broker's view without re-issuing
    /// `Whoami` / `Inspect`.
    SetListenOk {
        listen_mode: bool,
    },
    /// Reply to `BusMessage::SetWakeAddr`. Echoes the POST-VALIDATION
    /// stored value, so a client can observe that a malformed address
    /// stored nothing without inspecting broker internals.
    SetWakeAddrOk {
        wake_addr: Option<String>,
    },
    Err {
        kind: BusErrorKind,
        message: String,
    },
}

impl BusReply {
    /// Snake_case-ish variant name, no payload.
    ///
    /// Phase 14 T-14-08: several call sites used to interpolate an
    /// unexpected `BusReply` via `{:?}` into an error string that flows
    /// into MCP tool results / stderr — after this plan, `RegisterOk`,
    /// `JoinOk`, and `AwaitOk` all carry `StampedEnvelope`, whose
    /// `envelope` field is attacker-authored for a `Gateway`/`Unknown`
    /// origin sender. `{:?}` on the whole reply prints that payload
    /// verbatim. Callers reporting "unexpected reply" should use this
    /// variant name instead of `{:?}` — never the payload.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::HelloOk { .. } => "HelloOk",
            Self::HelloErr { .. } => "HelloErr",
            Self::RegisterOk { .. } => "RegisterOk",
            Self::SendOk { .. } => "SendOk",
            Self::InboxOk { .. } => "InboxOk",
            Self::AwaitOk { .. } => "AwaitOk",
            Self::AwaitTimeout {} => "AwaitTimeout",
            Self::JoinOk { .. } => "JoinOk",
            Self::LeaveOk { .. } => "LeaveOk",
            Self::SessionsOk { .. } => "SessionsOk",
            Self::WhoamiOk { .. } => "WhoamiOk",
            Self::InspectOk { .. } => "InspectOk",
            Self::SetListenOk { .. } => "SetListenOk",
            Self::SetWakeAddrOk { .. } => "SetWakeAddrOk",
            Self::Err { .. } => "Err",
        }
    }
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
/// - `wake_addr` — D2 (260810-hac): the recipient's Claude Code
///   `SendMessage` address, present ONLY when the recipient has listen
///   mode on, has a validated address stored, AND the SENDING client's
///   declared origin is `Local`. Absent on every channel fan-out row.
///   The sending model relays a content-free ping to it; see
///   `docs/superpowers/specs/2026-08-10-native-wake-ping-design.md`.
///
/// Wire compat: `woken` is `#[serde(default)]` so frames produced
/// by pre-`woken` peers deserialize with `woken = false`. `wake_addr`
/// follows the same pattern plus `skip_serializing_if` so an absent
/// address serializes to the exact pre-260810-hac JSON.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered {
    // woken field is serde-defaulted for wire compatibility.
    pub to: Target,
    pub ok: bool,
    #[serde(default)]
    pub woken: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake_addr: Option<String>,
}

/// Hand-written to keep `Debug` output byte-identical to the pre-
/// `wake_addr` derive when no address is present.
///
/// This is NOT cosmetic. `cli::send::SendOutcome.delivered` is literally
/// `format!("{delivered:?}")` over a `Vec<Delivered>`, and that string is
/// surfaced on the `famp send` JSON line and the `famp_send` MCP tool
/// result. A derived `Debug` would print `wake_addr: None` on every
/// channel row and every no-address DM, silently changing an output
/// contract that this change is supposed to leave untouched — and the
/// serde round-trip tests would not catch it, because they check JSON,
/// not `Debug`.
impl fmt::Debug for Delivered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Delivered");
        s.field("to", &self.to)
            .field("ok", &self.ok)
            .field("woken", &self.woken);
        if let Some(addr) = &self.wake_addr {
            s.field("wake_addr", addr);
        }
        s.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRow {
    pub name: String,
    pub pid: u32,
    pub joined: Vec<String>,
}

/// Per-member info in [`BusReply::JoinOk`].
///
/// `role` is `None` for members that joined without declaring a role.
/// Wire compat: `skip_serializing_if = Option::is_none` omits the field
/// when absent so pre-role peers (which used `Vec<String>` and expected
/// only a name string) are not impacted — note this is a breaking change
/// to the JoinOk shape; callers must accept `Vec<MemberInfo>` not
/// `Vec<String>` after this version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

// #[serde(deny_unknown_fields)] for Target agent variant.
// #[serde(deny_unknown_fields)] for Target channel variant.
// #[serde(deny_unknown_fields)] for BusMessage variant payloads.

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{BusMessage, BusReply, Delivered, MemberInfo, SessionRow, Target};
    use crate::origin::{Origin, StampedEnvelope};
    use crate::{BusErrorKind, MailboxName};
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
    fn roundtrip_busmessage_inspect_broker() {
        use famp_inspect_proto::{InspectBrokerRequest, InspectKind};
        let v = BusMessage::Inspect {
            kind: InspectKind::Broker(InspectBrokerRequest::default()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_busmessage_inspect_identities() {
        use famp_inspect_proto::{InspectIdentitiesRequest, InspectKind};
        let v = BusMessage::Inspect {
            kind: InspectKind::Identities(InspectIdentitiesRequest::default()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_busmessage_inspect_tasks() {
        use famp_inspect_proto::{InspectKind, InspectTasksRequest};
        let v = BusMessage::Inspect {
            kind: InspectKind::Tasks(InspectTasksRequest::default()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_busmessage_inspect_messages() {
        use famp_inspect_proto::{InspectKind, InspectMessagesRequest};
        let v = BusMessage::Inspect {
            kind: InspectKind::Messages(InspectMessagesRequest::default()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_busreply_inspect_ok() {
        let v = BusReply::InspectOk {
            payload: serde_json::json!({"state": "HEALTHY", "pid": 1234_u32}),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_await_ok_batch() {
        let v = BusReply::AwaitOk {
            envelopes: vec![StampedEnvelope {
                origin: Origin::Local,
                envelope: serde_json::json!({"body": "hello"}),
            }],
            mailbox: MailboxName::Channel("#team".into()),
            next_offset: 42,
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_register_with_cwd_and_listen() {
        let v = BusMessage::Register {
            name: "alice".into(),
            pid: 12345,
            cwd: Some("/Users/alice/proj".into()),
            listen: true,
            origin: Some(Origin::Local),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_register_without_cwd_or_listen_byte_exact() {
        // BUS-02 byte-exact: a Register frame omitting cwd, listen, and
        // origin serializes IDENTICALLY to a pre-v0.10 frame.
        let v = BusMessage::Register {
            name: "alice".into(),
            pid: 12345,
            cwd: None,
            listen: false,
            origin: None,
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
        // Wire MUST NOT contain "cwd", "listen", or "origin" keys when
        // all are at default values - otherwise pre-v0.10 peers (which
        // used deny_unknown_fields with no cwd/listen/origin) would
        // reject.
        let wire = String::from_utf8(bytes).unwrap();
        assert!(
            !wire.contains("\"cwd\""),
            "cwd must be omitted at default; got {wire}"
        );
        assert!(
            !wire.contains("\"listen\""),
            "listen must be omitted at default; got {wire}"
        );
        assert!(
            !wire.contains("\"origin\""),
            "origin must be omitted at default; got {wire}"
        );
    }

    /// D-01 fail-closed pin at the wire layer: a `Register` frame
    /// deserialized from JSON with no `origin` key produces `origin ==
    /// None` (which the broker's `register()` handler then resolves via
    /// `unwrap_or_default()` to `Origin::Unknown`, never `Origin::Local`).
    #[test]
    fn register_without_origin_field_deserializes_to_none() {
        let wire = br#"{"name":"alice","op":"register","pid":12345}"#;
        let decoded: BusMessage = famp_canonical::from_slice_strict(wire).unwrap();
        assert_eq!(
            decoded,
            BusMessage::Register {
                name: "alice".into(),
                pid: 12345,
                cwd: None,
                listen: false,
                origin: None,
            }
        );
    }

    #[test]
    fn roundtrip_inbox_ok_stamped() {
        let v = BusReply::InboxOk {
            envelopes: vec![StampedEnvelope {
                origin: Origin::Gateway,
                envelope: json!({"body": "hello"}),
            }],
            next_offset: 7,
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn roundtrip_set_listen_message() {
        for listen in [true, false] {
            let v = BusMessage::SetListen { listen };
            let bytes = famp_canonical::canonicalize(&v).unwrap();
            let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn roundtrip_set_listen_reply() {
        for listen_mode in [true, false] {
            let v = BusReply::SetListenOk { listen_mode };
            let bytes = famp_canonical::canonicalize(&v).unwrap();
            let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn roundtrip_set_wake_addr_message() {
        for wake_addr in [None, Some("uds:/tmp/cc-socks/8091.sock".to_string())] {
            let v = BusMessage::SetWakeAddr { wake_addr };
            let bytes = famp_canonical::canonicalize(&v).unwrap();
            let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn roundtrip_set_wake_addr_reply() {
        for wake_addr in [None, Some("uds:/tmp/cc-socks/8091.sock".to_string())] {
            let v = BusReply::SetWakeAddrOk { wake_addr };
            let bytes = famp_canonical::canonicalize(&v).unwrap();
            let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn wake_addr_regex_accepts_only_cc_socks_shape() {
        assert!(super::wake_addr_valid("uds:/tmp/cc-socks/8091.sock"));
        assert!(super::wake_addr_valid("uds:/tmp/cc-socks/1.sock"));
        // Rejects: wrong scheme, wrong dir, traversal, non-numeric pid,
        // trailing junk, embedded newline, over-long pid.
        for bad in [
            "/tmp/cc-socks/8091.sock",
            "uds:/tmp/other/8091.sock",
            "uds:/tmp/cc-socks/../../etc/passwd.sock",
            "uds:/tmp/cc-socks/abc.sock",
            "uds:/tmp/cc-socks/8091.sock.evil",
            "uds:/tmp/cc-socks/8091.sock\nuds:/tmp/cc-socks/1.sock",
            "uds:/tmp/cc-socks/12345678901.sock",
            "",
        ] {
            assert!(!super::wake_addr_valid(bad), "should reject {bad:?}");
        }
    }

    #[test]
    fn delivered_without_wake_addr_serializes_byte_identically_to_pre_change_form() {
        // The additive field must be invisible on the wire when absent.
        let v = Delivered {
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: false,
            wake_addr: None,
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"ok":true,"to":{"kind":"agent","name":"alice"},"woken":false}"#
        );
    }

    #[test]
    fn delivered_from_a_peer_omitting_wake_addr_deserializes_with_it_absent() {
        let bytes = br#"{"ok":true,"to":{"kind":"agent","name":"alice"},"woken":true}"#;
        let decoded: Delivered = famp_canonical::from_slice_strict(bytes).unwrap();
        assert_eq!(decoded.wake_addr, None);
        assert!(decoded.woken);
    }

    #[test]
    fn delivered_with_wake_addr_round_trips() {
        let v = Delivered {
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: false,
            wake_addr: Some("uds:/tmp/cc-socks/8091.sock".into()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: Delivered = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn delivered_debug_omits_wake_addr_when_absent() {
        // Guards `cli::send::SendOutcome.delivered`, which is a Debug
        // string on a public output surface. A derived Debug would emit
        // `wake_addr: None` here and change that contract.
        let v = Delivered {
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: false,
            wake_addr: None,
        };
        assert_eq!(
            format!("{v:?}"),
            r#"Delivered { to: Agent { name: "alice" }, ok: true, woken: false }"#
        );
    }

    #[test]
    fn delivered_debug_includes_wake_addr_when_present() {
        let v = Delivered {
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: true,
            wake_addr: Some("uds:/tmp/cc-socks/8091.sock".into()),
        };
        let rendered = format!("{v:?}");
        assert!(
            rendered.contains(r#"wake_addr: "uds:/tmp/cc-socks/8091.sock""#),
            "present address must be visible in Debug output; got: {rendered}"
        );
    }

    #[test]
    fn bus_proto_version_is_three() {
        assert_eq!(super::BUS_PROTO_VERSION, 3);
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
                woken: false,
                wake_addr: None,
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
            Delivered {
                // woken defaults false when omitted on the wire.
                to: Target::Agent {
                    name: "alice".into()
                },
                ok: true,
                woken: false,
                wake_addr: None,
            }
        );
    }

    #[test]
    fn delivered_with_woken_round_trips() {
        let delivered = Delivered {
            to: Target::Agent {
                name: "alice".into(),
            },
            ok: true,
            woken: true,
            wake_addr: None,
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

    /// Join with no role omits the field on the wire (wire compat).
    #[test]
    fn join_without_role_byte_exact_no_role_field() {
        let v = BusMessage::Join {
            channel: "#team".into(),
            role: None,
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let wire = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            !wire.contains("\"role\""),
            "role must be omitted at default; got {wire}"
        );
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    /// Join with a role includes the field on the wire.
    #[test]
    fn join_with_role_roundtrips() {
        let v = BusMessage::Join {
            channel: "#team".into(),
            role: Some("judge".into()),
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let wire = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            wire.contains("\"role\""),
            "role must be present; got {wire}"
        );
        let decoded: BusMessage = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    /// JoinOk with MemberInfo, one with role and one without.
    #[test]
    fn joinok_memberinfo_roundtrips() {
        let v = BusReply::JoinOk {
            channel: "#team".into(),
            members: vec![
                MemberInfo {
                    name: "alice".into(),
                    role: Some("judge".into()),
                },
                MemberInfo {
                    name: "bob".into(),
                    role: None,
                },
            ],
            drained: vec![StampedEnvelope {
                origin: Origin::Gateway,
                envelope: serde_json::json!({"body": "hello"}),
            }],
        };
        let bytes = famp_canonical::canonicalize(&v).unwrap();
        let decoded: BusReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
        // Bob has no role — verify field is absent in wire JSON.
        let wire = String::from_utf8(bytes).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&wire).unwrap();
        let bob = parsed["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "bob")
            .unwrap();
        assert!(bob.get("role").is_none(), "bob should have no role field");
    }
}
