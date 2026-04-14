//! Graceful-shutdown signal future for `famp listen`.
//!
//! Resolves on the first of SIGINT (Ctrl-C) or SIGTERM. Unix-only per v0.8
//! scope (CONTEXT §Graceful Shutdown — "Windows is out of scope for v0.8").
//! The non-unix cfg branch degrades to `ctrl_c` only so the crate still
//! compiles on other targets.

pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        // If installing the SIGTERM handler fails, fall back to ctrl_c only.
        // Degraded but not silent — the daemon still shuts down on SIGINT.
        let Ok(mut sigterm) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn shutdown_signal_is_a_future() {
        // Smoke test: the function returns a future. Drop it without
        // awaiting to avoid waiting for a real signal. Real signal-delivery
        // tests live in Plan 02-03.
        let _f = super::shutdown_signal();
    }
}
