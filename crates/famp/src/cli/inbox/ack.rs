//! `famp inbox ack` — manual cursor advance with no output.

use std::path::Path;

use famp_inbox::InboxCursor;

use crate::cli::error::CliError;
use crate::cli::paths;

/// Advance the inbox cursor to `offset`. Trusts the caller —
/// does not validate that `offset` sits on a JSONL line boundary.
pub async fn run_ack(home: &Path, offset: u64) -> Result<(), CliError> {
    let cursor = InboxCursor::at(paths::inbox_cursor_path(home));
    cursor.advance(offset).await?;
    Ok(())
}
