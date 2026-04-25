//! `famp await` — block until a new inbox entry arrives past the cursor.
//!
//! # Relationship to `famp inbox list`
//!
//! `await` is deliberately unfiltered. `famp inbox list` filters out
//! entries for tasks in a terminal FSM state by default (spec
//! `2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`), which
//! means the closing `deliver` for a task you originated is NOT
//! visible via `list` after the daemon flips the taskdir record.
//!
//! `await` IS the canonical real-time signal for task completion.
//! Agents waiting for a task to close should `await`; `list` is for
//! "what's still on my plate."
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
//! ## Phase 4: FSM advance on commit envelopes
//!
//! When a matched entry has `class == "commit"` and its `task_id` matches a
//! local task record in `TaskDir`, `advance_committed` is called on the record
//! before printing the structured line. This drives REQUESTED → COMMITTED on
//! the originator side without any test-only state seeding.
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

use famp_inbox::{read::read_from, InboxCursor, InboxLock};
use famp_taskdir::TaskDir;

use crate::cli::error::{parse_duration, CliError};
use crate::cli::send::fsm_glue::advance_committed;
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

/// Structured outcome returned by [`run_at_structured`]. Maps to the JSON
/// shape locked by Phase 3 Plan 03-03 but without printing.
#[derive(Debug, Clone)]
pub struct AwaitOutcome {
    pub offset: u64,
    pub task_id: String,
    pub from: String,
    pub class: String,
    pub body: serde_json::Value,
}

/// Top-level entry point. Resolves `FAMP_HOME` and forwards to [`run_at`].
pub async fn run(args: AwaitArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout();
    run_at(&home, args, &mut stdout).await
}

/// Structured entry — returns [`AwaitOutcome`] without printing. Used by the
/// MCP tool wrapper so it can embed the matched entry as a JSON-RPC result.
pub async fn run_at_structured(home: &Path, args: AwaitArgs) -> Result<AwaitOutcome, CliError> {
    let mut buf = Vec::<u8>::new();
    run_at(home, args, &mut buf).await?;
    // `run_at` writes exactly one JSON line on success.
    let line = std::str::from_utf8(&buf)
        .map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?
        .trim_end();
    let value: serde_json::Value = serde_json::from_str(line).map_err(|e| CliError::Io {
        path: std::path::PathBuf::new(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    Ok(AwaitOutcome {
        offset: value
            .get("offset")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        task_id: value
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        from: value
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        class: value
            .get("class")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        body: value
            .get("body")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    })
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

    // Advisory lock (Plan 03-04 INBOX-05): fail-fast if another
    // single-consumer reader holds the lock. Held for the lifetime of
    // this call; dropped on return (happy path, timeout, or error).
    let _lock = InboxLock::acquire(home).map_err(CliError::Inbox)?;

    let deadline = Instant::now() + timeout;

    loop {
        let start = cursor.read().await?;
        let entries = read_from(&inbox_path, start).map_err(CliError::Inbox)?;

        if let Some((value, advance_to)) = poll::find_match(&entries, &args.task) {
            // Phase 4: if this is a commit-class envelope matching a local
            // task in REQUESTED, advance the record to COMMITTED before
            // printing. This is the T-04-07 mitigation — only advances when
            // class == "commit" AND task_id matches a local record.
            let class = value.get("class").and_then(|v| v.as_str()).unwrap_or("");
            let task_id_str = value.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            if class == "commit" && !task_id_str.is_empty() {
                let tasks_dir = paths::tasks_dir(home);
                match TaskDir::open(&tasks_dir) {
                    Ok(tasks) => {
                        // No matching local record — not our task; nothing to advance.
                        // An Err from tasks.read is silently skipped (matches the
                        // prior `if tasks.read(...).is_ok()` behavior — a commit
                        // envelope for someone else's task is not an error here).
                        if let Ok(mut record) = tasks.read(task_id_str) {
                            // Run the FSM advance OUTSIDE the update closure so we
                            // can observe the result. TaskDir::update's closure is
                            // FnOnce(TaskRecord) -> TaskRecord with no Result, so
                            // an in-closure error has nowhere to go.
                            match advance_committed(&mut record) {
                                Ok(_) => {
                                    if let Err(e) = tasks.update(task_id_str, |_| record.clone()) {
                                        eprintln!(
                                            "famp await: failed to persist commit-advance for task {task_id_str}: {e}"
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "famp await: advance_committed failed for task {task_id_str}: {e}"
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "famp await: failed to open task dir while handling commit for {task_id_str}: {e}"
                        );
                    }
                }
            }

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
