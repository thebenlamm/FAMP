//! Pure broker actor. `Broker::handle(input, now) -> Vec<Out>` is total,
//! infallible, synchronous, and stages every side effect as an ordered intent.

pub mod handle;
mod state;

use std::time::Instant;

use crate::{AwaitFilter, BrokerEnv, BusMessage, BusReply, ClientId, MailboxName};

pub use state::{BrokerStateView, ClientStateView};

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
    /// v0.10 inspector dispatch sentinel. Emitted when the broker
    /// receives a `BusMessage::Inspect { kind }` frame. The executor
    /// pre-reads mailbox metadata, builds `BrokerCtx`, calls
    /// `famp_inspect_server::dispatch`, and synthesizes a
    /// `BusReply::InspectOk` reply back to `client`.
    ///
    /// Why a sentinel instead of dispatching inline in the actor:
    /// the Identities handler needs mailbox metadata, which lives
    /// on disk and would require synchronous file I/O inside the
    /// pure tokio-free actor. The actor stays tokio-free; the
    /// executor handles the I/O.
    InspectRequest {
        client: ClientId,
        kind: famp_inspect_proto::InspectKind,
    },
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

    /// v0.10 read-only state snapshot for `famp-inspect-server`
    /// dispatch. Cheap clone of in-memory state. Does NOT include
    /// mailbox metadata; the broker executor reads that separately and
    /// passes it alongside the view via `BrokerCtx`.
    pub fn view(&self) -> BrokerStateView {
        self.state.view()
    }

}
