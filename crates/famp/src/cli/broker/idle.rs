//! Idle-timer state machine helper for the broker run loop.
//!
//! The broker arms a 5-minute `tokio::time::Sleep` whenever the live
//! client count drops `1 → 0` (per BROKER-04). A new connection cancels
//! the timer by setting the slot back to `None`. The select arm needs a
//! future that:
//!   - polls the inner `Sleep` when `idle = Some(_)`
//!   - never resolves when `idle = None`
//!
//! `wait_or_never` is that future. Implementation matches RESEARCH §5.

use std::pin::Pin;

/// Polls the wrapped sleep when present; otherwise hangs forever.
///
/// Returns `()` when the inner sleep elapses. Pinning is required because
/// `tokio::time::Sleep` is `!Unpin`; the broker keeps the box pinned in
/// place across select-loop iterations.
pub async fn wait_or_never(idle: &mut Option<Pin<Box<tokio::time::Sleep>>>) {
    match idle {
        Some(s) => s.as_mut().await,
        None => std::future::pending::<()>().await,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(start_paused = true)]
    async fn wait_or_never_pending_when_none() {
        let mut idle: Option<Pin<Box<tokio::time::Sleep>>> = None;
        let res = tokio::time::timeout(Duration::from_millis(10), wait_or_never(&mut idle)).await;
        assert!(res.is_err(), "wait_or_never must hang when idle=None");
    }

    #[tokio::test(start_paused = true)]
    async fn wait_or_never_resolves_when_some() {
        let mut idle: Option<Pin<Box<tokio::time::Sleep>>> =
            Some(Box::pin(tokio::time::sleep(Duration::from_secs(1))));
        tokio::time::advance(Duration::from_secs(2)).await;
        wait_or_never(&mut idle).await;
    }
}
