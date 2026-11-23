//! W0: a per-box RECIPIENT allowlist for gateway-origin messages (default deny).
//!
//! When the gateway is started with `--recipient-allowlist <path>`, a verified
//! message is delivered only if its recipient NAME is in that file. The file is
//! read on EVERY delivery, so editing it takes effect on the next message with
//! no restart, and any problem reading it denies everything (fail closed).
//!
//! This is enforced in the gateway's `deliver` path, which both the HTTP ingress
//! and the relay-fetch loop share. It closes D-22 (the audience check binds only
//! the recipient DOMAIN) per box: without it the broker would create a mailbox
//! for any name a verified peer addresses.
//!
//! Format (written by `famp network allow|deny`, tolerant of nothing else):
//! `{"version":1,"names":["alice","guardian-devgpu"]}`. The file must be at most
//! [`MAX_BYTES`], and not group- or world-writable.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use serde::Deserialize;

/// Upper bound on the allowlist file size.
pub const MAX_BYTES: u64 = 64 * 1024;

/// Why a delivery was refused. Never sent to the caller; operator log only.
#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    Missing,
    Unreadable(String),
    Unsafe(String),
    Malformed(String),
    NotListed,
}

impl std::fmt::Display for Denied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "allowlist file is missing (default deny)"),
            Self::Unreadable(e) => write!(f, "allowlist file is unreadable: {e}"),
            Self::Unsafe(e) => write!(f, "allowlist file is unsafe: {e}"),
            Self::Malformed(e) => write!(f, "allowlist file is malformed: {e}"),
            Self::NotListed => write!(f, "recipient is not on the allowlist"),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    names: Vec<String>,
}

/// Parse the file contents. Any deviation from the exact format is an error.
fn parse(bytes: &[u8]) -> Result<Vec<String>, String> {
    let file: File = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if file.version != 1 {
        return Err(format!("unsupported version {}", file.version));
    }
    if file.names.iter().any(|n| n.is_empty() || n.len() > 253) {
        return Err("a name is empty or too long".to_owned());
    }
    Ok(file.names)
}

/// Allow `name` only if the allowlist at `path` currently lists it.
///
/// # Errors
///
/// A [`Denied`] reason for every refusal, including any I/O or parse failure.
pub async fn check(path: &Path, name: &str) -> Result<(), Denied> {
    use tokio::io::AsyncReadExt as _;

    // A symlink could point the check at a file someone else controls.
    match tokio::fs::symlink_metadata(path).await {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(Denied::Unsafe("the allowlist is a symlink".to_owned()))
        }
        // A FIFO or device would block or misbehave on open; only a regular file passes.
        Ok(m) if !m.file_type().is_file() => {
            return Err(Denied::Unsafe("not a regular file".to_owned()))
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Denied::Missing),
        Err(e) => return Err(Denied::Unreadable(e.to_string())),
    }
    // Open ONCE and check the open handle (fstat), then read from that same handle,
    // so the file cannot change between the checks and the read.
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| Denied::Unreadable(e.to_string()))?;
    let meta = file
        .metadata()
        .await
        .map_err(|e| Denied::Unreadable(e.to_string()))?;
    if !meta.is_file() {
        return Err(Denied::Unsafe("not a regular file".to_owned()));
    }
    if meta.len() > MAX_BYTES {
        return Err(Denied::Unsafe(format!("larger than {MAX_BYTES} bytes")));
    }
    let mode = meta.permissions().mode();
    if mode & 0o022 != 0 {
        return Err(Denied::Unsafe(format!(
            "mode {:o} lets other users write it",
            mode & 0o777
        )));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| Denied::Unreadable(e.to_string()))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Denied::Unsafe(format!("larger than {MAX_BYTES} bytes")));
    }
    let names = parse(&bytes).map_err(Denied::Malformed)?;
    if names.iter().any(|n| n == name) {
        Ok(())
    } else {
        Err(Denied::NotListed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn write(dir: &Path, text: &str, mode: u32) -> std::path::PathBuf {
        let p = dir.join("recipients.json");
        std::fs::write(&p, text).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        p
    }

    #[tokio::test]
    async fn only_exact_listed_names_pass() {
        let d = tempfile::tempdir().unwrap();
        let p = write(d.path(), r#"{"version":1,"names":["alice","bob"]}"#, 0o600);
        assert_eq!(check(&p, "alice").await, Ok(()));
        assert_eq!(check(&p, "alic").await, Err(Denied::NotListed));
        assert_eq!(check(&p, "alice2").await, Err(Denied::NotListed));
    }

    #[tokio::test]
    async fn every_kind_of_trouble_denies() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("nope.json");
        assert_eq!(check(&missing, "alice").await, Err(Denied::Missing));
        for (text, mode) in [
            ("not json", 0o600),
            (r#"{"version":2,"names":["alice"]}"#, 0o600),
            (r#"{"version":1,"names":["alice"],"extra":1}"#, 0o600),
            (r#"{"version":1,"names":[""]}"#, 0o600),
            // A torn write that truncated the list must not allow a prefix.
            (r#"{"version":1,"names":["alice","gua"#, 0o600),
            // Valid content but writable by others.
            (r#"{"version":1,"names":["alice"]}"#, 0o666),
            (r#"{"version":1,"names":["alice"]}"#, 0o620),
        ] {
            let p = write(d.path(), text, mode);
            let got = check(&p, "alice").await;
            assert!(
                got.is_err() && got != Err(Denied::NotListed),
                "{text} {mode:o}: {got:?}"
            );
        }
        // A symlink to a perfectly good file is still refused.
        let target = write(d.path(), r#"{"version":1,"names":["alice"]}"#, 0o600);
        let link = d.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(matches!(
            check(&link, "alice").await,
            Err(Denied::Unsafe(_))
        ));
        let big = format!(r#"{{"version":1,"names":["{}"]}}"#, "a".repeat(70_000));
        let p = write(d.path(), &big, 0o600);
        assert!(matches!(check(&p, "alice").await, Err(Denied::Unsafe(_))));
    }
}
