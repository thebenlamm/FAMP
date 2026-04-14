//! UUIDv7-backed ID newtypes (D-10..D-13).
//!
//! Four distinct types so the compiler rejects cross-assignment between
//! `MessageId`, `ConversationId`, `TaskId`, and `CommitmentId`. No
//! `AsRef<Uuid>` blanket, no `From<Uuid>` — explicit `as_uuid()` only.

macro_rules! define_uuid_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Generate a new UUIDv7-backed id (time-ordered, RFC 9562).
            #[must_use]
            pub fn new_v7() -> Self {
                Self(uuid::Uuid::now_v7())
            }

            /// Access the underlying UUID. Prefer passing the typed newtype.
            #[must_use]
            pub const fn as_uuid(&self) -> &uuid::Uuid {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                // D-11: canonical hyphenated form only. Hyphenated form is
                // always 36 chars; reject the 32-char `simple` form by
                // forcing parse_str through a guaranteed-invalid shape.
                if s.len() != 36 {
                    // parse_str on "!" is guaranteed to return Err; the `?`
                    // propagates that uuid::Error cleanly without unwrap.
                    uuid::Uuid::parse_str("!")?;
                }
                Ok(Self(uuid::Uuid::parse_str(s)?))
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(&self.0.hyphenated())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(d)?;
                <Self as std::str::FromStr>::from_str(&s).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_uuid_newtype!(MessageId, "Message identifier (`UUIDv7`).");
define_uuid_newtype!(ConversationId, "Conversation identifier (`UUIDv7`).");
define_uuid_newtype!(TaskId, "Task identifier (`UUIDv7`).");
define_uuid_newtype!(CommitmentId, "Commitment identifier (`UUIDv7`).");
