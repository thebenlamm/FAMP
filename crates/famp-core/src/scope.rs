//! Authority scope (FAMP v0.5.1 §5.3).
//!
//! Five-level ladder: `advisory` < `negotiate` < `commit_local` < `commit_delegate` < `transfer`.
//! The ladder is *semantic*, not lexical, so we do NOT derive `Ord`/`PartialOrd` —
//! auto-derived ordering would couple correctness to declaration order (D-31).
//! Instead, `satisfies` routes through a private `rank` helper whose exhaustive
//! match makes any new variant a compile error until the ladder is updated.

use std::fmt;
use std::str::FromStr;

/// The five authority scopes defined by FAMP v0.5.1 §5.3.
///
/// Wire form is `snake_case` — asserted by the `scope_wire_strings` fixture test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityScope {
    Advisory,
    Negotiate,
    CommitLocal,
    CommitDelegate,
    Transfer,
}

impl AuthorityScope {
    /// Canonical wire string for this scope (private source of truth for
    /// `Display` and `FromStr`).
    const fn as_wire(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::Negotiate => "negotiate",
            Self::CommitLocal => "commit_local",
            Self::CommitDelegate => "commit_delegate",
            Self::Transfer => "transfer",
        }
    }

    /// Private rank helper — MUST NOT be `pub` (D-33). Exposing ranks would
    /// leak declaration-order semantics into the public API.
    const fn rank(self) -> u8 {
        match self {
            Self::Advisory => 0,
            Self::Negotiate => 1,
            Self::CommitLocal => 2,
            Self::CommitDelegate => 3,
            Self::Transfer => 4,
        }
    }

    /// Returns true iff this scope grants at least the authority of `required`,
    /// per the ladder in spec §5.3
    /// (`advisory` < `negotiate` < `commit_local` < `commit_delegate` < `transfer`).
    #[must_use]
    pub const fn satisfies(self, required: Self) -> bool {
        self.rank() >= required.rank()
    }
}

impl fmt::Display for AuthorityScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// Parse error for `AuthorityScope::from_str`. Narrow, type-local, and
/// deliberately distinct from `ProtocolErrorKind` per D-35.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown authority scope")]
pub struct ParseAuthorityScopeError;

impl FromStr for AuthorityScope {
    type Err = ParseAuthorityScopeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "advisory" => Ok(Self::Advisory),
            "negotiate" => Ok(Self::Negotiate),
            "commit_local" => Ok(Self::CommitLocal),
            "commit_delegate" => Ok(Self::CommitDelegate),
            "transfer" => Ok(Self::Transfer),
            _ => Err(ParseAuthorityScopeError),
        }
    }
}
