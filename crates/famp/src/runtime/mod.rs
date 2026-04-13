//! Runtime glue: the one place that imports envelope + fsm + transport +
//! keyring together. Phase 3 D-D1 — no separate `famp-runtime` crate.

pub mod adapter;
pub mod error;
pub mod loop_fn;
pub mod peek;

pub use adapter::fsm_input_from_envelope;
pub use error::RuntimeError;
pub use loop_fn::process_one_message;
pub use peek::peek_sender;
