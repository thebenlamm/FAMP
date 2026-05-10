//! Pure broker actor. `Broker::handle(input, now) -> Vec<Out>` is total,
//! infallible, synchronous, and stages every side effect as an ordered intent.

pub mod handle;
mod state;

use std::time::Instant;

use crate::{AwaitFilter, BrokerEnv, BusMessage, BusReply, ClientId, MailboxName};

#[derive(Debug, Clone)]
pub enum BrokerInput {
    Wire { client: ClientId, msg: BusMessage },
    Disconnect(ClientId),
    Tick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    Reply(ClientId, BusReply),
    AppendMailbox {
        target: MailboxName,
        line: Vec<u8>,
    },
    AdvanceCursor {
        name: MailboxName,
        offset: u64,
    },
    ParkAwait {
        client: ClientId,
        filter: AwaitFilter,
        deadline: Instant,
    },
    UnparkAwait {
        client: ClientId,
    },
    ReleaseClient(ClientId),
    /// WR-07: emitted by `disconnect` for a canonical-holder client at
    /// the moment its session ends, BEFORE its `joined` set is cleared.
    /// The executor surfaces this as a `SessionRow` write to
    /// `~/.famp/sessions.jsonl` so post-mortem operators can see which
    /// channels the session held when it disconnected. Proxy
    /// disconnects do NOT emit this variant (proxies never appended a
    /// `SessionRow` on register).
    SessionEnded {
        name: String,
        pid: u32,
        joined: Vec<String>,
    },
}

pub struct Broker<E: BrokerEnv> {
    env: E,
    state: state::BrokerState,
}

impl<E: BrokerEnv> Broker<E> {
    pub fn new(env: E) -> Self {
        Self {
            env,
            state: state::BrokerState::new(),
        }
    }

    pub fn handle(&mut self, input: BrokerInput, now: Instant) -> Vec<Out> {
        handle::handle(self, input, now)
    }
}
