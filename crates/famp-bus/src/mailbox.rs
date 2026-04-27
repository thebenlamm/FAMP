//! Read-only mailbox trait and deterministic in-memory test implementation.

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

mod private {
    pub trait Sealed {}
    impl<T> Sealed for T {}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MailboxName {
    Agent(String),
    Channel(String),
}

impl fmt::Display for MailboxName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(name) | Self::Channel(name) => f.write_str(name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainResult {
    pub lines: Vec<Vec<u8>>,
    pub next_offset: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum MailboxErr {
    #[error("mailbox {name} does not exist")]
    NotFound { name: MailboxName },
    #[error("cursor offset {requested} exceeds mailbox size {actual}")]
    CursorOutOfRange { requested: u64, actual: u64 },
    #[error("internal mailbox error: {0}")]
    Internal(String),
}

pub trait MailboxRead: private::Sealed {
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemoryMailbox {
    boxes: Arc<Mutex<BTreeMap<MailboxName, Vec<Vec<u8>>>>>,
}

impl InMemoryMailbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&self, name: &MailboxName, line: Vec<u8>) {
        let Ok(mut guard) = self.boxes.lock() else {
            panic!("in-memory mailbox lock poisoned");
        };
        guard.entry(name.clone()).or_default().push(line);
    }
}

impl MailboxRead for InMemoryMailbox {
    #[allow(clippy::significant_drop_tightening)]
    fn drain_from(&self, name: &MailboxName, since_bytes: u64) -> Result<DrainResult, MailboxErr> {
        let entries = {
            let guard = self
                .boxes
                .lock()
                .map_err(|_| MailboxErr::Internal("in-memory mailbox lock poisoned".into()))?;
            let Some(entries) = guard.get(name) else {
                return Ok(DrainResult {
                    lines: Vec::new(),
                    next_offset: since_bytes,
                });
            };
            entries.clone()
        };

        let total: u64 = entries.iter().map(|line| (line.len() + 1) as u64).sum();
        if since_bytes > total {
            return Err(MailboxErr::CursorOutOfRange {
                requested: since_bytes,
                actual: total,
            });
        }

        let mut cursor = 0_u64;
        let mut lines = Vec::new();
        for line in entries {
            // Each line's size is `line.len() + 1` (line + `\n`) to mirror
            // the disk JSONL format from `famp-inbox/src/read.rs` lines 38-82.
            let next = cursor + (line.len() + 1) as u64;
            if cursor >= since_bytes {
                lines.push(line.clone());
            }
            cursor = next;
        }

        Ok(DrainResult {
            lines,
            next_offset: cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{InMemoryMailbox, MailboxName, MailboxRead};

    #[test]
    fn in_memory_mailbox_accounts_for_newline_offsets() {
        let mailbox = InMemoryMailbox::new();
        let name = MailboxName::Agent("alice".into());

        mailbox.append(&name, b"line1".to_vec());
        let drained = mailbox.drain_from(&name, 0).unwrap();

        assert_eq!(drained.lines, vec![b"line1".to_vec()]);
        assert_eq!(drained.next_offset, 6);
    }
}
