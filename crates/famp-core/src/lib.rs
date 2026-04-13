//! `famp-core` — FAMP v0.5.1 shared value types.
#![forbid(unsafe_code)]

pub mod artifact;
pub mod identity;
pub mod ids;

pub use artifact::{ArtifactId, ParseArtifactIdError};
pub use identity::{Instance, ParseInstanceError, ParsePrincipalError, Principal};
pub use ids::{CommitmentId, ConversationId, MessageId, TaskId};

// serde_json is used by integration tests under `tests/` which are separate
// crates; silence the workspace `unused_crate_dependencies` warning for the
// library test profile.
#[cfg(test)]
use serde_json as _;
