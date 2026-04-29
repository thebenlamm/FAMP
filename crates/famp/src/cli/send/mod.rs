//! `famp send` subcommand — Phase 02 Plan 02-04 (UDS bus rewire, D-10 proxy).
//!
//! v0.9 evolves `famp send` from the v0.8 HTTPS-via-`famp listen` transport
//! to the v0.9 UDS-via-`famp broker` transport. Identity binding follows
//! D-10: connection-level via `Hello { bind_as: Some(resolved_identity) }`,
//! NOT a per-message field. The `BusMessage::Send` shape is unchanged from
//! Phase 1 — `to: Target, envelope: serde_json::Value` only, no `act_as`.
//!
//! ## Three modes (preserved verbatim from v0.8)
//!
//! - `--new-task "<text>" --to <name>` → DM to agent.
//! - `--task <uuid>` → continue an existing task.
//! - `--task <uuid> --terminal` → terminal deliver (FSM-advances on the receiver).
//! - `--channel <#name>` → channel post (mutually exclusive with `--to`).
//!
//! ## Identity resolution (D-01 + D-10)
//!
//! 1. `--as <name>` (CLI flag, Tier 1).
//! 2. `$FAMP_LOCAL_IDENTITY` (Tier 2).
//! 3. cwd → `~/.famp-local/wires.tsv` exact match (Tier 3).
//! 4. Hard error: `CliError::NoIdentityBound`.
//!
//! The resolved identity is passed to `BusClient::connect(sock, Some(name))`,
//! which forwards it as `Hello { bind_as: Some(name) }`. The broker validates
//! at Hello time that `name` is held by a live `famp register` process and
//! rejects with `HelloErr { NotRegistered }` otherwise. Per-op liveness
//! re-check runs on every Send/Inbox/etc. — if the holder dies between
//! Hello and Send, the op returns `Err { NotRegistered }` for that op only.
//!
//! ## Output (JSON-Line on stdout)
//!
//! `{"task_id":"<uuid>","delivered":"<debug-of-Vec<Delivered>>"}`
//!
//! Mirrors the v0.8 send shape so MCP-tool output stays compatible. The
//! `delivered` field is a debug-format of the broker's `Vec<Delivered>`
//! reply because the `Delivered` struct lives in `famp-bus` and exposing
//! its full shape on stdout would couple the CLI surface to a wire-layer
//! crate; debug-stringify keeps the surface ergonomic for shell pipes.
//!
//! ## v0.8 federation (HTTPS) path
//!
//! The v0.8 HTTPS code in `client.rs` (`post_envelope`) is kept compilable
//! but no longer invoked from `run_at_structured`. Phase 4 deletes it.
//! The federation HTTPS path is exercised through `cli/listen` directly
//! (the `e2e_two_daemons.rs` integration tests are marked `#[ignore]` for
//! Phase 02; Phase 4 will migrate or delete them).

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply, Target};

use crate::bus_client::{BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

pub mod client;
pub mod fsm_glue;

/// Channel name validation (mirrors `famp_bus::proto::CHANNEL_PATTERN`).
/// Locally inlined because `famp_bus` does not export the regex publicly.
const CHANNEL_PATTERN: &str = r"^#[a-z0-9][a-z0-9_-]{0,31}$";

/// CLI arg set for `famp send`.
///
/// `--to` and `--channel` are mutually exclusive (exactly one required).
/// `--new-task` and `--task` are mutually exclusive (exactly one required).
/// `--terminal` requires `--task`.
/// `--more-coming` requires `--new-task` (clap-enforced + run-time guard).
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    /// Direct-message recipient identity (mutually exclusive with `--channel`).
    #[arg(long, conflicts_with = "channel")]
    pub to: Option<String>,
    /// Channel target (mutually exclusive with `--to`). Accepts both
    /// `planning` and `#planning`; rejects `##planning`.
    #[arg(long, conflicts_with = "to")]
    pub channel: Option<String>,
    /// Open a new task with the given natural-language summary.
    #[arg(long, conflicts_with = "task")]
    pub new_task: Option<String>,
    /// Continue an existing task (`UUIDv7` from a prior `--new-task`).
    #[arg(long, conflicts_with = "new_task")]
    pub task: Option<String>,
    /// Mark the deliver envelope terminal (requires `--task`).
    #[arg(long, requires = "task")]
    pub terminal: bool,
    /// Optional freeform body text (used as `natural_language_summary`).
    #[arg(long)]
    pub body: Option<String>,
    /// Signal "more briefing follows" on a `--new-task` envelope. Default
    /// false → key omitted, byte-exact backwards-compat.
    #[arg(long, requires = "new_task")]
    pub more_coming: bool,
    /// Override identity resolution (D-01 Tier 1: `--as` >
    /// `$FAMP_LOCAL_IDENTITY` > cwd→wires.tsv > error). The resolved
    /// identity becomes `Hello { bind_as: Some(name) }` per D-10.
    /// `--as` is the CLI surface; the Rust field is `act_as` because
    /// `as` is a reserved keyword.
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Outcome returned by [`run_at_structured`].
///
/// Carries the broker's `task_id` (`UUIDv7` string) and a debug-format of
/// the per-target delivery slice. JSON-Line shape on stdout:
/// `{"task_id":"<uuid>","delivered":"<debug>"}` — preserved from v0.8 for
/// MCP-tool output compatibility.
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The `UUIDv7` task id assigned by the broker.
    pub task_id: String,
    /// Debug-format of the broker's `Vec<Delivered>` reply slice. The
    /// `Delivered` struct from `famp_bus` is intentionally NOT exposed on
    /// the CLI's structured-result surface; debug-stringify keeps the
    /// CLI/MCP boundary independent of wire-layer types.
    pub delivered: String,
}

