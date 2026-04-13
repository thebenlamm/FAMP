//! `famp-core` — FAMP v0.5.1 shared value types.
#![forbid(unsafe_code)]

pub mod artifact;
pub mod error;
pub mod identity;
pub mod ids;
pub mod invariants;
pub mod scope;

pub use artifact::{ArtifactId, ParseArtifactIdError};
pub use error::{ProtocolError, ProtocolErrorKind};
pub use identity::{Instance, ParseInstanceError, ParsePrincipalError, Principal};
pub use ids::{CommitmentId, ConversationId, MessageId, TaskId};
pub use scope::{AuthorityScope, ParseAuthorityScopeError};

// serde_json is used by integration tests under `tests/` which are separate
// crates; silence the workspace `unused_crate_dependencies` warning for the
// library test profile.
#[cfg(test)]
use serde_json as _;
