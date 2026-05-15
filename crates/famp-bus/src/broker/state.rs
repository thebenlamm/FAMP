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
    /// listen-mode flag from `BusMessage::Register`. `false` for
    /// pre-v0.10 senders that didn't include the field. Surfaced
    /// in `famp inspect identities` rows (INSP-IDENT-01).
    pub(super) listen_mode: bool,
    /// Wall-clock registration time. Set in the Register handler
    /// arm so `famp inspect identities` can compute registered-at
    /// per row. `Instant` is NOT used because Instant has no
    /// epoch encoding; `SystemTime` serializes to u64 epoch seconds.
    pub(super) registered_at: SystemTime,
    /// Wall-clock last-activity time. Updated by the Register
    /// handler initially, refreshed on every authenticated wire
    /// frame from the client (Send/Inbox/Await/Join/Leave/Whoami).
    /// Wave-0 sets it at register time only; Wave-2 broker dispatch
    /// arm updates it on inspect calls. Pre-existing identity rows
    /// are populated retroactively from `registered_at`.
    pub(super) last_activity: SystemTime,
    /// Broker-owned delivery offsets for `Await`.
    ///
    /// These are per canonical session identity, not per proxy client.
    /// One-shot CLI/MCP `Await` calls connect as D-10 proxies and then
    /// disappear, so the offset must live on the canonical holder if
    /// repeated awaits are to drain without replaying old messages.
    pub(super) await_offsets: BTreeMap<MailboxName, u64>,
}

#[derive(Debug, Clone)]
pub(super) struct ParkedAwait {
    pub(super) client: ClientId,
    pub(super) filter: AwaitFilter,
    pub(super) deadline: Instant,
}

/// Broker-actor state. v0.10 added `started_at` (D-07) populated
/// at construction; `derive(Default)` was REMOVED because Default
/// for `SystemTime` is `UNIX_EPOCH`, which would falsely report
/// 1970-01-01 as broker startup time (D-08).
///
/// Disk cursor truth for inspector unread counts lives at
/// `~/.famp/mailboxes/.<name>.cursor` (written by
/// `cursor_exec::execute_advance_cursor`). `await_offsets` is separate:
/// it is in-memory, session-scoped delivery state used only by
/// `BusMessage::Await`.
#[derive(Debug)]
pub(super) struct BrokerState {
    pub(super) clients: BTreeMap<ClientId, ClientState>,
    pub(super) channels: BTreeMap<String, BTreeSet<String>>,
    pub(super) pending_awaits: BTreeMap<ClientId, ParkedAwait>,
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

        // Fan-out waiters: one row per (pending_await × monitored mailbox).
        // A parked await watches the identity's own agent mailbox plus all
        // joined channels. `await_offsets` holds current cursor per mailbox;
        // deadline_ms is clamped to zero if already past (Instant is monotonic).
        let now = Instant::now();
        let mut waiters: Vec<WaiterStateView> = Vec::new();
        for (client_id, parked) in &self.pending_awaits {
            // Resolve canonical identity name from ClientId.
            let Some(client_state) = self.clients.get(client_id) else {
                continue;
            };
            let name = match (&client_state.name, &client_state.bind_as) {
                (Some(n), _) => n.clone(),
                (None, Some(b)) => b.clone(),
                (None, None) => continue,
            };
            // Resolve owner (canonical holder) — for proxy connections the
            // await_offsets live on the canonical holder's ClientState.
            let owner_id = if client_state.name.is_some() {
                *client_id
            } else {
                // Look up the canonical holder by bind_as name.
                let bound = client_state.bind_as.as_deref().unwrap_or("");
                self.clients
                    .iter()
                    .find_map(|(id, s)| {
                        if s.connected && s.name.as_deref() == Some(bound) {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(*client_id)
            };
            let owner_state = self.clients.get(&owner_id);

            let deadline_ms = parked
                .deadline
                .checked_duration_since(now)
                .map_or(0, |d| d.as_millis() as u64);

            // Agent mailbox (always watched).
            let agent_mailbox = crate::MailboxName::Agent(name.clone());
            let cursor = owner_state
                .and_then(|s| s.await_offsets.get(&agent_mailbox).copied())
                .unwrap_or(0);
            waiters.push(WaiterStateView {
                name: name.clone(),
                mailbox: name.clone(), // agent mailbox key = identity name
                cursor,
                deadline_ms,
            });

            // Channel mailboxes (one row per joined channel).
            if let Some(s) = owner_state {
                for channel in &s.joined {
                    let ch_mailbox = crate::MailboxName::Channel(channel.clone());
                    let ch_cursor = s.await_offsets.get(&ch_mailbox).copied().unwrap_or(0);
                    waiters.push(WaiterStateView {
                        name: name.clone(),
                        mailbox: channel.clone(),
                        cursor: ch_cursor,
                        deadline_ms,
                    });
                }
            }
        }

        BrokerStateView {
            started_at: self.started_at,
            clients,
            waiters,
        }
    }
}

/// v0.10 read-only snapshot of `BrokerState`.
///
/// The inspector cannot reach `pub(super)` fields directly across
/// crate boundaries, so we expose a structurally-equivalent `pub`
/// view-type populated by `BrokerState::view()`.
///
/// INSP-RPC-02: this view is produced by an `&BrokerState -> Self`
/// transform. The server crate receives `&BrokerStateView` and
/// cannot upgrade it to `&mut BrokerState`.
#[derive(Debug, Clone)]
pub struct BrokerStateView {
    pub started_at: SystemTime,
    pub clients: Vec<ClientStateView>,
    /// Fan-out rows for pending awaits: one row per (waiter × mailbox).
    pub waiters: Vec<WaiterStateView>,
}

/// One row in the waiters view: a single (identity, mailbox) pair for
/// a client currently parked in `famp_await`. Fan-out: one parked
/// await yields one row for the agent mailbox plus one per joined channel.
#[derive(Debug, Clone)]
pub struct WaiterStateView {
    /// Canonical identity name of the waiting client.
    pub name: String,
    /// Mailbox being watched: identity name for agent, `"#channel"` for channel.
    pub mailbox: String,
    /// Current await offset (bytes already consumed from this mailbox).
    pub cursor: u64,
    /// Remaining wait time in milliseconds (0 if past deadline).
    pub deadline_ms: u64,
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
