//! `famp-inbox` — durable JSONL inbox for inbound signed envelopes.
//!
//! Public surface:
//! - [`Inbox::open`] — open/create a 0600-mode JSONL file
//! - [`Inbox::append`] — append raw envelope bytes with fsync-before-return
//! - [`read::read_all`] — read every complete line, tail-tolerant
//! - [`InboxError`] — narrow error enum
//!
//! The append path takes already-serialized envelope bytes (`&[u8]`) rather
//! than a typed envelope. This preserves byte-exactness (P3): the bytes that
//! were signed on the wire are the bytes that land on disk — no typed
//! decode-then-re-encode round-trip.

#![forbid(unsafe_code)]

pub mod append;
pub mod cursor;
pub mod error;
pub mod lock;
pub mod read;

pub use crate::append::Inbox;
pub use crate::cursor::InboxCursor;
pub use crate::error::InboxError;
pub use crate::lock::InboxLock;