/// Production entry — resolves the broker socket via
/// `bus_client::resolve_sock_path` and prints a JSON-Line on success.
pub async fn run(args: SendArgs) -> Result<(), CliError> {
    let sock = crate::bus_client::resolve_sock_path();
    let outcome = run_at_structured(&sock, args).await?;
    let line = serde_json::json!({
        "task_id":   outcome.task_id,
        "delivered": outcome.delivered,
    });
    println!("{line}");
    Ok(())
}

/// Test-facing entry — accepts an explicit broker socket path so integration
/// tests can wire ephemeral sockets without polluting `$FAMP_BUS_SOCKET`.
/// Prints the same JSON-Line as [`run`].
pub async fn run_at(sock: &Path, args: SendArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(sock, args).await?;
    let line = serde_json::json!({
        "task_id":   outcome.task_id,
        "delivered": outcome.delivered,
    });
    println!("{line}");
    Ok(())
}

/// Structured entry — returns a [`SendOutcome`] without printing. Used by
/// the MCP `famp_send` tool wrapper so it can embed `task_id` and the
/// per-target delivery summary in the JSON-RPC result.
///
/// D-10 proxy semantics:
/// 1. Resolve identity via `cli::identity::resolve_identity`.
/// 2. Open `BusClient::connect(sock, Some(identity))` — the bus client
///    forwards `Hello { bind_as: Some(identity) }` and the broker
///    validates the canonical holder is live.
/// 3. Send `BusMessage::Send { to, envelope }` — NO per-message identity
///    field. The broker stamps `from` based on `effective_identity(state)`,
///    which resolves to the bound proxy name.
/// 4. On `HelloErr { NotRegistered }` or per-op `Err { NotRegistered }`,
///    surface `CliError::NotRegisteredHint { name }` with the canonical
///    operator hint.
pub async fn run_at_structured(sock: &Path, args: SendArgs) -> Result<SendOutcome, CliError> {
    // 1. Resolve identity (D-01) for the Hello.bind_as proxy.
    let identity = resolve_identity(args.act_as.as_deref())?;

    // 2. Belt-and-suspenders: clap's `conflicts_with` + `requires` already
    //    cover the flag matrix, but a defense-in-depth check protects
    //    callers that construct `SendArgs` programmatically (tests, MCP).
    if args.more_coming && args.new_task.is_none() {
        return Err(CliError::SendArgsInvalid {
            reason: "--more-coming is only valid with --new-task".to_string(),
        });
    }

    // 3. Build the target.
    let target = match (args.to.as_deref(), args.channel.as_deref()) {
        (Some(name), None) => Target::Agent {
            name: name.to_string(),
        },
        (None, Some(ch)) => Target::Channel {
            name: normalize_channel(ch)?,
        },
        (Some(_), Some(_)) => {
            return Err(CliError::SendArgsInvalid {
                reason: "--to and --channel are mutually exclusive".to_string(),
            });
        }
        (None, None) => {
            return Err(CliError::SendArgsInvalid {
                reason: "exactly one of --to or --channel is required".to_string(),
            });
        }
    };

    // 4. Build the envelope value. Phase 02 wires a minimal envelope shape
    //    (mode + optional body) so the broker has typed JSON to fan out.
    //    Phase 4 will graft the full signed-envelope construction back in
    //    once the federation gateway lands and the keyring loader is wired
    //    into the bus path. Until then the envelope carries enough metadata
    //    for the receiver to drive an `await` cursor without depending on
    //    keyring fields the local broker does not need.
    let envelope = build_envelope_value(&args)?;

    // 5. Connect. `Some(identity)` = D-10 proxy shape; broker validates
    //    at Hello time. Rich-error mapping: HelloErr{NotRegistered} =>
    //    NotRegisteredHint; everything else => BusClient or BrokerUnreachable.
    let mut bus = BusClient::connect(sock, Some(identity.clone()))
        .await
        .map_err(|e| match &e {
            BusClientError::HelloFailed {
                kind: BusErrorKind::NotRegistered,
                ..
            } => CliError::NotRegisteredHint {
                name: identity.clone(),
            },
            BusClientError::Io(_) | BusClientError::BrokerDidNotStart(_) => {
                CliError::BrokerUnreachable
            }
            // Frame, Decode, HelloFailed (other kinds), UnexpectedReply.
            _ => CliError::BusClient {
                detail: format!("{e:?}"),
            },
        })?;

    // 6. Send. NO act_as field; broker stamps `from` via D-10
    //    `effective_identity(state)`.
    let reply = bus
        .send_recv(BusMessage::Send {
            to: target,
            envelope,
        })
        .await
        .map_err(|e| CliError::BusClient {
            detail: format!("{e:?}"),
        })?;

    // 7. Best-effort shutdown so the broker observes Disconnect.
    bus.shutdown().await;

    match reply {
        BusReply::SendOk { task_id, delivered } => Ok(SendOutcome {
            task_id: task_id.to_string(),
            delivered: format!("{delivered:?}"),
        }),
        // Per-op liveness re-check failed (the holder died between Hello
        // and Send). Same operator hint as the Hello-time refusal.
        BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            ..
        } => Err(CliError::NotRegisteredHint { name: identity }),
        BusReply::Err { kind, message } => Err(CliError::BusError { kind, message }),
        // Any other reply variant indicates a broker-protocol violation
        // (e.g. SessionsOk in response to Send). Surface as a typed bus
        // client error for the operator to inspect.
        other => Err(CliError::BusClient {
            detail: format!("unexpected reply to Send: {other:?}"),
        }),
    }
}

