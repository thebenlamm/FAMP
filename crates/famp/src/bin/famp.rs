#![forbid(unsafe_code)]
// The famp lib crate will be used by the binary once Phase 8 re-exports land.
// For now, silence the workspace-level unused_crate_dependencies warning.
#![allow(unused_crate_dependencies)]

fn main() {
    println!("famp v0.5.1 placeholder");
}
