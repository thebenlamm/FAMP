#![allow(dead_code)]

use famp_bus::{
    DrainResult, FakeLiveness, InMemoryMailbox, LivenessProbe, MailboxErr, MailboxName,
    MailboxRead,
};

#[derive(Debug, Clone, Default)]
pub struct TestEnv {
    mailbox: InMemoryMailbox,
    liveness: FakeLiveness,
}

impl TestEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mailbox(&self) -> &InMemoryMailbox {
        &self.mailbox
    }

    pub fn liveness_mut(&mut self) -> &mut FakeLiveness {
        &mut self.liveness
    }
}

impl MailboxRead for TestEnv {
    fn drain_from(
        &self,
        name: &MailboxName,
        since_bytes: u64,
    ) -> Result<DrainResult, MailboxErr> {
        self.mailbox.drain_from(name, since_bytes)
    }
}

impl LivenessProbe for TestEnv {
    fn is_alive(&self, pid: u32) -> bool {
        self.liveness.is_alive(pid)
    }
}
