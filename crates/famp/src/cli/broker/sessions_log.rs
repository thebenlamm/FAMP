//! Append-only diagnostic JSONL for `~/.famp/sessions.jsonl`.
//!
//! CLI-11: diagnostic-only; broker MUST NOT read this file back. The
//! authoritative session list comes from `BusReply::SessionsOk`, which
//! is built from in-memory broker state. This file exists only as an
//! observability aid for operators (`tail -f sessions.jsonl`).

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;

use famp_bus::SessionRow;

/// Append one canonical-JSON-encoded `SessionRow` to
/// `<bus_dir>/sessions.jsonl`. Mode 0600 on Unix; created on first call.
///
/// CLI-11: diagnostic-only; broker MUST NOT read this file back.
pub fn append_session_row(bus_dir: &Path, row: &SessionRow) -> Result<(), std::io::Error> {
    let path = bus_dir.join("sessions.jsonl");
    let bytes = famp_canonical::canonicalize(row)
        .map_err(|e| std::io::Error::other(format!("sessions.jsonl canonicalize: {e}")))?;

    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }

    let mut file = opts.open(&path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn append_session_row_writes_jsonl() {
        let tmp = tempfile::TempDir::new().unwrap();
        let row = SessionRow {
            name: "alice".into(),
            pid: 12345,
            joined: vec!["#planning".into()],
        };
        append_session_row(tmp.path(), &row).unwrap();
        let bytes = std::fs::read(tmp.path().join("sessions.jsonl")).unwrap();
        assert!(bytes.ends_with(b"\n"));
        let trimmed = &bytes[..bytes.len() - 1];
        let v: serde_json::Value = serde_json::from_slice(trimmed).unwrap();
        assert_eq!(v["name"], "alice");
        assert_eq!(v["pid"], 12345);
    }

    #[test]
    fn append_session_row_appends_not_truncates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let r1 = SessionRow {
            name: "a".into(),
            pid: 1,
            joined: vec![],
        };
        let r2 = SessionRow {
            name: "b".into(),
            pid: 2,
            joined: vec![],
        };
        append_session_row(tmp.path(), &r1).unwrap();
        append_session_row(tmp.path(), &r2).unwrap();
        let bytes = std::fs::read(tmp.path().join("sessions.jsonl")).unwrap();
        let count = bytes
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .count();
        assert_eq!(count, 2);
    }
}
