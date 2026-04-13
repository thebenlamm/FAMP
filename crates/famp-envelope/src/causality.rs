//! Envelope causality sub-struct.
//!
//! v0.7 ships the five relations actually emitted by the Personal Runtime
//! (`acknowledges`, `requests`, `commits`, `delivers`, `cancels`). The other
//! six relations from the v0.5.1 §7.1 / §13 / §11.3 catalog are deferred to
//! v0.9 Causality & Replay Defense (ENV-13). Re-widening this enum in v0.7
//! is intentionally a compiler-visible break.

use famp_core::MessageId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Relation {
    Acknowledges,
    Requests,
    Commits,
    Delivers,
    Cancels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Causality {
    pub rel: Relation,
    #[serde(rename = "ref")]
    pub referenced: MessageId,
}
