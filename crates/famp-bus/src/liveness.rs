//! PID liveness probe abstraction for pure broker tests.

use std::collections::BTreeSet;

pub trait LivenessProbe {
    fn is_alive(&self, pid: u32) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysAliveLiveness;

impl LivenessProbe for AlwaysAliveLiveness {
    fn is_alive(&self, _pid: u32) -> bool {
        true
    }
}

#[derive(Debug, Default, Clone)]
pub struct FakeLiveness {
    dead_pids: BTreeSet<u32>,
}

impl FakeLiveness {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_dead(&mut self, pid: u32) {
        self.dead_pids.insert(pid);
    }

    pub fn mark_alive(&mut self, pid: u32) {
        self.dead_pids.remove(&pid);
    }
}

impl LivenessProbe for FakeLiveness {
    fn is_alive(&self, pid: u32) -> bool {
        !self.dead_pids.contains(&pid)
    }
}
