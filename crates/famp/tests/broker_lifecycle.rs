#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Plan 02-02 Task 3: BROKER-01 closure. Other tests (BROKER-04 idle exit,
// CLI-11 sessions diagnostic-only, BROKER-05 NFS warning, full
// integration) remain stubs owned by plan 02-11.

use famp::bus_client::codec;
use famp_bus::{BusMessage, BusReply};
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};

/// BROKER-01: a freshly-spawned broker accepts a UDS connection,
/// completes the BUS-06 Hello handshake (with the D-10 `bind_as`
/// field), and responds with `HelloOk`.
#[test]
fn test_broker_accepts_connection() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let bus_dir = tmp.path().to_path_buf();
        let sock = bus_dir.join("bus.sock");

        // Bind the listener up front so we know the broker is ready
        // before we connect (avoids the spawn-then-bind race).
        let listener = UnixListener::bind(&sock).unwrap();

        // In-process shutdown future — fires when the receiver drops.
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let shutdown_fut = async move {
            let _ = shutdown_rx.await;
        };

        // Spawn the broker on its own task.
        let sock_for_broker = sock.clone();
        let bus_dir_for_broker = bus_dir.clone();
        let broker_handle = tokio::spawn(async move {
            famp::cli::broker::run_on_listener(
                &sock_for_broker,
                &bus_dir_for_broker,
                listener,
                shutdown_fut,
            )
            .await
        });

        // Give the broker a tick to enter its select loop.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Connect + Hello.
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let hello = BusMessage::Hello {
            bus_proto: 1,
            client: "broker-lifecycle-test/0.0.1".into(),
            bind_as: None,
        };
        codec::write_frame(&mut stream, &hello).await.unwrap();
        let reply: BusReply = codec::read_frame(&mut stream).await.unwrap();
        match reply {
            BusReply::HelloOk { bus_proto } => assert_eq!(bus_proto, 1),
            other => panic!("expected HelloOk, got {other:?}"),
        }

        // Shut down cleanly.
        drop(stream);
        let _ = shutdown_tx.send(());
        let res = tokio::time::timeout(Duration::from_secs(2), broker_handle)
            .await
            .expect("broker did not shut down")
            .expect("broker task join failed");
        res.expect("broker exited with error");
    });
}

#[test]
#[ignore = "stub: implementation lands in plan 02-11"]
fn test_broker_idle_exit() {
    unimplemented!("filled in by plan 02-11");
}

#[test]
#[ignore = "stub: implementation lands in plan 02-11"]
fn test_sessions_jsonl_diagnostic_only() {
    unimplemented!("filled in by plan 02-11");
}

#[test]
#[ignore = "stub: implementation lands in plan 02-11"]
fn test_nfs_warning() {
    unimplemented!("filled in by plan 02-11");
}
