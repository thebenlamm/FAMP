//! Per-client tokio task that drives one UDS connection.
//!
//! Owns one `UnixStream`. Splits into owned read/write halves
//! (`UnixStream::into_split`) so the read loop and write loop can run
//! concurrently as a `tokio::join!` pair (full-duplex required because
//! the broker may push replies — e.g. `AwaitOk` after a long park —
//! between caller-driven request frames).
//!
//! The read loop forwards every decoded `BusMessage` to the broker over
//! `broker_tx`. EOF or codec failure ends both loops via the shared
//! `BrokerMsg::Disconnect` send + a closed reply channel.

use famp_bus::{BusMessage, BusReply, ClientId};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::bus_client::codec;

/// Wire-frame message emitted by a per-client task to the central broker
/// task.
#[derive(Debug)]
pub enum BrokerMsg {
    /// One decoded request frame from `client`.
    Frame(ClientId, BusMessage),
    /// `client`'s connection has closed (EOF or codec failure).
    Disconnect(ClientId),
}

/// Drive one `UnixStream` until EOF or codec failure.
///
/// `broker_tx` is the central broker's inbox. `reply_rx` is the
/// per-client outbound channel; the broker `send`s `BusReply` frames
/// here and we serialize them to the wire. When the broker drops the
/// reply sender, the write loop exits.
pub async fn client_task(
    id: ClientId,
    stream: UnixStream,
    broker_tx: mpsc::Sender<BrokerMsg>,
    mut reply_rx: mpsc::Receiver<BusReply>,
) {
    let (mut reader, mut writer) = stream.into_split();
    let read_tx = broker_tx.clone();

    let read_handle = tokio::spawn(async move {
        loop {
            let Ok(msg) = codec::read_frame::<_, BusMessage>(&mut reader).await else {
                let _ = read_tx.send(BrokerMsg::Disconnect(id)).await;
                return;
            };
            if read_tx.send(BrokerMsg::Frame(id, msg)).await.is_err() {
                return; // broker shut down
            }
        }
    });

    let write_handle = tokio::spawn(async move {
        while let Some(reply) = reply_rx.recv().await {
            if codec::write_frame(&mut writer, &reply).await.is_err() {
                // Wire write failed — peer probably gone. Drain remaining
                // queued replies into the void so the broker's send side
                // doesn't back-pressure-block; the read loop will surface
                // the same disconnect through its own EOF path.
                while reply_rx.recv().await.is_some() {}
                return;
            }
        }
    });

    // Whichever loop finishes first triggers the other to wind down:
    // - read loop EOF → `read_tx` drop → broker eventually drops reply_tx
    //   → write loop's `reply_rx.recv()` returns None → it exits.
    // - write loop wire failure → return; read loop continues until
    //   peer closes the read half (which it will, since the wire is dead).
    let _ = tokio::join!(read_handle, write_handle);
}
