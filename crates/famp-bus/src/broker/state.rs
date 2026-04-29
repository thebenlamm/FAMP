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
    /// D-10: the proxy identity supplied at Hello time.
    ///
    /// `None` = canonical/unbound connection. The slot is owned by the
    /// connection if `name = Some(_)` (post-Register). `Some(holder)` =
    /// this connection is a read/write-through proxy to a separate
    /// canonical holder; `name` MUST stay `None` and `pid` MUST stay
    /// `None` for the lifetime of the proxy connection. The broker
    /// re-verifies `holder` is still live on every identity-required
    /// op via `proxy_holder_alive`.
    pub(super) bind_as: Option<String>,
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
