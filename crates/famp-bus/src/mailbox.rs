//! Read-only mailbox trait and deterministic in-memory test implementation.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use serde::{
    de::{Error as DeError, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

mod private {
    pub trait Sealed {}
    impl<T> Sealed for T {}
}

/// Bytes a JSONL record occupies beyond its payload: the single trailing `\n`.
///
/// This is the workspace-wide JSONL framing width. Every crate that computes a
/// mailbox byte cursor MUST source the terminator width here rather than
/// hardcoding `+ 1` — the `famp` crate's `read_raw_from` imports it via
/// `famp_bus::JSONL_RECORD_TERMINATOR_LEN`.
///
/// The mirror is not only about framing width: [`InMemoryMailbox`] must also
/// mirror production's past-EOF cursor semantics. See [`MailboxRead`].
///
/// **`famp-inbox` is the one exception, and it cannot be fixed.** `famp-inbox`
/// is the tokio-backed durable-storage layer that sits BELOW the pure,
/// tokio-free actor in `famp-bus` (invariant BUS-01, enforced by
/// `just check-no-tokio-in-bus`). `famp-bus` therefore cannot depend on
/// `famp-inbox`, and importing `famp-bus` from `famp-inbox` would invert the
/// layering. The mirror this constant is coupled to is the `+ 1` at
/// `famp-inbox/src/read.rs:175`; if either moves, both must move.
pub const JSONL_RECORD_TERMINATOR_LEN: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MailboxName {
    Agent(String),
    Channel(String),
}

impl Serialize for MailboxName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            kind: &'a str,
            name: &'a str,
        }

        let wire = match self {
            Self::Agent(name) => Wire {
                kind: "agent",
                name,
            },
            Self::Channel(name) => Wire {
                kind: "channel",
                name,
            },
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MailboxName {
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

        struct MailboxNameVisitor;

        impl<'de> Visitor<'de> for MailboxNameVisitor {
            type Value = MailboxName;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a mailbox object with kind and name")
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
                    "agent" => Ok(MailboxName::Agent(name)),
                    "channel" => Ok(MailboxName::Channel(name)),
                    _ => Err(DeError::unknown_variant(&kind, &["agent", "channel"])),
                }
            }
        }

        deserializer.deserialize_map(MailboxNameVisitor)
    }
}

impl fmt::Display for MailboxName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(name) | Self::Channel(name) => f.write_str(name),
        }
    }
}

/// One JSONL record drained from a mailbox, carrying its own byte framing.
///
/// Consumers MUST NOT re-derive offsets from `bytes.len()`: `end` is the
/// single authority for "the cursor value after consuming exactly this
/// record". Producing `start`/`end` in the `MailboxRead` impl (the only
/// code that knows the on-disk layout) is what keeps the three drain
/// consumers from each maintaining their own framing arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainedRecord {
    /// The record's payload, WITHOUT its trailing `\n`.
    pub bytes: Vec<u8>,
    /// Absolute byte offset of the record's first byte.
    pub start: u64,
    /// Absolute byte offset one past the record's terminating `\n`.
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainResult {
    pub records: Vec<DrainedRecord>,
    pub next_offset: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum MailboxErr {
    #[error("mailbox {name} does not exist")]
    NotFound { name: MailboxName },
    #[error("internal mailbox error: {0}")]
    Internal(String),
}

pub trait MailboxRead: private::Sealed {
    /// Drain every complete record at or after `since_bytes`.
    ///
    /// # Past-EOF cursor contract (issues #11 / #12)
    ///
    /// A `since_bytes` at or beyond the mailbox's current length is **not an
    /// error**. Implementations MUST return an empty drain whose `next_offset`
    /// is the mailbox's current end. Mailbox files legitimately shrink: the
    /// `/famp-clear` skill truncates `~/.famp/mailboxes/*.jsonl` while the
    /// broker is running and still holding in-memory cursors into them. A
    /// shrinking mailbox is an expected external event, not a broker invariant
    /// violation.
    ///
    /// `next_offset` is therefore the single authority on where the mailbox now
    /// ends, and a caller whose cursor sits past it must clamp forward-progress
    /// down to it rather than treat its own cursor as truth. `broker::drain_walk`
    /// is the one place that does this; do not re-derive it per call site.
    ///
    /// [`InMemoryMailbox`] and `famp`'s `DiskMailboxEnv` are pinned to this
    /// contract by `past_eof_cursor_clamps_and_never_errors` below.
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryMailbox {
    boxes: Arc<Mutex<BTreeMap<MailboxName, Vec<Vec<u8>>>>>,
}

impl InMemoryMailbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&self, name: &MailboxName, line: Vec<u8>) {
        let Ok(mut guard) = self.boxes.lock() else {
            panic!("in-memory mailbox lock poisoned");
        };
        guard.entry(name.clone()).or_default().push(line);
    }

    /// Drop every record from `name`, leaving a present-but-empty mailbox.
    ///
    /// Models the external truncation that `/famp-clear` performs on a live
    /// `~/.famp/mailboxes/*.jsonl` while the broker still holds cursors into
    /// it. Distinct from an absent mailbox: a truncated mailbox drains to
    /// `next_offset: 0`, an absent one echoes the caller's cursor back.
    pub fn truncate(&self, name: &MailboxName) {
        let Ok(mut guard) = self.boxes.lock() else {
            panic!("in-memory mailbox lock poisoned");
        };
        if let Some(entries) = guard.get_mut(name) {
            entries.clear();
        }
    }
}

