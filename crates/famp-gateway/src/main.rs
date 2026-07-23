//! `famp-gateway` — killable process backing 1+ remote principals on the
//! local UDS bus (LIVE-02: a real OS process the broker's `kill(pid,0)`
//! liveness sweep can observe alive or dead).
//!
//! Skeleton only in this task: the real CLI-arg parsing, `GatewayRegistry`
//! wiring, and parking loop land in Task 2 of this plan.

// Silencers for dependencies not yet wired by this task (this binary crate
// is a separate compilation unit from lib.rs and needs its own). Remove
// each line as Task 2 wires the real registry/connect/parking logic.
use famp as _;
use famp_bus as _;
use famp_gateway as _;
use thiserror as _;
use tokio as _;

// Silencer for the dev-only dependency: no test file in this crate uses
// it yet (lands in a later plan in this phase). Remove once wired.
#[cfg(test)]
use assert_cmd as _;

fn main() {
    println!("famp-gateway: skeleton binary (backing logic lands in Task 2)");
}
