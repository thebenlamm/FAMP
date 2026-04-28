//! Envelope causality sub-struct.
//!
//! v0.5.2 ships six relations: the five Personal Runtime relations plus
//! `audits`, added by Delta 32 and distinct from `acknowledges` per D-16.

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
    Audits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Causality {
    pub rel: Relation,
    #[serde(rename = "ref")]
    pub referenced: MessageId,
}