impl MailboxRead for InMemoryMailbox {
    #[allow(clippy::significant_drop_tightening)]
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr> {
        let entries = {
            let guard = self
                .boxes
                .lock()
                .map_err(|_| MailboxErr::Internal("in-memory mailbox lock poisoned".into()))?;
            let Some(entries) = guard.get(name) else {
                return Ok(DrainResult {
                    records: Vec::new(),
                    next_offset: since_bytes,
                });
            };
            entries.clone()
        };

        let total: u64 = entries
            .iter()
            .map(|line| line.len() as u64 + JSONL_RECORD_TERMINATOR_LEN)
            .sum();
        // Past-EOF clamp, mirroring `DiskMailboxEnv::read_raw_from`'s
        // `since_bytes >= file_len` branch exactly (see the `MailboxRead`
        // contract). `>=` rather than `>` is deliberate: at `since_bytes ==
        // total` the loop below would also produce `(records: [], next_offset:
        // total)`, so the early return is a pure short-circuit there and only
        // changes behavior for the genuinely past-EOF case, where production
        // clamps and the double used to error.
        if since_bytes >= total {
            return Ok(DrainResult {
                records: Vec::new(),
                next_offset: total,
            });
        }

        let mut cursor = 0_u64;
        let mut records = Vec::new();
        for line in entries {
            // Each line's size is payload + `\n`, mirroring the disk JSONL
            // framing in `famp-inbox/src/read.rs`.
            let next = cursor + line.len() as u64 + JSONL_RECORD_TERMINATOR_LEN;
            if cursor >= since_bytes {
                records.push(DrainedRecord {
                    bytes: line,
                    start: cursor,
                    end: next,
                });
            }
            cursor = next;
        }

        Ok(DrainResult {
            records,
            next_offset: cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{DrainedRecord, InMemoryMailbox, MailboxName, MailboxRead};

    #[test]
    fn in_memory_mailbox_accounts_for_newline_offsets() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());

        mailbox.append(&name, b"line1".to_vec());
        let drained = mailbox.drain_from(&name, 0).unwrap();

        assert_eq!(
            drained.records,
            vec![DrainedRecord {
                bytes: b"line1".to_vec(),
                start: 0,
                end: 6,
            }]
        );
        assert_eq!(drained.next_offset, 6);
    }

    #[test]
    fn in_memory_mailbox_records_carry_absolute_offsets() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());

        mailbox.append(&name, b"aa".to_vec());
        mailbox.append(&name, b"bbbb".to_vec());
        mailbox.append(&name, b"c".to_vec());

        // Drain from the boundary after the first record: the surviving
        // records keep ABSOLUTE offsets, not offsets relative to `since`.
        let drained = mailbox.drain_from(&name, 3).unwrap();
        assert_eq!(
            drained.records,
            vec![
                DrainedRecord {
                    bytes: b"bbbb".to_vec(),
                    start: 3,
                    end: 8,
                },
                DrainedRecord {
                    bytes: b"c".to_vec(),
                    start: 8,
                    end: 10,
                },
            ]
        );
        assert_eq!(drained.next_offset, 10);
        // The last record's `end` is the drain's next cursor.
        assert_eq!(drained.records[1].end, drained.next_offset);
    }

    /// Issue #12: the double used to return `Err(CursorOutOfRange)` where
    /// production (`DiskMailboxEnv::read_raw_from`) silently clamps, which made
    /// the truncation edge in #11 untestable. Pins the `MailboxRead` past-EOF
    /// contract: clamp to the mailbox end, empty drain, never error.
    #[test]
    fn past_eof_cursor_clamps_and_never_errors() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());

        mailbox.append(&name, b"aa".to_vec());
        mailbox.append(&name, b"bbbb".to_vec());
        let total = 8_u64; // (2 + 1) + (4 + 1)

        for since in [total, total + 1, total + 5000, u64::MAX] {
            let drained = mailbox
                .drain_from(&name, since)
                .unwrap_or_else(|error| panic!("since={since} must not error: {error}"));
            assert_eq!(drained.records, vec![], "since={since}");
            assert_eq!(drained.next_offset, total, "since={since}");
        }
    }

    /// The `>= total` short-circuit must not change the `since == total`
    /// (caught-up, not truncated) case: the pre-clamp loop reached the same
    /// answer by falling through. Boundary pinned so the short-circuit stays a
    /// short-circuit.
    #[test]
    fn cursor_exactly_at_eof_is_an_empty_drain_at_eof() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());
        mailbox.append(&name, b"line1".to_vec());

        let drained = mailbox.drain_from(&name, 6).unwrap();
        assert_eq!(drained.records, vec![]);
        assert_eq!(drained.next_offset, 6);
    }

    /// An empty-but-present mailbox: `total == 0`, `since == 0`. The clamp
    /// branch fires (`0 >= 0`) where the loop used to fall through — same
    /// answer, and distinct from the absent-mailbox branch above it, which
    /// echoes `since_bytes` back instead.
    #[test]
    fn empty_mailbox_at_zero_cursor_clamps_to_zero() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());
        mailbox.append(&name, b"x".to_vec());
        mailbox.truncate(&name);

        let drained = mailbox.drain_from(&name, 0).unwrap();
        assert_eq!(drained.records, vec![]);
        assert_eq!(drained.next_offset, 0);
    }
}
