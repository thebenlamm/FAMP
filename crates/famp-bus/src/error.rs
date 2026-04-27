//! `BusErrorKind` - typed bus protocol error categories.
//!
//! Closed enum, no wildcard. BUS-05 forces every downstream consumer
//! (Phase 2 MCP error mapping per MCP-10, the Phase 1 consumer stub at
//! `tests/buserror_consumer_stub.rs`) to update via compile error when a
//! variant is added. Mirrors `famp-core::ProtocolErrorKind`'s exhaustive
//! flat-enum precedent.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BusErrorKind {
    NotRegistered,
    NameTaken,
    ChannelNameInvalid,
    NotJoined,
    EnvelopeInvalid,
    EnvelopeTooLarge,
    TaskNotFound,
    BrokerProtoMismatch,
    BrokerUnreachable,
    Internal,
}

impl BusErrorKind {
    /// All 10 variants in declaration order. Used by exhaustive consumer-stub
    /// tests to prove no variant is silently dropped from a downstream match.
    pub const ALL: [Self; 10] = [
        Self::NotRegistered,
        Self::NameTaken,
        Self::ChannelNameInvalid,
        Self::NotJoined,
        Self::EnvelopeInvalid,
        Self::EnvelopeTooLarge,
        Self::TaskNotFound,
        Self::BrokerProtoMismatch,
        Self::BrokerUnreachable,
        Self::Internal,
    ];
}
