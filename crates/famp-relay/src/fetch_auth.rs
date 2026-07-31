//! Signed fetch authorization (Task 2, D-26).
//!
//! This module is stubbed by Task 1 to the single item
//! [`http`](crate::http)'s `RelayState` needs at the type level — the
//! full signed-fetch mechanism (the signed form,
//! `sign_fetch_auth`/`verify_fetch_auth`, the replay cache, and the rate
//! limiter) lands in Task 2's commit. See that commit for the complete
//! module doc and implementation.

/// Placeholder process-lifetime fetch-authorization state.
///
/// Task 2 replaces this with the real replay-cache + rate-limiter state;
/// the type name and constructor signature stay stable so
/// `http::RelayState` does not change shape when Task 2 lands.
#[derive(Default)]
pub struct FetchAuthState;

impl FetchAuthState {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
