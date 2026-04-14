//! Shared test helpers. Consumed via
//! `#[path = "common/cycle_driver.rs"] mod cycle_driver;` from each test binary
//! that needs the cycle driver.
//!
//! This file exists only so `crates/famp/tests/common/` is a valid directory
//! in Cargo's view; the actual helper lives in `cycle_driver.rs`.
