use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::{AwaitFilter, ClientId, MailboxName};

#[derive(Debug, Clone)]
pub(crate) struct ClientState {
    pub handshaked: bool,
    #[allow(dead_code)]
    pub bus_proto: u32,
    pub name: Option<String>,
    pub pid: Option<u32>,
    pub joined: BTreeSet<String>,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ParkedAwait {
    pub client: ClientId,
    pub filter: AwaitFilter,
    pub deadline: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct BrokerState {
    pub clients: BTreeMap<ClientId, ClientState>,
    pub channels: BTreeMap<String, BTreeSet<String>>,
    pub pending_awaits: BTreeMap<ClientId, ParkedAwait>,
    pub cursors: BTreeMap<MailboxName, u64>,
}
