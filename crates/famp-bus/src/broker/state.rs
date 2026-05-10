use std::collections::{BTreeMap, BTreeSet};
use std::time::{Instant, SystemTime};

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
    /// D-01/D-03: client's working directory at registration time.
    /// Captured ONCE; never refreshed (D-02). If the client chdir's
    /// after registering, this field reflects where the agent was
    /// born. `None` for pre-v0.10 senders that didn't include the
    /// field on the Register frame.
    pub(super) cwd: Option<String>,
    /// listen-mode flag from BusMessage::Register. `false` for
    /// pre-v0.10 senders that didn't include the field. Surfaced
    /// in `famp inspect identities` rows (INSP-IDENT-01).
    pub(super) listen_mode: bool,
    /// Wall-clock registration time. Set in the Register handler
    /// arm so `famp inspect identities` can compute registered-at
    /// per row. `Instant` is NOT used because Instant has no
    /// epoch encoding; SystemTime serializes to u64 epoch seconds.
    pub(super) registered_at: SystemTime,
    /// Wall-clock last-activity time. Updated by the Register
    /// handler initially, refreshed on every authenticated wire
    /// frame from the client (Send/Inbox/Await/Join/Leave/Whoami).
    /// Wave-0 sets it at register time only; Wave-2 broker dispatch
    /// arm updates it on inspect calls. Pre-existing identity rows
    /// are populated retroactively from registered_at.
    pub(super) last_activity: SystemTime,
}

#[derive(Debug, Clone)]
pub(super) struct ParkedAwait {
    pub(super) client: ClientId,
    pub(super) filter: AwaitFilter,
    pub(super) deadline: Instant,
}

/// Broker-actor state. v0.10 added `started_at` (D-07) populated
/// at construction; `derive(Default)` was REMOVED because Default
/// for SystemTime is `UNIX_EPOCH`, which would falsely report
/// 1970-01-01 as broker startup time (D-08).
#[derive(Debug)]
pub(super) struct BrokerState {
    pub(super) clients: BTreeMap<ClientId, ClientState>,
    pub(super) channels: BTreeMap<String, BTreeSet<String>>,
    pub(super) pending_awaits: BTreeMap<ClientId, ParkedAwait>,
    pub(super) cursors: BTreeMap<MailboxName, u64>,
    /// D-07: wall-clock startup time, set by the answering process.
    /// Surfaced in `famp inspect broker` reply (INSP-BROKER-01).
    /// NEVER socket file mtime (D-08): mtime lies after restart-
    /// with-reused-socket, `touch` from external tools, and FS quirks.
    #[allow(dead_code)]
    pub(super) started_at: SystemTime,
}

impl BrokerState {
    pub(super) fn new() -> Self {
        Self {
            clients: BTreeMap::new(),
            channels: BTreeMap::new(),
            pending_awaits: BTreeMap::new(),
            cursors: BTreeMap::new(),
            started_at: SystemTime::now(),
        }
    }

    /// Build a read-only view for inspector RPCs. Includes only
    /// `connected = true` clients with a registered `name` (skips
    /// pure-Hello connections and proxy connections that have not
    /// resolved a holder yet).
    pub(crate) fn view(&self) -> BrokerStateView {
        let clients = self
            .clients
            .values()
            .filter(|c| c.connected && c.name.is_some())
            .map(|c| ClientStateView {
                name: c.name.clone().unwrap_or_default(),
                pid: c.pid,
                bind_as: c.bind_as.clone(),
                cwd: c.cwd.clone(),
                listen_mode: c.listen_mode,
                registered_at: c.registered_at,
                last_activity: c.last_activity,
                joined: c.joined.iter().cloned().collect(),
            })
            .collect();
        BrokerStateView {
            started_at: self.started_at,
            clients,
        }
    }
}

/// v0.10 read-only snapshot of `BrokerState` for `famp-inspect-server`
/// consumption. The inspector cannot reach `pub(super)` fields
/// directly across crate boundaries, so we expose a structurally-
/// equivalent `pub` view-type populated by `BrokerState::view()`.
///
/// INSP-RPC-02: this view is produced by an `&BrokerState -> Self`
/// transform. The server crate receives `&BrokerStateView` and
/// cannot upgrade it to `&mut BrokerState`.
#[derive(Debug, Clone)]
pub struct BrokerStateView {
    pub started_at: SystemTime,
    pub clients: Vec<ClientStateView>,
}

/// One row per registered client, derived from `ClientState`.
/// Skips connections that have not yet `Register`'d (no name).
#[derive(Debug, Clone)]
pub struct ClientStateView {
    pub name: String,
    pub pid: Option<u32>,
    pub bind_as: Option<String>,
    pub cwd: Option<String>,
    pub listen_mode: bool,
    pub registered_at: SystemTime,
    pub last_activity: SystemTime,
    pub joined: Vec<String>,
}
