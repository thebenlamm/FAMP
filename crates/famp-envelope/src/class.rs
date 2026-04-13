//! `MessageClass` — re-exported from `famp-core` for backward compatibility.
//!
//! The canonical definition lives in `famp_core::class`. This module re-exports
//! it so existing code using `famp_envelope::MessageClass` continues to work.

pub use famp_core::MessageClass;
