//! Read-side of the inbox: tail-tolerant, mid-file-strict.
//!
//! Contract:
//! - Every complete JSONL line (i.e., terminated by `\n`) must parse as
//!   valid JSON, or [`InboxError::CorruptLine`] is returned identifying
//!   the offending 1-indexed line number.
//! - If the file's final byte is NOT `\n`, the trailing chunk is a
//!   partial line (the daemon crashed mid-write). The partial line is
//!   silently skipped — this is the crash-recovery tolerance Phase 2
//!   requires.
//! - An empty file yields an empty vector.

use std::path::Path;

use crate::InboxError;

/// Read every complete JSONL line from `path` as `serde_json::Value`s.
///
/// Synchronous by design: called from daemon cold-start and from
/// `famp inbox` / `famp await` read paths, never from the hot HTTP
/// handler.
pub fn read_all(path: impl AsRef<Path>) -> Result<Vec<serde_json::Value>, InboxError> {
    let path = path.as_ref();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(source) => {
            return Err(InboxError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let file_ends_with_newline = bytes.last() == Some(&b'\n');

    // `split(b'\n')` on `b"a\nb\n"` yields `["a", "b", ""]`; on `b"a\nb"`
    // yields `["a", "b"]`. So: if the file ends with `\n`, the final
    // element from `split` is an empty trailing chunk we discard. If it
    // does not, the final element is the partial tail.
    let chunks: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();

    let mut out = Vec::new();
    let total = chunks.len();

    for (idx, chunk) in chunks.iter().enumerate() {
        let is_last = idx + 1 == total;
        let line_no = idx + 1;

        if is_last {
            if file_ends_with_newline {
                // Final empty element after the terminating newline —
                // not a real line, skip.
                debug_assert!(chunk.is_empty());
                continue;
            }
            // Partial tail from a mid-write crash. Try to parse; on any
            // failure, silently drop it.
            if chunk.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(chunk) {
                out.push(value);
            }
            // else: swallowed — tail tolerance.
            continue;
        }

        // Non-terminal line. Must parse or we return a hard error.
        match serde_json::from_slice::<serde_json::Value>(chunk) {
            Ok(value) => out.push(value),
            Err(source) => {
                return Err(InboxError::CorruptLine { line_no, source });
            }
        }
    }

    Ok(out)
}
