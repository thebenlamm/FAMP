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
//! New-task send: `{"task_id":"<uuid>","delivered":"<debug>"}`
//! Reply send:    `{"task_id":"<envelope-uuid>","thread_task_id":"<replied-to-uuid>","delivered":"<debug>"}`
//!
//! `task_id` is the fresh envelope/message id returned by the broker for
//! THIS send. For replies (`--task`), `thread_task_id` is the originating
//! task id (== `causality.ref` in the wire envelope) and is omitted on
//! new-task sends. Mirrors the v0.8 send shape (additive — existing
//! `task_id` consumers are unaffected). The `delivered` field is a
//! debug-format of the broker's `Vec<Delivered>` reply because the
//! `Delivered` struct lives in `famp-bus` and exposing its full shape on
//! stdout would couple the CLI surface to a wire-layer crate;
//! debug-stringify keeps the surface ergonomic for shell pipes.
//!
// v0.8 federation HTTPS path was deleted in Phase 4; see
// `docs/MIGRATION-v0.8-to-v0.9.md`.

use std::path::Path;

use famp_bus::{BusErrorKind, BusMessage, BusReply, Target};
use famp_core::Principal;

use crate::bus_client::{BusClient, BusClientError};
use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::identity::resolve_identity;
use crate::cli::own_domain::resolve_own_domain;
use crate::cli::util::normalize_channel;

/// CLI arg set for `famp send`.
///
/// `--to` and `--channel` are mutually exclusive (exactly one required).
/// `--new-task` and `--task` are mutually exclusive (exactly one required).
/// `--terminal` requires `--task`.
/// `--more-coming` requires `--new-task` (clap-enforced + run-time guard).
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    /// Direct-message recipient identity (mutually exclusive with `--channel`).
    /// For a remote `agent:<domain>/<name>` target, a zero exit code means
    /// only local acceptance into the gateway-backed outbound mailbox on
    /// this host — see docs/GATEWAY-SETUP.md section 6 for what it does
    /// not confirm.
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
    /// Mark the deliver envelope terminal — the FINAL reply that closes the
    /// task (requires `--task`). Put your real, complete answer in this send;
    /// do not fire a placeholder terminal (e.g. `--body ack-minimal`) and then
    /// the real reply. If you are still testing the send path, omit `--terminal`
    /// so the task stays open until the real close is ready.
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
    /// Override this host's federation authority for a remote send
    /// (highest-precedence source in `own_domain::resolve_own_domain`:
    /// `--domain` > `FAMP_OWN_DOMAIN` env > `$FAMP_HOME/own-domain` file).
    /// Only consulted when `--to` parses as a full `agent:<domain>/<name>`
    /// principal (the remote-send branch, D-02); ignored for a bare-name
    /// local send (D-04).
    #[arg(long)]
    pub domain: Option<String>,
}

/// Outcome returned by [`run_at_structured`].
///
/// Carries the broker's envelope id (`task_id`), the optional replied-to
/// task id (`thread_task_id`, set only for reply sends), and a debug-format
/// of the per-target delivery slice. `delivered_rows` is the structured
/// equivalent used by the MCP tool. JSON-Line shape on stdout:
/// `{"task_id":"<uuid>","delivered":"<debug>"}` (new-task) or
/// `{"task_id":"<env-uuid>","thread_task_id":"<thread-uuid>","delivered":"<debug>"}` (reply).
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The fresh `UUIDv7` envelope/message id of THIS send, as returned by
    /// the broker in `SendOk`. For replies, the replied-to task is in
    /// `thread_task_id`; `task_id` here is the new reply envelope's own id.
    pub task_id: String,
    /// For a reply (`--task`), the replied-to task id (== `causality.ref`
    /// in the wire envelope). `None` for a new-task send (where it would
    /// be redundant with `task_id`).
    pub thread_task_id: Option<String>,
    /// Debug-format of the broker's `Vec<Delivered>` reply slice. The
    /// `Delivered` struct from `famp_bus` is intentionally NOT exposed on
    /// the CLI's structured-result surface; debug-stringify keeps the
    /// CLI/MCP boundary independent of wire-layer types.
    pub delivered: String,
    pub delivered_rows: Vec<DeliveredRow>,
}

