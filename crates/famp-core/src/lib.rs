//! `famp-core` — FAMP v0.5.1 shared value types.
#![forbid(unsafe_code)]

pub mod identity;
pub mod ids;
pub mod artifact;

pub use identity::{Principal, Instance, ParsePrincipalError, ParseInstanceError};
pub use ids::{MessageId, ConversationId, TaskId, CommitmentId};
pub use artifact::{ArtifactId, ParseArtifactIdError};

// serde_json is used by integration tests under `tests/` which are separate
// crates; silence the workspace `unused_crate_dependencies` warning for the
// library test profile.
#[cfg(test)]
use serde_json as _;
