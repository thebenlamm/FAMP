use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::{AwaitFilter, ClientId, MailboxName};

#[derive(Debug, Clone)]
pub(super) struct ClientState {
    pub(super) handshaked: bool,
    #[allow(dead_code)]
    pub(super) bus_proto: u32,
    pub(super) name: Option<String>,
    pub(super) pid: Option<u32>,
    pub(super) joined: BTreeSet<String>,
    pub(super) connected: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ParkedAwait {
    pub(super) client: ClientId,
    pub(super) filter: AwaitFilter,
    pub(super) deadline: Instant,
}

#[derive(Debug, Default)]
pub(super) struct BrokerState {
    pub(super) clients: BTreeMap<ClientId, ClientState>,
    pub(super) channels: BTreeMap<String, BTreeSet<String>>,
    pub(super) pending_awaits: BTreeMap<ClientId, ParkedAwait>,
    pub(super) cursors: BTreeMap<MailboxName, u64>,
}