/// Structured per-target delivery row. Used by the
/// `famp_send` MCP tool to surface `ok` and `woken`
/// programmatically without parsing the
/// `SendOutcome::delivered` debug string.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeliveredRow {
    pub to_kind: String,
    pub to_name: String,
    pub ok: bool,
    pub woken: bool,
}

/// Production entry — resolves the broker socket via
/// `bus_client::resolve_sock_path` and prints a JSON-Line on success.
pub async fn run(args: SendArgs) -> Result<(), CliError> {
    let sock = crate::bus_client::resolve_sock_path();
    let outcome = run_at_structured(&sock, args).await?;
    println!("{}", outcome_to_json_line(&outcome));
    Ok(())
}

/// Test-facing entry — accepts an explicit broker socket path so integration
/// tests can wire ephemeral sockets without polluting `$FAMP_BUS_SOCKET`.
/// Prints the same JSON-Line as [`run`].
pub async fn run_at(sock: &Path, args: SendArgs) -> Result<(), CliError> {
    let outcome = run_at_structured(sock, args).await?;
    println!("{}", outcome_to_json_line(&outcome));
    Ok(())
}

/// Serialize a [`SendOutcome`] to a JSON-Line string for stdout.
///
/// `thread_task_id` is included only when `Some` (reply sends). New-task
/// sends omit the key entirely, preserving byte-exact backward-compat with
/// consumers that parse only `task_id` and `delivered`.
fn outcome_to_json_line(outcome: &SendOutcome) -> String {
    let mut map = serde_json::Map::new();
    map.insert(
        "task_id".to_string(),
        serde_json::Value::String(outcome.task_id.clone()),
    );
    if let Some(tid) = &outcome.thread_task_id {
        map.insert(
            "thread_task_id".to_string(),
            serde_json::Value::String(tid.clone()),
        );
    }
    map.insert(
        "delivered".to_string(),
        serde_json::Value::String(outcome.delivered.clone()),
    );
    serde_json::Value::Object(map).to_string()
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
#[allow(clippy::too_many_lines)]
pub async fn run_at_structured(sock: &Path, args: SendArgs) -> Result<SendOutcome, CliError> {
    // Capture the replied-to task id BEFORE any field is moved out of `args`.
    // This is `Some` only for reply sends (`--task <uuid>`); `None` for new-task
    // sends. The broker's `SendOk.task_id` is the REPLY envelope's own id;
    // `thread_task_id` is the originating thread — surfaced as `causality.ref`
    // in the wire envelope and used by `famp_verify` to find the row.
    let thread_task_id: Option<String> = args.task.clone();

    // 1. Resolve identity (D-01) for the Hello.bind_as proxy.
    let identity = resolve_identity(args.act_as.as_deref())?;

    // Guard: reject #-prefixed identity strings (--as '#bad' edge case).
    // Identities must not start with '#' — that prefix is reserved for channels.
    if identity.starts_with('#') {
        return Err(CliError::SendArgsInvalid {
            reason: format!(
                "'{identity}' looks like a channel name; identities must not start with '#'"
            ),
        });
    }

    // 2. Belt-and-suspenders: clap's `conflicts_with` + `requires` already
    //    cover the flag matrix, but a defense-in-depth check protects
    //    callers that construct `SendArgs` programmatically (tests, MCP).
    if args.more_coming && args.new_task.is_none() {
        return Err(CliError::SendArgsInvalid {
            reason: "--more-coming is only valid with --new-task".to_string(),
        });
    }

    // 3. Parse `--to` as a remote `Principal` up front (D-01/D-02
    //    split-addressing). `Some(p)` marks the remote branch: the bus
    //    `Target` routes by the LEAF name only (Pitfall 2 / T-11-09),
    //    while the envelope `to`/`from` carry the full domain-qualified
    //    principal (provenance). A string that starts with `agent:` but
    //    fails to parse is a malformed remote target — reject typed here,
    //    never fall through to a silent local `agent:local.bus/agent:...`
    //    shape (review LOW). A bare name (no `agent:` prefix) fails with
    //    `MissingScheme` and takes the unchanged local path (D-04).
    let remote_principal: Option<Principal> = match args.to.as_deref() {
        Some(raw) => match raw.parse::<Principal>() {
            Ok(p) => Some(p),
            Err(e) => {
                if raw.starts_with("agent:") {
                    return Err(CliError::SendArgsInvalid {
                        reason: format!(
                            "'{raw}' looks like a remote principal but failed to parse: {e}"
                        ),
                    });
                }
                None
            }
        },
        None => None,
    };

    // 4. Build the target. Remote sends route the bus by the LEAF name —
    //    the gateway's proxy mailbox for a remote principal is bound
    //    under the bare leaf, never the full `agent:...` string.
    let target = match (args.to.as_deref(), args.channel.as_deref()) {
        (Some(name), None) => Target::Agent {
            name: remote_principal
                .as_ref()
                .map_or_else(|| name.to_string(), |p| p.name().to_string()),
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

    // Guard: reject #-prefixed names passed as agent targets (--to '#foo').
    // '#foo' is a channel name; callers must use --channel / channel= instead.
    // This fires for both CLI and MCP paths since both flow through run_at_structured.
    if let Target::Agent { name } = &target {
        if name.starts_with('#') {
            return Err(CliError::SendArgsInvalid {
                reason: format!(
                    "'{name}' looks like a channel name; pass channel= instead of peer="
                ),
            });
        }
    }

    // 5. Home is only needed for the remote-send branch (own-domain
    //    resolution, D-02). Local (bare-name) sends never touch it —
    //    requiring FAMP_HOME/HOME for a purely local send would be a
    //    regression (D-04).
    let home_path = if remote_principal.is_some() {
        home::resolve_famp_home()?
    } else {
        std::path::PathBuf::new()
    };

    // 6. Build the envelope value. The local (bare-name) branch wires a
    //    minimal mode-tagged payload wrapped in a typed `audit_log`
    //    BusEnvelope so the broker's Phase-1 D-09 typed-decoder
    //    (`AnyBusEnvelope::decode`) accepts the line on drain. The
    //    mode-tagged payload (mode + summary + task + body + flags)
    //    lives under `body.details`, preserving the v0.8 send surface
    //    verbatim for downstream readers. The audit_log class is chosen
    //    because it is fire-and-forget (no FSM-firing on receipt), its
    //    body schema is the most permissive (event + optional details),
    //    and BUS-11 forbids signatures on the bus path so an unsigned
    //    envelope is the correct shape. The remote (domain-qualified)
    //    branch is D-01/D-02/D-03 — see `build_remote_envelope_value`.
    let envelope = build_envelope_value(
        &args,
        &identity,
        &target,
        remote_principal.as_ref(),
        &home_path,
    )?;

    // 7. Connect. `Some(identity)` = D-10 proxy shape; broker validates
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

    // 8. Send. NO act_as field; broker stamps `from` via D-10
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

    // 9. Best-effort shutdown so the broker observes Disconnect.
    bus.shutdown().await;

    match reply {
        BusReply::SendOk { task_id, delivered } => {
            let delivered_rows = delivered
                .iter()
                .map(|d| {
                    let (to_kind, to_name) = match &d.to {
                        Target::Agent { name } => ("agent".to_string(), name.clone()),
                        Target::Channel { name } => ("channel".to_string(), name.clone()),
                    };
                    DeliveredRow {
                        to_kind,
                        to_name,
                        ok: d.ok,
                        woken: d.woken,
                    }
                })
                .collect();
            Ok(SendOutcome {
                task_id: task_id.to_string(),
                thread_task_id,
                delivered: format!("{delivered:?}"),
                delivered_rows,
            })
        }
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

/// RFC 3339 (second-precision, `Z`-suffixed) timestamp, shared by both the
/// local and remote envelope-build paths.
///
/// Shallow format match for `Timestamp::shallow_validate` (≥20 bytes,
/// `-`/`T`/`:` at fixed offsets, ends with `Z`): subsecond components (if
/// `time` emits one) are stripped so the trimmed `YYYY-MM-DDTHH:MM:SSZ`
/// form matches the fixture used by `audit_log_dispatch.rs`.
fn fresh_ts() -> Result<String, CliError> {
    let ts = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| CliError::SendArgsInvalid {
            reason: format!("failed to format envelope ts: {e}"),
        })?;
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
    Ok(ts)
}

/// Fresh `UUIDv7` message id (string form, local/audit_log envelope
/// shape), paired with [`fresh_ts`]. Used by the local (bare-name)
/// envelope path only — the remote (typed) path generates its own
/// `MessageId` directly via `MessageId::new_v7()`.
fn fresh_id_and_ts() -> Result<(String, String), CliError> {
    let id = uuid::Uuid::now_v7().to_string();
    let ts = fresh_ts()?;
    Ok((id, ts))
}

/// Build the wire envelope sent in `BusMessage::Send.envelope`.
///
/// Dispatches on `remote`: `Some(principal)` takes the domain-qualified
/// remote path (D-01/D-02/D-03, [`build_remote_envelope_value`]); `None`
/// keeps the local (bare-name) path byte-unchanged modulo id/ts (D-04).
///
/// The local path wraps the mode-tagged payload from
/// [`build_inner_payload`] in a typed unsigned `audit_log` `BusEnvelope`
/// shape so the broker's Phase-1 D-09 typed-decoder accepts each drained
/// line. BUS-11 forbids signatures on the bus path, so the envelope is
/// signature-less and `from`/`to` use a synthetic `agent:local.bus/<name>`
/// Principal scheme. Channel sends surface the channel name in `to` as
/// `agent:local.bus/<channel-without-#>` — pure cosmetic; the broker
/// routes by `BusMessage::Send.to: Target`, not by the envelope `to` field.
fn build_envelope_value(
    args: &SendArgs,
    identity: &str,
    target: &Target,
    remote: Option<&Principal>,
    home: &Path,
) -> Result<serde_json::Value, CliError> {
    if let Some(principal) = remote {
        return build_remote_envelope_value(args, identity, principal, home);
    }

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

    let (id, ts) = fresh_id_and_ts()?;

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

    let mut envelope = serde_json::json!({
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
    });

    if let (Some(task_uuid), Some(obj)) = (args.task.as_deref(), envelope.as_object_mut()) {
        // Reply envelopes (deliver / deliver_terminal) carry causality back
        // to the originating task so inbox readers can thread by task_id.
        // poll.rs already reads causality["ref"] for non-request classes;
        // the send path must populate it. JSON-literal form matches the
        // surrounding build style — no typed Causality dependency added.
        obj.insert(
            "causality".to_string(),
            serde_json::json!({
                "rel": "delivers",
                "ref": task_uuid,
            }),
        );
    }

    Ok(envelope)
}

/// Throwaway signing key used ONLY to produce the exact `WireEnvelope`-
/// shaped bytes `UnsignedEnvelope::sign` -> `SignedEnvelope::encode`
/// requires, then immediately stripped (BUS-11: the local bus never
/// carries a `signature`). Mirrors the sanctioned "sign-then-strip"
/// pattern already used by `famp-gateway/src/egress.rs`'s own
/// `plain_request_value` unit-test helper and
/// `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`'s
/// `unsigned_value`. There is no separate "encode unsigned" accessor.
const THROWAWAY_SIGN_SEED: [u8; 32] = [42u8; 32];

/// §9.3 `Bounds` shape carrying exactly 2 of the 8 optional keys (the
/// minimum the spec requires for enforceable bounds). Mirrors
/// `egress.rs`'s / the e2e test's own `two_key_bounds()` helper.
fn two_key_bounds() -> famp_envelope::body::Bounds {
    famp_envelope::body::Bounds {
        deadline: None,
        budget: Some(famp_envelope::body::Budget {
            amount: "0".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: Some(8),
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: None,
    }
}

/// Sign `env` with the throwaway key then strip the `signature` field —
/// the only path to wire bytes is `sign()` -> `encode()`; there is no
/// public "unsigned-to-value" accessor (BUS-11).
fn sign_then_strip<B: famp_envelope::BodySchema>(
    env: famp_envelope::UnsignedEnvelope<B>,
) -> Result<serde_json::Value, CliError> {
    let sk = famp_crypto::FampSigningKey::from_bytes(THROWAWAY_SIGN_SEED);
    let signed = env.sign(&sk).map_err(|e| CliError::Envelope(Box::new(e)))?;
    let bytes = signed
        .encode()
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| CliError::Envelope(Box::new(e)))?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("signature");
    }
    Ok(value)
}

/// Parse `args.task` (the `--task <uuid>` continuation id) into a typed
/// `MessageId`, surfacing a typed `SendArgsInvalid` on a malformed value
/// rather than panicking.
fn parse_task_id(task: &str) -> Result<famp_core::MessageId, CliError> {
    task.parse().map_err(|_| CliError::SendArgsInvalid {
        reason: format!("invalid --task id '{task}': expected a UUIDv7"),
    })
}

/// Build the domain-qualified remote-send envelope (D-01/D-02/D-03): `to`
/// is the full principal verbatim, `from` is
/// `agent:{own_domain}/{identity}` resolved via [`resolve_own_domain`]
/// (own-domain plan 02). The envelope CLASS is branched on send mode
/// (review HIGH #1) so the FSM can reach a terminal state
/// (`famp-fsm::engine::TaskFsm::step` only legalizes
/// `Requested -(Commit)-> Committed -(Deliver+terminal_status)->
/// terminal`; emitting `request` for every remote send would leave the
/// FSM stuck at REQUESTED forever):
///
/// - `--new-task` -> typed `RequestBody` (class `request`).
/// - `--task` (non-terminal) -> typed `CommitBody` (class `commit`) with
///   `Causality { rel: Commits, referenced: task_id }`.
/// - `--task --terminal` -> typed `DeliverBody` (class `deliver`) with
///   `Causality { rel: Delivers, referenced: task_id }` +
///   `terminal_status: Completed`.
///
/// Every shape is produced unsigned via [`sign_then_strip`] (BUS-11 — no
/// `signature` key ever reaches the bus).
fn build_remote_envelope_value(
    args: &SendArgs,
    identity: &str,
    principal: &Principal,
    home: &Path,
) -> Result<serde_json::Value, CliError> {
    let own_domain = resolve_own_domain(args.domain.as_deref(), home)?;
    let from: Principal = format!("agent:{own_domain}/{identity}")
        .parse()
        .map_err(|e| CliError::SendArgsInvalid {
            reason: format!("failed to build a valid `from` principal: {e}"),
        })?;
    let to = principal.clone();

    let ts = famp_envelope::Timestamp(fresh_ts()?);

    match (
        args.new_task.as_deref(),
        args.task.as_deref(),
        args.terminal,
    ) {
        (Some(summary), None, false) => {
            let body = famp_envelope::body::RequestBody {
                scope: serde_json::json!({}),
                bounds: two_key_bounds(),
                natural_language_summary: Some(summary.to_string()),
            };
            let env = famp_envelope::UnsignedEnvelope::<famp_envelope::body::RequestBody>::new(
                famp_core::MessageId::new_v7(),
                from,
                to,
                famp_core::AuthorityScope::Advisory,
                ts,
                body,
            );
            sign_then_strip(env)
        }
        (None, Some(task), false) => {
            let task_id = parse_task_id(task)?;
            let body = famp_envelope::body::CommitBody {
                scope: serde_json::json!({}),
                scope_subset: None,
                bounds: two_key_bounds(),
                accepted_policies: Vec::new(),
                delegation_permissions: None,
                reporting_obligations: None,
                terminal_condition: serde_json::json!({"type": "final_delivery"}),
                conditions: None,
                natural_language_summary: args.body.clone(),
            };
            let env = famp_envelope::UnsignedEnvelope::<famp_envelope::body::CommitBody>::new(
                famp_core::MessageId::new_v7(),
                from,
                to,
                famp_core::AuthorityScope::CommitLocal,
                ts,
                body,
            )
            .with_causality(famp_envelope::Causality {
                rel: famp_envelope::Relation::Commits,
                referenced: task_id,
            });
            sign_then_strip(env)
        }
        (None, Some(task), true) => {
            let task_id = parse_task_id(task)?;
            let body = famp_envelope::body::DeliverBody {
                interim: false,
                artifacts: None,
                result: args.body.as_deref().map(|b| serde_json::json!({"text": b})),
                usage_metrics: None,
                error_detail: None,
                provenance: Some(serde_json::json!({"signer": from.to_string()})),
                natural_language_summary: None,
            };
            let env = famp_envelope::UnsignedEnvelope::<famp_envelope::body::DeliverBody>::new(
                famp_core::MessageId::new_v7(),
                from,
                to,
                famp_core::AuthorityScope::Advisory,
                ts,
                body,
            )
            .with_causality(famp_envelope::Causality {
                rel: famp_envelope::Relation::Delivers,
                referenced: task_id,
            })
            .with_terminal_status(famp_core::TerminalStatus::Completed);
            sign_then_strip(env)
        }
        _ => Err(CliError::SendArgsInvalid {
            reason: "exactly one of --new-task / --task is required for a remote send".to_string(),
        }),
    }
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
            domain: None,
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
            domain: None,
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
            domain: None,
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
            domain: None,
        };
        let target = Target::Agent {
            name: "bob".to_string(),
        };
        let envelope = build_envelope_value(
            &args,
            "alice",
            &target,
            None,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
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
            domain: None,
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

    #[test]
    fn send_agent_with_hash_prefix_is_rejected() {
        // Guard fires before broker connection — no live broker needed.
        // Uses act_as to short-circuit identity resolution (same pattern as
        // `more_coming_without_new_task_errors_in_run_at_structured`).
        let args = SendArgs {
            to: Some("#bad-channel".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: Some("alice".to_string()),
            domain: None,
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
                assert!(reason.contains("channel"), "{reason}");
            }
            other => panic!("expected SendArgsInvalid, got {other:?}"),
        }
    }

    // --- causality regression tests ---

    #[test]
    fn build_envelope_value_emits_causality_on_reply() {
        // Regression: reply sends (--task <uuid>) must carry
        // causality{rel:"delivers",ref:<uuid>} so poll.rs can thread by task_id.
        let args = SendArgs {
            to: Some("alice".to_string()),
            channel: None,
            new_task: None,
            task: Some("0193abcd-ef01-7000-8000-000000000002".to_string()),
            terminal: false,
            body: Some("ok".to_string()),
            more_coming: false,
            act_as: None,
            domain: None,
        };
        let target = Target::Agent {
            name: "alice".to_string(),
        };
        let env = build_envelope_value(
            &args,
            "bob",
            &target,
            None,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(
            env["causality"]["rel"],
            serde_json::Value::String("delivers".into())
        );
        assert_eq!(
            env["causality"]["ref"],
            serde_json::Value::String("0193abcd-ef01-7000-8000-000000000002".into()),
        );
    }

    #[test]
    fn build_envelope_value_omits_causality_on_new_task() {
        // New-task sends have no originating task to thread back to.
        let args = SendArgs {
            to: Some("alice".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
            domain: None,
        };
        let target = Target::Agent {
            name: "alice".to_string(),
        };
        let env = build_envelope_value(
            &args,
            "bob",
            &target,
            None,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert!(
            env.get("causality").is_none(),
            "new_task envelopes must not carry causality"
        );
    }

    // --- remote-target split-addressing tests (D-01/D-02) ---

    /// `--to agent:hostb.test/bob` must qualify both `to` (full principal
    /// verbatim) and `from` (`agent:{own_domain}/{identity}`). `--domain`
    /// is the CLI flag — highest-precedence source in
    /// `own_domain::resolve_own_domain` — so this test injects the
    /// own-domain value through the args struct rather than touching
    /// process env (avoids the `FAMP_OWN_DOMAIN` serial-env race with
    /// `own_domain.rs`'s own test).
    #[test]
    fn build_envelope_value_remote_qualifies_from_and_to() {
        let args = SendArgs {
            to: Some("agent:hostb.test/bob".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
            domain: Some("hosta.test".to_string()),
        };
        let principal: Principal = "agent:hostb.test/bob".parse().unwrap();
        let target = Target::Agent {
            name: principal.name().to_string(),
        };
        let env = build_envelope_value(
            &args,
            "alice",
            &target,
            Some(&principal),
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(
            env["to"],
            serde_json::Value::String("agent:hostb.test/bob".into())
        );
        assert_eq!(
            env["from"],
            serde_json::Value::String("agent:hosta.test/alice".into())
        );
    }

    /// `run_at_structured`'s target-build seam must route the bus by the
    /// LEAF name (`bob`), never the full `agent:hostb.test/bob` string
    /// (Pitfall 2 / T-11-09) — verified by reproducing the same
    /// `remote_principal` -> `Target` logic the production seam uses.
    #[test]
    fn remote_target_splits_bus_leaf_from_envelope_principal() {
        let principal: Principal = "agent:hostb.test/bob".parse().unwrap();
        let target = Target::Agent {
            name: principal.name().to_string(),
        };
        assert_eq!(
            target,
            Target::Agent {
                name: "bob".to_string()
            }
        );
    }

    /// Regression: `--to bob` (bare name) is unaffected by the new
    /// `--domain` flag or the remote branch — identical `agent:local.bus`
    /// shape modulo id/ts.
    #[test]
    fn build_envelope_value_local_path_unchanged_with_domain_unset() {
        let args = SendArgs {
            to: Some("bob".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
            domain: None,
        };
        let target = Target::Agent {
            name: "bob".to_string(),
        };
        let env = build_envelope_value(
            &args,
            "alice",
            &target,
            None,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(
            env["to"],
            serde_json::Value::String("agent:local.bus/bob".into())
        );
        assert_eq!(
            env["from"],
            serde_json::Value::String("agent:local.bus/alice".into())
        );
    }

    /// A remote send with no own-domain source (no `--domain`, no
    /// `FAMP_OWN_DOMAIN`, no `$FAMP_HOME/own-domain` file) returns the
    /// typed `OwnDomainNotSet` error rather than a silent local fallback.
    /// Guarded by `temp_env::with_var_unset` (process-global mutex) so
    /// this cannot race `own_domain.rs`'s own `FAMP_OWN_DOMAIN`-touching
    /// test even under cargo's default multi-threaded test runner.
    #[test]
    fn remote_send_with_no_own_domain_source_returns_typed_error() {
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || {
            let tmp = tempfile::tempdir().unwrap();
            let args = SendArgs {
                to: Some("agent:hostb.test/bob".to_string()),
                channel: None,
                new_task: Some("hi".to_string()),
                task: None,
                terminal: false,
                body: None,
                more_coming: false,
                act_as: None,
                domain: None,
            };
            let principal: Principal = "agent:hostb.test/bob".parse().unwrap();
            let target = Target::Agent {
                name: principal.name().to_string(),
            };
            match build_envelope_value(&args, "alice", &target, Some(&principal), tmp.path()) {
                Err(CliError::OwnDomainNotSet) => {}
                other => panic!("expected OwnDomainNotSet, got {other:?}"),
            }
        });
    }

    /// `--to agent:garbage` (starts with `agent:` but does not parse as a
    /// full `agent:<authority>/<name>` principal) must be rejected typed
    /// by `run_at_structured` BEFORE any bus connection is attempted —
    /// never silently falls through to a local `agent:local.bus/agent:garbage`
    /// shape (review LOW). No live broker needed: the guard fires before
    /// `BusClient::connect`.
    #[test]
    fn malformed_agent_prefixed_target_is_rejected_typed_no_local_fallback() {
        let args = SendArgs {
            to: Some("agent:garbage".to_string()),
            channel: None,
            new_task: Some("hi".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: Some("alice".to_string()),
            domain: None,
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
                assert!(reason.contains("agent:garbage"), "{reason}");
            }
            other => panic!("expected SendArgsInvalid, got {other:?}"),
        }
    }

    // --- mode-branched typed class tests (D-03, review HIGH #1) ---

    fn remote_target() -> Principal {
        "agent:hostb.test/bob".parse().unwrap()
    }

    fn remote_args(new_task: Option<&str>, task: Option<&str>, terminal: bool) -> SendArgs {
        SendArgs {
            to: Some("agent:hostb.test/bob".to_string()),
            channel: None,
            new_task: new_task.map(str::to_string),
            task: task.map(str::to_string),
            terminal,
            body: Some("ok".to_string()),
            more_coming: false,
            act_as: None,
            domain: Some("hosta.test".to_string()),
        }
    }

    /// `--new-task` on a remote send must produce a typed `RequestBody`
    /// (class `request`), unsigned (no `signature` key on the bus Value),
    /// so the FSM starts at REQUESTED.
    #[test]
    fn remote_new_task_emits_typed_request_no_signature() {
        let args = remote_args(Some("ping"), None, false);
        let principal = remote_target();
        let env = build_remote_envelope_value(
            &args,
            "alice",
            &principal,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(env["class"], serde_json::Value::String("request".into()));
        assert_eq!(
            env["body"]["natural_language_summary"],
            serde_json::Value::String("ping".into())
        );
        assert!(env.get("signature").is_none(), "unsigned: {env}");
    }

    /// `--task` (non-terminal) on a remote send must produce a typed
    /// `CommitBody` (class `commit`) with `Causality{rel:Commits,ref:task}`
    /// — the transition that advances REQUESTED -> COMMITTED
    /// (`famp-fsm::engine.rs:29`), unsigned.
    #[test]
    fn remote_task_non_terminal_emits_typed_commit_with_commits_causality() {
        let task_id = "0193abcd-ef01-7000-8000-000000000003";
        let args = remote_args(None, Some(task_id), false);
        let principal = remote_target();
        let env = build_remote_envelope_value(
            &args,
            "alice",
            &principal,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(env["class"], serde_json::Value::String("commit".into()));
        assert_eq!(
            env["causality"]["rel"],
            serde_json::Value::String("commits".into())
        );
        assert_eq!(
            env["causality"]["ref"],
            serde_json::Value::String(task_id.into())
        );
        assert!(env.get("signature").is_none(), "unsigned: {env}");
    }

    /// `--task --terminal` on a remote send must produce a typed
    /// `DeliverBody` (class `deliver`) with `Causality{rel:Delivers,...}`
    /// AND `terminal_status: completed` — the transition that advances
    /// COMMITTED -> COMPLETED (terminal), unsigned.
    #[test]
    fn remote_task_terminal_emits_typed_deliver_with_terminal_status() {
        let task_id = "0193abcd-ef01-7000-8000-000000000004";
        let args = remote_args(None, Some(task_id), true);
        let principal = remote_target();
        let env = build_remote_envelope_value(
            &args,
            "alice",
            &principal,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(env["class"], serde_json::Value::String("deliver".into()));
        assert_eq!(
            env["causality"]["rel"],
            serde_json::Value::String("delivers".into())
        );
        assert_eq!(
            env["terminal_status"],
            serde_json::Value::String("completed".into())
        );
        assert!(env.get("signature").is_none(), "unsigned: {env}");
    }

    /// Regression: the bare-name local branch is byte-unchanged (modulo
    /// id/ts) by Task 2's remote-class branching — still the `audit_log`
    /// shape, `class` untouched by send mode.
    #[test]
    fn local_branch_still_audit_log_after_typed_class_branching() {
        let args = SendArgs {
            to: Some("bob".to_string()),
            channel: None,
            new_task: None,
            task: Some("0193abcd-ef01-7000-8000-000000000005".to_string()),
            terminal: true,
            body: None,
            more_coming: false,
            act_as: None,
            domain: None,
        };
        let target = Target::Agent {
            name: "bob".to_string(),
        };
        let env = build_envelope_value(
            &args,
            "alice",
            &target,
            None,
            Path::new("/nonexistent-famp-home"),
        )
        .unwrap();
        assert_eq!(env["class"], serde_json::Value::String("audit_log".into()));
    }
}
