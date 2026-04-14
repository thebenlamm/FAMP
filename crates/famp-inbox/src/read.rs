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

/// Read every complete JSONL line past `start_offset`.
///
/// Returns each value alongside the byte offset of the first byte
/// AFTER that line. The per-entry `end_offset` is the cursor value
/// callers should advance to if they want to consume exactly that
/// one entry.
///
/// Semantics:
/// - `start_offset = 0` yields every complete line in the file.
/// - `start_offset >= file_len` yields `vec![]` (clamped; not an error).
/// - A `start_offset` that falls mid-line is snapped forward to the
///   next byte after the next `\n`. No partial-line reads. This
///   mirrors how `InboxCursor::advance` is used: callers always set
///   the cursor to a line boundary, but a truncated / rewritten file
///   could leave a stale cursor mid-line, and we recover gracefully.
/// - Tail tolerance: a final partial line (no terminating newline)
///   is silently dropped, and `end_offset` stops at the last
///   newline — this matches `read_all`'s crash-recovery behavior.
/// - A non-terminal line that fails to parse is a hard
///   [`InboxError::CorruptLine`] error (same as `read_all`).
pub fn read_from(
    path: impl AsRef<Path>,
    start_offset: u64,
) -> Result<Vec<(serde_json::Value, u64)>, InboxError> {
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
    let file_len = bytes.len() as u64;
    if start_offset >= file_len {
        return Ok(Vec::new());
    }
    let Ok(start) = usize::try_from(start_offset) else {
        return Ok(Vec::new());
    };

    // Snap forward to the next `\n + 1` boundary if `start` is mid-line.
    // Convention: byte at position `start - 1` SHOULD be `\n` (cursors
    // are always advanced to line boundaries). If not, skip forward to
    // the next newline and discard the partial leading bytes.
    let snapped = if start == 0 || bytes.get(start - 1) == Some(&b'\n') {
        start
    } else {
        match bytes[start..].iter().position(|&b| b == b'\n') {
            Some(off) => start + off + 1,
            None => return Ok(Vec::new()),
        }
    };
    if snapped >= bytes.len() {
        return Ok(Vec::new());
    }

    let slice = &bytes[snapped..];
    let file_ends_with_newline = bytes.last() == Some(&b'\n');

    // Walk slice, splitting on `\n`. Track a running offset so each
    // complete line gets its own `end_offset`. The final chunk from
    // `split` is either the post-newline empty string (clean EOF) or
    // a partial tail (silently dropped).
    let chunks: Vec<&[u8]> = slice.split(|&b| b == b'\n').collect();
    let total = chunks.len();
    let mut out: Vec<(serde_json::Value, u64)> = Vec::new();
    let mut running = snapped as u64;

    for (idx, chunk) in chunks.iter().enumerate() {
        let is_last = idx + 1 == total;
        if is_last {
            // Clean EOF => this chunk is the empty string after the
            // final `\n`; `running` already accounts for it.
            // Dirty EOF (partial line) => drop silently, do not
            // advance `running` past it.
            debug_assert!(file_ends_with_newline || !chunk.is_empty() || chunks.len() == 1);
            let _ = file_ends_with_newline; // semantic marker; drop handled implicitly
            continue;
        }

        let line_no = idx + 1;
        match serde_json::from_slice::<serde_json::Value>(chunk) {
            Ok(value) => {
                // +1 for the terminating `\n`
                running += (chunk.len() as u64) + 1;
                out.push((value, running));
            }
            Err(source) => {
                return Err(InboxError::CorruptLine { line_no, source });
            }
        }
    }

    Ok(out)
}