/// Normalize a channel name: accept both `planning` and `#planning`; reject
/// `##planning`; validate against the bus channel regex
/// (`^#[a-z0-9][a-z0-9_-]{0,31}$`).
fn normalize_channel(input: &str) -> Result<String, CliError> {
    let normalized = if input.starts_with('#') {
        input.to_string()
    } else {
        format!("#{input}")
    };
    if normalized.starts_with("##") {
        return Err(CliError::SendArgsInvalid {
            reason: format!("channel name '{input}' cannot start with ##"),
        });
    }
    // The regex is compiled once on first use; failure to compile is a
    // programmer bug, not a runtime condition.
    let re = regex::Regex::new(CHANNEL_PATTERN).map_err(|e| CliError::SendArgsInvalid {
        reason: format!("internal: channel regex failed to compile: {e}"),
    })?;
    if !re.is_match(&normalized) {
        return Err(CliError::SendArgsInvalid {
            reason: format!("invalid channel name '{normalized}': must match {CHANNEL_PATTERN}"),
        });
    }
    Ok(normalized)
}

/// Build the JSON envelope value sent in `BusMessage::Send.envelope`.
///
/// Phase 02 ships a minimal mode-tagged shape rather than the full v0.8
/// signed envelope. The bus path does not yet route signed/keyring envelopes
/// (federation Phase 4 lights that up); for now the receiver only needs
/// enough structure to drive its inbox cursor and FSM. Fields:
///
/// - `mode`: `"new_task"` | `"deliver"` | `"deliver_terminal"` | `"channel_post"`
/// - `summary`: optional human-readable summary (from `--new-task`)
/// - `task`: optional `task_id` reference (for `--task` modes)
/// - `body`: optional freeform body text (from `--body`)
/// - `terminal`: bool (true on `--terminal`)
/// - `more_coming`: bool (only set when true, on `--new-task`)
fn build_envelope_value(args: &SendArgs) -> Result<serde_json::Value, CliError> {
    let mode = match (
        args.new_task.is_some(),
        args.task.is_some(),
        args.terminal,
        args.channel.is_some(),
    ) {
        (true, false, false, _) => "new_task",
        (false, true, true, _) => "deliver_terminal",
        (false, true, false, _) => "deliver",
        (false, false, false, true) => "channel_post",
        _ => {
            return Err(CliError::SendArgsInvalid {
                reason: "exactly one of --new-task / --task is required (or use --channel for a \
                         bare channel post)"
                    .to_string(),
            });
        }
    };
    let mut obj = serde_json::Map::new();
    obj.insert("mode".to_string(), serde_json::Value::String(mode.into()));
    if let Some(summary) = &args.new_task {
        obj.insert(
            "summary".to_string(),
            serde_json::Value::String(summary.clone()),
        );
    }
    if let Some(task) = &args.task {
        obj.insert("task".to_string(), serde_json::Value::String(task.clone()));
    }
    if let Some(body) = &args.body {
        obj.insert("body".to_string(), serde_json::Value::String(body.clone()));
    }
    if args.terminal {
        obj.insert("terminal".to_string(), serde_json::Value::Bool(true));
    }
    if args.more_coming {
        obj.insert("more_coming".to_string(), serde_json::Value::Bool(true));
    }
    Ok(serde_json::Value::Object(obj))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn normalize_channel_adds_hash_prefix() {
        assert_eq!(normalize_channel("planning").unwrap(), "#planning");
    }

    #[test]
    fn normalize_channel_accepts_existing_hash() {
        assert_eq!(normalize_channel("#planning").unwrap(), "#planning");
    }

    #[test]
    fn normalize_channel_rejects_double_hash() {
        let err = normalize_channel("##planning").unwrap_err();
        match err {
            CliError::SendArgsInvalid { reason } => assert!(
                reason.contains("cannot start with ##"),
                "unexpected reason: {reason}"
            ),
            other => panic!("expected SendArgsInvalid, got {other:?}"),
        }
    }

    #[test]
    fn normalize_channel_rejects_uppercase() {
        let err = normalize_channel("BadCaps").unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }

    #[test]
    fn normalize_channel_rejects_overlong() {
        // 33 chars → 33-char tail after `#`, exceeds the {0,31} bound.
        let long = format!("#a{}", "b".repeat(32));
        let err = normalize_channel(&long).unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }

    #[test]
    fn build_envelope_new_task_shape() {
        let args = SendArgs {
            to: Some("alice".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: Some("prose".to_string()),
            more_coming: true,
            act_as: None,
        };
        let v = build_envelope_value(&args).unwrap();
        assert_eq!(v["mode"], serde_json::Value::String("new_task".into()));
        assert_eq!(v["summary"], serde_json::Value::String("hi".into()));
        assert_eq!(v["body"], serde_json::Value::String("prose".into()));
        assert_eq!(v["more_coming"], serde_json::Value::Bool(true));
        // terminal not set → key omitted.
        assert!(v.get("terminal").is_none());
    }

    #[test]
    fn build_envelope_deliver_terminal_shape() {
        let args = SendArgs {
            to: Some("alice".to_string()),
            channel: None,
            new_task: None,
            task: Some("0193abcd-ef01-7000-8000-000000000000".to_string()),
            terminal: true,
            body: None,
            more_coming: false,
            act_as: None,
        };
        let v = build_envelope_value(&args).unwrap();
        assert_eq!(
            v["mode"],
            serde_json::Value::String("deliver_terminal".into())
        );
        assert_eq!(v["terminal"], serde_json::Value::Bool(true));
    }

    #[test]
    fn build_envelope_invalid_combo_errors() {
        let args = SendArgs {
            to: None,
            channel: None,
            new_task: None,
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
        };
        let err = build_envelope_value(&args).unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }

    #[test]
    fn more_coming_without_new_task_errors_in_run_at_structured() {
        // We don't need a live broker — `run_at_structured` validates flags
        // before opening a connection. resolve_identity will fall through
        // to tier-4 if no env/wires.tsv is set; we set --as to short-circuit.
        let args = SendArgs {
            to: Some("alice".to_string()),
            channel: None,
            new_task: None,
            task: Some("0193abcd-ef01-7000-8000-000000000000".to_string()),
            terminal: false,
            body: None,
            more_coming: true,
            act_as: Some("bob".to_string()),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let res = rt.block_on(run_at_structured(
            std::path::Path::new("/nonexistent-famp-sock"),
            args,
        ));
        match res.unwrap_err() {
            CliError::SendArgsInvalid { reason } => {
                assert!(reason.contains("--more-coming"), "{reason}");
            }
            other => panic!("expected SendArgsInvalid, got {other:?}"),
        }
    }
}
