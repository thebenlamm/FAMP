#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Plan 02-02 Task 3: BROKER-01 closure (test_broker_accepts_connection).
// Plan 02-11: BROKER-04 idle-exit (`test_broker_idle_exit`),
// CLI-11 sessions.jsonl diagnostic-only (`test_sessions_jsonl_diagnostic_only`),
// BROKER-05 NFS detector public-API gate (`test_nfs_warning`).

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

/// BROKER-04: the broker shuts down cleanly after 5 minutes of zero
/// connected clients. Drives `tokio::time::pause` + `advance` so the
/// 300s timer fires deterministically (no real sleep).
#[tokio::test(start_paused = true)]
async fn test_broker_idle_exit() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bus_dir = tmp.path().to_path_buf();
    let sock = bus_dir.join("bus.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    // shutdown_signal that never fires — only idle-exit triggers shutdown.
    let never_shutdown = std::future::pending::<()>();

    let sock_clone = sock.clone();
    let bus_dir_clone = bus_dir.clone();
    let broker = tokio::spawn(async move {
        famp::cli::broker::run_on_listener(&sock_clone, &bus_dir_clone, listener, never_shutdown)
            .await
    });

    // Yield so the broker enters its select loop before we connect.
    tokio::task::yield_now().await;

    // Connect one client, complete Hello so the broker accounts for it,
    // then disconnect — count goes 1 → 0 and the idle timer arms.
    {
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let hello = BusMessage::Hello {
            bus_proto: 1,
            client: "idle-exit-test/0.0.1".into(),
            bind_as: None,
        };
        codec::write_frame(&mut stream, &hello).await.unwrap();
        let _: BusReply = codec::read_frame(&mut stream).await.unwrap();
        // drop closes; broker_rx receives Disconnect → idle = Some(Sleep(300s))
    }
    // Yield several times so the broker observes accept + Disconnect,
    // then arms the idle timer before we advance virtual time.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    // Fast-forward past the 300s idle threshold.
    tokio::time::advance(Duration::from_secs(301)).await;
    tokio::task::yield_now().await;

    // The broker task must complete (clean shutdown via idle-exit arm).
    let join_result = tokio::time::timeout(Duration::from_secs(5), broker).await;
    assert!(
        join_result.is_ok(),
        "broker must exit after virtual idle-timer fires"
    );
    assert!(
        !sock.exists(),
        "broker must unlink socket on idle exit (BROKER-04)"
    );
}

/// CLI-11: the broker MUST NOT consult `sessions.jsonl` to populate the
/// `Sessions` reply — the file is diagnostic-only, intended for human
/// post-mortems. Pre-write a row referencing a guaranteed-dead PID, and
/// assert that row never appears in the runtime view.
#[tokio::test]
async fn test_sessions_jsonl_diagnostic_only() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bus_dir = tmp.path().to_path_buf();
    let sock = bus_dir.join("bus.sock");

    // Pre-write a fake sessions.jsonl row with a guaranteed-dead PID.
    // CLI-11: the broker MUST NOT read this file when answering Sessions.
    std::fs::write(
        bus_dir.join("sessions.jsonl"),
        r#"{"name":"ghost","pid":99999999,"joined":[]}"#.to_string() + "\n",
    )
    .unwrap();

    let listener = UnixListener::bind(&sock).unwrap();
    let sock_clone = sock.clone();
    let bus_dir_clone = bus_dir.clone();
    let never_shutdown = std::future::pending::<()>();
    let broker = tokio::spawn(async move {
        famp::cli::broker::run_on_listener(&sock_clone, &bus_dir_clone, listener, never_shutdown)
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Register a real client (Hello with bind_as: None, then Register).
    let mut s = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let hello = BusMessage::Hello {
        bus_proto: 1,
        client: "ghost-test/0.0.1".into(),
        bind_as: None,
    };
    codec::write_frame(&mut s, &hello).await.unwrap();
    let _: BusReply = codec::read_frame(&mut s).await.unwrap();
    let reg = BusMessage::Register {
        name: "real".into(),
        pid: std::process::id(),
    };
    codec::write_frame(&mut s, &reg).await.unwrap();
    let _: BusReply = codec::read_frame(&mut s).await.unwrap();

    // Query Sessions on a second connection.
    let mut peek = tokio::net::UnixStream::connect(&sock).await.unwrap();
    let hello2 = BusMessage::Hello {
        bus_proto: 1,
        client: "peek/0.0.1".into(),
        bind_as: None,
    };
    codec::write_frame(&mut peek, &hello2).await.unwrap();
    let _: BusReply = codec::read_frame(&mut peek).await.unwrap();
    codec::write_frame(&mut peek, &BusMessage::Sessions {})
        .await
        .unwrap();
    let reply: BusReply = codec::read_frame(&mut peek).await.unwrap();

    match reply {
        BusReply::SessionsOk { rows } => {
            let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
            assert!(names.contains(&"real"), "real must appear, got {names:?}");
            assert!(
                !names.contains(&"ghost"),
                "broker MUST NOT read ghost from sessions.jsonl (CLI-11): {names:?}"
            );
        }
        other => panic!("unexpected reply: {other:?}"),
    }

    drop(peek);
    drop(s);
    broker.abort();
}

/// BROKER-05 (integration): the public `is_nfs` API returns `false` on
/// a non-NFS tempfile path, which is the only filesystem CI can guarantee.
/// Real NFS validation is captured in `02-VALIDATION.md` "Manual-Only
/// Verifications" and is not automatable in this test suite.
#[test]
fn test_nfs_warning() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(
        !famp::cli::broker::nfs_check::is_nfs(tmp.path()),
        "tempfile dir is not NFS; warning should not fire"
    );
}
