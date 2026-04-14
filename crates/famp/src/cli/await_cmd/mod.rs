//! `famp await` — block until a new inbox entry arrives past the cursor.
//!
//! Polls `inbox.jsonl` every 250 ms (matches REQUIREMENTS.md INBOX-03),
//! reads every line past the current cursor via `famp_inbox::read::read_from`,
//! and:
//!
//! - If any entry matches the optional `--task <id>` filter, prints ONE
//!   structured JSON line (see [`poll::find_match`] for the exact shape),
//!   advances the cursor PAST that single entry, and exits 0. Remaining
//!   entries from the same batch are left for subsequent `await` calls.
//! - If a `--task` filter is set and none of the read entries match,
//!   advances the cursor past the whole batch (consume-and-discard) so
//!   we do not re-poll the same already-rejected bytes forever.
//! - On timeout, returns [`CliError::AwaitTimeout`] with the original
//!   string and leaves the cursor untouched.
//!
//! The output JSON shape is locked by Phase 3 Plan 03-03 and documented
//! in that plan's SUMMARY:
//!
//! ```json
//! { "offset": <u64>, "task_id": "<str>", "from": "<str>",
//!   "class": "<str>", "body": <json> }
//! ```
//!
//! `offset` is the byte offset AFTER the consumed line (the cursor
//! value after advance).

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use famp_inbox::{read::read_from, InboxCursor};

use crate::cli::error::{parse_duration, CliError};
use crate::cli::{home, paths};

pub mod poll;

pub const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(clap::Args, Debug)]
pub struct AwaitArgs {
    /// Block timeout. Accepts "30s", "5m", "1h", "250ms".
    #[arg(long, default_value = "30s")]
    pub timeout: String,
    /// Optional task-id filter — only return envelopes whose top-level
    /// `task_id` field equals this value.
    #[arg(long)]
    pub task: Option<String>,
}

/// Top-level entry point. Resolves `FAMP_HOME` and forwards to [`run_at`].
pub async fn run(args: AwaitArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout();
    run_at(&home, args, &mut stdout).await
}

/// Core polling loop. `out` lets tests capture the one structured
/// line without a process boundary.
pub async fn run_at(
    home: &Path,
    args: AwaitArgs,
    out: &mut (dyn Write + Send),
) -> Result<(), CliError> {
    let timeout = parse_duration(&args.timeout)?;
    let inbox_path = paths::inbox_jsonl_path(home);
    let cursor = InboxCursor::at(paths::inbox_cursor_path(home));
    let deadline = Instant::now() + timeout;

    loop {
        let start = cursor.read().await?;
        let entries = read_from(&inbox_path, start).map_err(CliError::Inbox)?;

        if let Some((value, advance_to)) = poll::find_match(&entries, &args.task) {
            let line = serde_json::to_string(&value).unwrap_or_default();
            writeln!(out, "{line}").map_err(|e| CliError::Io {
                path: inbox_path.clone(),
                source: e,
            })?;
            cursor.advance(advance_to).await?;
            return Ok(());
        }

        // If a filter consumed-and-discarded every entry in this batch,
        // advance past the whole batch so the next poll sees new bytes.
        if args.task.is_some() {
            if let Some((_, batch_end)) = entries.last() {
                cursor.advance(*batch_end).await?;
            }
        }

        if Instant::now() >= deadline {
            return Err(CliError::AwaitTimeout {
                timeout: args.timeout,
            });
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
