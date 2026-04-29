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
use crate::cli::util::normalize_channel;

pub mod client;
pub mod fsm_glue;

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

    // 4. Build the envelope value. Phase 02 wires a minimal mode-tagged
    //    payload wrapped in a typed `audit_log` BusEnvelope so the broker's
    //    Phase-1 D-09 typed-decoder (`AnyBusEnvelope::decode`) accepts the
    //    line on drain. The mode-tagged payload (mode + summary + task +
    //    body + flags) lives under `body.details`, preserving the v0.8 send
    //    surface verbatim for downstream readers. The audit_log class is
    //    chosen because it is fire-and-forget (no FSM-firing on receipt),
    //    its body schema is the most permissive (event + optional details),
    //    and BUS-11 forbids signatures on the bus path so an unsigned
    //    envelope is the correct shape. Phase 4 will graft the full signed
    //    Request/Deliver envelope construction back in once the federation
    //    gateway lands and the keyring loader is wired into the bus path.
    let envelope = build_envelope_value(&args, &identity, &target)?;

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

/// Build the inner mode-tagged payload (the "what does this send mean?"
/// shape). Embedded under `body.details` of the outer `audit_log` envelope
/// so existing v0.8 consumers continue to read fields by name.
///
/// Fields:
/// - `mode`: `"new_task"` | `"deliver"` | `"deliver_terminal"` | `"channel_post"`
/// - `summary`: optional human-readable summary (from `--new-task`)
/// - `task`: optional `task_id` reference (for `--task` modes)
/// - `body`: optional freeform body text (from `--body`)
/// - `terminal`: bool (true on `--terminal`)
/// - `more_coming`: bool (only set when true, on `--new-task`)
fn build_inner_payload(args: &SendArgs) -> Result<serde_json::Value, CliError> {
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

/// Build the wire envelope sent in `BusMessage::Send.envelope`.
///
/// Wraps the mode-tagged payload from [`build_inner_payload`] in a typed
/// unsigned `audit_log` `BusEnvelope` shape so the broker's Phase-1 D-09
/// typed-decoder accepts each drained line. BUS-11 forbids signatures on
/// the bus path, so the envelope is signature-less and `from`/`to` use
/// a synthetic `agent:local.bus/<name>` Principal scheme. Channel sends
/// surface the channel name in `to` as `agent:local.bus/<channel-without-#>`
/// — pure cosmetic; the broker routes by `BusMessage::Send.to: Target`,
/// not by the envelope `to` field.
fn build_envelope_value(
    args: &SendArgs,
    identity: &str,
    target: &Target,
) -> Result<serde_json::Value, CliError> {
    let inner = build_inner_payload(args)?;

    // Synthesize Principal-shaped `from` / `to` strings. The local bus
    // does not enforce Principal authority/name validation beyond what
    // the typed-decoder requires (`from_str` parsing during deserialize).
    // Use a fixed `local.bus` authority so canonical bytes are stable
    // across runs for byte-exact round-trip in property tests.
    let from = format!("agent:local.bus/{identity}");
    let to = match target {
        Target::Agent { name } => format!("agent:local.bus/{name}"),
        Target::Channel { name } => {
            // Channel names start with `#`; strip for the Principal name
            // segment (which forbids `#`). `agent:local.bus/channel-X`.
            let stripped = name.trim_start_matches('#');
            format!("agent:local.bus/channel-{stripped}")
        }
    };

    // Synthesize a fresh UUIDv7 message id and the current timestamp.
    let id = uuid::Uuid::now_v7().to_string();
    // RFC 3339 UTC timestamp, second precision, trailing `Z`. Shallow
    // format match for `Timestamp::shallow_validate` (≥20 bytes,
    // `-`/`T`/`:` at fixed offsets, ends with `Z`).
    let ts = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| CliError::SendArgsInvalid {
            reason: format!("failed to format envelope ts: {e}"),
        })?;
    // Strip subsecond component if `time` emitted one — `Timestamp`'s
    // shallow validator accepts the trimmed `YYYY-MM-DDTHH:MM:SSZ` form
    // and the fixture used by `audit_log_dispatch.rs`. Find the first
    // dot or `Z` after the `T` and rebuild as `<HMS>Z`.
    let ts = if let Some(dot_idx) = ts.find('.') {
        // Find tail offset end (the `Z` or +/-HH:MM after subsecs).
        let tail_idx = ts[dot_idx..]
            .find(['Z', '+', '-'])
            .map_or(ts.len(), |i| dot_idx + i);
        let mut out = String::with_capacity(ts.len() - (tail_idx - dot_idx));
        out.push_str(&ts[..dot_idx]);
        out.push_str(&ts[tail_idx..]);
        out
    } else {
        ts
    };

    // The audit_log body's only required field is `event`; we encode the
    // mode-tagged payload under `details` for Phase-2 consumers that
    // continue to read by name (`details.mode`, `details.summary`, ...).
    let event = match (
        args.new_task.is_some(),
        args.task.is_some(),
        args.terminal,
        args.channel.is_some(),
    ) {
        (true, false, false, _) => "famp.send.new_task",
        (false, true, true, _) => "famp.send.deliver_terminal",
        (false, true, false, _) => "famp.send.deliver",
        (false, false, false, true) => "famp.send.channel_post",
        _ => "famp.send", // unreachable: build_inner_payload would have errored.
    };

    Ok(serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": id,
        "from": from,
        "to": to,
        "authority": "advisory",
        "ts": ts,
        "body": {
            "event": event,
            "details": inner,
        }
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn build_inner_payload_new_task_shape() {
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
        let v = build_inner_payload(&args).unwrap();
        assert_eq!(v["mode"], serde_json::Value::String("new_task".into()));
        assert_eq!(v["summary"], serde_json::Value::String("hi".into()));
        assert_eq!(v["body"], serde_json::Value::String("prose".into()));
        assert_eq!(v["more_coming"], serde_json::Value::Bool(true));
        // terminal not set → key omitted.
        assert!(v.get("terminal").is_none());
    }

    #[test]
    fn build_inner_payload_deliver_terminal_shape() {
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
        let v = build_inner_payload(&args).unwrap();
        assert_eq!(
            v["mode"],
            serde_json::Value::String("deliver_terminal".into())
        );
        assert_eq!(v["terminal"], serde_json::Value::Bool(true));
    }

    #[test]
    fn build_inner_payload_invalid_combo_errors() {
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
        let err = build_inner_payload(&args).unwrap_err();
        assert!(matches!(err, CliError::SendArgsInvalid { .. }));
    }

    /// The wrapped `build_envelope_value` MUST produce a typed
    /// `audit_log` envelope that round-trips through
    /// `AnyBusEnvelope::decode`. Locks the Phase-2 fix for the
    /// Phase-1 D-09 typed-decoder regression.
    #[test]
    fn build_envelope_value_decodes_as_audit_log() {
        let args = SendArgs {
            to: Some("bob".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
        };
        let target = Target::Agent {
            name: "bob".to_string(),
        };
        let envelope = build_envelope_value(&args, "alice", &target).unwrap();
        // Top-level keys required by `AnyBusEnvelope::decode`.
        assert_eq!(
            envelope["class"],
            serde_json::Value::String("audit_log".into())
        );
        assert_eq!(envelope["famp"], serde_json::Value::String("0.5.2".into()));
        // The mode-tagged inner payload lives under body.details.
        assert_eq!(
            envelope["body"]["details"]["mode"],
            serde_json::Value::String("new_task".into())
        );
        assert_eq!(
            envelope["body"]["details"]["summary"],
            serde_json::Value::String("hi".into())
        );
        // Round-trip through the broker's typed decoder.
        let bytes = famp_canonical::canonicalize(&envelope).unwrap();
        let _decoded = famp_envelope::AnyBusEnvelope::decode(&bytes)
            .expect("audit_log envelope MUST decode via AnyBusEnvelope");
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
