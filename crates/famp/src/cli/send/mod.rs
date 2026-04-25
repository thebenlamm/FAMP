//! `famp send` subcommand — Phase 3 Plan 03-02.
//!
//! Three modes:
//! - `--new-task "<text>" --to <alias>`: signs a `request` envelope, POSTs
//!   it, creates `<home>/tasks/<uuid>.toml` in `REQUESTED` state, prints the
//!   uuid to stdout.
//! - `--task <uuid> --to <alias>`: signs a non-terminal `deliver` envelope,
//!   POSTs it, updates `last_send_at` only.
//! - `--task <uuid> --terminal --to <alias>`: signs a terminal `deliver`
//!   envelope (`terminal_status = Completed`), POSTs it, advances the local
//!   FSM to COMPLETED, marks the record terminal. Subsequent sends on the
//!   same task return `CliError::TaskTerminal`.
//!
//! Send-and-persist ordering: the envelope is sent FIRST; task records are
//! only mutated after an HTTP 2xx. On network failure, TOFU mismatch, or
//! non-2xx, no local state changes.
//!
//! TLS: a custom rustls `ServerCertVerifier` captures the leaf SHA-256 on
//! first contact (TOFU) and pins it on subsequent contacts. See
//! `send::client`.

use std::path::Path;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_envelope::body::request::{
    REQUEST_SCOPE_INSTRUCTIONS_KEY, REQUEST_SCOPE_MORE_COMING_KEY,
};
use famp_envelope::body::{request::RequestBody, AckBody, Bounds, DeliverBody, TerminalStatus};
use famp_envelope::{Causality, Relation, SignedEnvelope, Timestamp, UnsignedEnvelope};
use famp_taskdir::{TaskDir, TaskRecord, TryUpdateError};

use crate::cli::config::{read_peers, write_peers_atomic, PeerEntry};
use crate::cli::error::CliError;
use crate::cli::init::load_identity;
use crate::cli::paths::IdentityLayout;
use crate::cli::{home, paths};

pub mod client;
pub mod fsm_glue;

/// CLI arg set for `famp send`.
#[derive(clap::Args, Debug)]
pub struct SendArgs {
    /// Peer alias (must exist in `peers.toml`).
    #[arg(long)]
    pub to: String,
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
    /// Signal "more briefing follows" on a `--new-task` envelope. The
    /// receiver should wait for follow-up `deliver`s before treating
    /// the task as ready to commit. Mirrors the `body.interim` flag on
    /// `deliver` envelopes. Default false → key omitted, byte-exact
    /// backwards-compat. Quick-260425-pc7 (T2.1).
    #[arg(long, requires = "new_task")]
    pub more_coming: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendMode {
    NewTask,
    DeliverNonTerminal,
    DeliverTerminal,
}

/// Outcome returned by [`run_at_structured`].
///
/// Carries the task id and final FSM state string without printing to stdout.
/// The CLI entry [`run_at`] calls this and prints `task_id` for new-task mode;
/// the MCP tool uses the structured form directly.
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// The `UUIDv7` task id (the opening request's `id` field).
    pub task_id: String,
    /// Human-readable FSM state after the send (`"requested"`, `"delivered"`,
    /// `"terminal"`).
    pub state: String,
}

/// Production entry — resolves `FAMP_HOME`.
pub async fn run(args: SendArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    run_at(&home, args).await
}

/// Test-facing entry — prints `task_id` to stdout on new-task mode (CLI parity).
pub async fn run_at(home: &Path, args: SendArgs) -> Result<(), CliError> {
    let new_task = args.new_task.is_some();
    let outcome = run_at_structured(home, args).await?;
    if new_task {
        println!("{}", outcome.task_id);
    }
    Ok(())
}

/// Structured entry — returns [`SendOutcome`] without printing. Used by the
/// MCP tool wrapper so it can embed `task_id` in the JSON-RPC result.
pub async fn run_at_structured(home: &Path, args: SendArgs) -> Result<SendOutcome, CliError> {
    let layout = load_identity(home)?;
    let signing_key = load_signing_key(&layout)?;
    let from_principal = load_self_principal(&layout)?;

    let peers_path = paths::peers_toml_path(home);
    let mut peers = read_peers(&peers_path)?;
    let peer = peers
        .find(&args.to)
        .cloned()
        .ok_or_else(|| CliError::PeerNotFound {
            alias: args.to.clone(),
        })?;

    let tasks = TaskDir::open(paths::tasks_dir(home))?;

    // Determine mode + pre-check terminal before any envelope work.
    let mode = match (&args.new_task, &args.task, args.terminal) {
        (Some(_), None, false) => SendMode::NewTask,
        (None, Some(_), false) => SendMode::DeliverNonTerminal,
        (None, Some(_), true) => SendMode::DeliverTerminal,
        _ => {
            return Err(CliError::SendArgsInvalid {
                reason: "exactly one of --new-task or --task must be provided".to_string(),
            });
        }
    };

    // Belt-and-suspenders guard for `--more-coming`: clap-derive's
    // `requires = "new_task"` does not fire when `--task` is also present
    // (clap evaluates `conflicts_with` before `requires`), so the flag
    // would otherwise reach the deliver path and be silently dropped.
    // Quick-260425-pc7 BL-01.
    if args.more_coming && !matches!(mode, SendMode::NewTask) {
        return Err(CliError::SendArgsInvalid {
            reason: "--more-coming is only valid with --new-task".to_string(),
        });
    }

    // Check if existing task is terminal (if it exists locally).
    // If the task doesn't exist, we'll create it on-demand when sending the reply.
    // This happens when replying to a request we *received* (not originated).
    if let Some(id) = &args.task {
        match tasks.read(id) {
            Ok(existing) => {
                if existing.terminal {
                    return Err(CliError::TaskTerminal {
                        task_id: id.clone(),
                    });
                }
            }
            Err(famp_taskdir::TaskDirError::NotFound { .. }) => {
                // Task doesn't exist locally — we're replying to a received request.
                // We'll create a record in persist_post_send.
            }
            Err(other) => return Err(CliError::TaskDir(other)),
        }
    }

    let to_principal = resolve_peer_principal(&peer)?;

    // Build the envelope bytes.
    let (envelope_bytes, task_id) = match mode {
        SendMode::NewTask => {
            let summary = args
                .new_task
                .clone()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "missing --new-task summary".to_string(),
                })?;
            build_request_envelope(
                &signing_key,
                &from_principal,
                &to_principal,
                &summary,
                args.body.as_deref(),
                args.more_coming,
            )?
        }
        SendMode::DeliverNonTerminal | SendMode::DeliverTerminal => {
            let task_id = args.task.clone().ok_or_else(|| CliError::SendArgsInvalid {
                reason: "missing --task id".to_string(),
            })?;
            let bytes = build_deliver_envelope(
                &signing_key,
                &from_principal,
                &to_principal,
                &task_id,
                mode == SendMode::DeliverTerminal,
                args.body.as_deref(),
            )?;
            (bytes, task_id)
        }
    };

    // POST the envelope.
    let recipient_url_seg = to_principal.to_string();
    let outcome = client::post_envelope(
        &peer.endpoint,
        &recipient_url_seg,
        envelope_bytes,
        peer.tls_fingerprint_sha256.clone(),
        &args.to,
    )
    .await?;

    // Persist task record + TOFU fingerprint on 2xx.
    persist_post_send(
        &tasks,
        &task_id,
        &args.to,
        mode,
        &mut peers,
        &peers_path,
        outcome.captured_fingerprint,
    )?;

    let state = match mode {
        SendMode::NewTask => "requested",
        SendMode::DeliverNonTerminal => "delivered",
        SendMode::DeliverTerminal => "terminal",
    };
    Ok(SendOutcome {
        task_id,
        state: state.to_string(),
    })
}

/// Load the daemon's 32-byte Ed25519 seed from disk and wrap it as a
/// `FampSigningKey`. Mirrors the Phase 2 listen loader (`listen::run_on_listener`).
fn load_signing_key(layout: &IdentityLayout) -> Result<famp_crypto::FampSigningKey, CliError> {
    let seed_bytes = std::fs::read(&layout.key_ed25519).map_err(|source| CliError::Io {
        path: layout.key_ed25519.clone(),
        source,
    })?;
    let seed: [u8; 32] = <[u8; 32]>::try_from(seed_bytes.as_slice()).map_err(|_| CliError::Io {
        path: layout.key_ed25519.clone(),
        source: std::io::Error::other("key.ed25519 is not 32 bytes"),
    })?;
    Ok(famp_crypto::FampSigningKey::from_bytes(seed))
}

/// Resolve the daemon's own `from` principal.
///
/// Reads the optional `principal` field from `config.toml`. The fallback
/// `agent:localhost/self` is used **only** when the field is genuinely absent
/// (or the config file does not exist). Any other failure — IO error, TOML
/// parse error, or a present-but-malformed principal string — is a hard
/// error: signing outbound traffic under the wrong identity would be a
/// silent security defect, not a recoverable best-effort condition.
fn load_self_principal(layout: &IdentityLayout) -> Result<Principal, CliError> {
    let fallback = "agent:localhost/self";

    // File missing → treat as "principal field absent" and use fallback.
    let bytes = match std::fs::read(&layout.config_toml) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return fallback
                .parse()
                .map_err(|err: famp_core::ParsePrincipalError| {
                    CliError::SendFailed(Box::new(std::io::Error::other(format!(
                        "fallback principal {fallback} failed to parse: {err}"
                    ))))
                });
        }
        Err(source) => {
            return Err(CliError::Io {
                path: layout.config_toml.clone(),
                source,
            });
        }
    };

    let text = std::str::from_utf8(&bytes).map_err(|err| CliError::Io {
        path: layout.config_toml.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, err),
    })?;

    // Empty file → no principal field configured; use fallback.
    if text.trim().is_empty() {
        return fallback
            .parse()
            .map_err(|err: famp_core::ParsePrincipalError| {
                CliError::SendFailed(Box::new(std::io::Error::other(format!(
                    "fallback principal {fallback} failed to parse: {err}"
                ))))
            });
    }

    // Malformed TOML is a hard failure — refuse to silently sign as the
    // fallback identity when the operator's config is corrupt.
    let cfg: crate::cli::config::Config =
        toml::from_str(text).map_err(|source| CliError::TomlParse {
            path: layout.config_toml.clone(),
            source,
        })?;

    cfg.principal.map_or_else(
        // Field genuinely absent → fallback. The fallback string is a
        // compile-time constant we know parses; the unwrap is a programmer
        // assertion, not a user-facing failure mode.
        || {
            fallback
                .parse()
                .map_err(
                    |err: famp_core::ParsePrincipalError| CliError::PrincipalInvalid {
                        path: layout.config_toml.clone(),
                        value: fallback.to_string(),
                        reason: format!("internal fallback principal failed to parse: {err}"),
                    },
                )
        },
        // Field present → MUST parse cleanly; no silent fallback on garbage.
        |s| {
            s.parse().map_err(
                |err: famp_core::ParsePrincipalError| CliError::PrincipalInvalid {
                    path: layout.config_toml.clone(),
                    value: s.clone(),
                    reason: err.to_string(),
                },
            )
        },
    )
}

/// Resolve the peer's on-wire principal. Prefers the explicit
/// `principal` field in `peers.toml` (set by `famp peer add --principal`);
/// otherwise falls back to `agent:localhost/self` so Phase 3 tests that
/// POST at the Phase 2 listen daemon's self-keyring continue to work
/// without a second flag.
fn resolve_peer_principal(peer: &PeerEntry) -> Result<Principal, CliError> {
    let s = peer
        .principal
        .clone()
        .unwrap_or_else(|| "agent:localhost/self".to_string());
    s.parse().map_err(|e: famp_core::ParsePrincipalError| {
        CliError::PrincipalInvalid {
            // Caller-side label: not a real on-disk path, but identifies the
            // source for the operator. peers.toml does not give us a
            // per-entry path, so use a synthetic marker.
            path: std::path::PathBuf::from(format!("peers.toml#{}", peer.alias)),
            value: s.clone(),
            reason: e.to_string(),
        }
    })
}

/// Build + sign a `request` envelope. `task_id` returned is the `UUIDv7`
/// string form of the envelope's `id` field, which becomes the local
/// famp-taskdir record key.
fn build_request_envelope(
    sk: &famp_crypto::FampSigningKey,
    from: &Principal,
    to: &Principal,
    summary: &str,
    body: Option<&str>,
    more_coming: bool,
) -> Result<(Vec<u8>, String), CliError> {
    let id = MessageId::new_v7();
    let ts = now_timestamp();
    // §9.3 requires ≥2 bounds keys. Hop limit + recursion depth is the
    // minimal legal pair that does not require timestamps or policy config.
    let bounds = Bounds {
        deadline: None,
        budget: None,
        hop_limit: Some(16),
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: Some(4),
    };
    // PROVISIONAL convention (ADR 0001): prose body lands under
    // scope.<REQUEST_SCOPE_INSTRUCTIONS_KEY>. Body-less sends preserve
    // the pre-existing `scope:{}` shape so nothing else regresses.
    //
    // Quick-260425-pc7: scope.more_coming is sender-omitted when false
    // to keep canonical bytes byte-exact with pre-flag envelopes.
    let mut scope_map = serde_json::Map::new();
    if let Some(text) = body {
        scope_map.insert(
            REQUEST_SCOPE_INSTRUCTIONS_KEY.to_string(),
            serde_json::Value::String(text.to_string()),
        );
    }
    if more_coming {
        scope_map.insert(
            REQUEST_SCOPE_MORE_COMING_KEY.to_string(),
            serde_json::Value::Bool(true),
        );
    }
    let scope = serde_json::Value::Object(scope_map);
    let request_body = RequestBody {
        scope,
        bounds,
        natural_language_summary: Some(summary.to_string()),
    };
    let unsigned: UnsignedEnvelope<RequestBody> = UnsignedEnvelope::new(
        id,
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts,
        request_body,
    );
    let signed: SignedEnvelope<RequestBody> = unsigned
        .sign(sk)
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    let bytes = signed
        .encode()
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    let task_id = uuid_str_from_message_id(&id);
    Ok((bytes, task_id))
}

/// Build + sign a `deliver` envelope referencing `task_id` via causality.
/// Phase 3 uses `Causality { rel: Delivers, ref: <task_message_id> }` to
/// link the deliver to the opening request.
fn build_deliver_envelope(
    sk: &famp_crypto::FampSigningKey,
    from: &Principal,
    to: &Principal,
    task_id: &str,
    terminal: bool,
    body_text: Option<&str>,
) -> Result<Vec<u8>, CliError> {
    let msg_id = MessageId::new_v7();
    let ts = now_timestamp();
    // `task_id` is a UUIDv7 string; re-parse it into a `MessageId` for the
    // causality reference. The local task record key IS the opening
    // request's `id`.
    let ref_id: MessageId =
        task_id
            .parse()
            .map_err(|e: uuid::Error| CliError::SendArgsInvalid {
                reason: format!("task id {task_id} is not a valid UUIDv7: {e}"),
            })?;
    let causality = Causality {
        rel: Relation::Delivers,
        referenced: ref_id,
    };

    // A terminal deliver requires `provenance` when status != Failed (§8a.3
    // `validate_against_terminal_status`). Phase 3 uses a minimal empty
    // object as a placeholder — Phase 4 wires real provenance.
    let body = DeliverBody {
        interim: !terminal,
        artifacts: None,
        result: None,
        usage_metrics: None,
        error_detail: None,
        provenance: if terminal {
            Some(serde_json::Value::Object(serde_json::Map::new()))
        } else {
            None
        },
        natural_language_summary: body_text.map(str::to_string),
    };

    let mut unsigned: UnsignedEnvelope<DeliverBody> = UnsignedEnvelope::new(
        msg_id,
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts,
        body,
    )
    .with_causality(causality);
    if terminal {
        unsigned = unsigned.with_terminal_status(TerminalStatus::Completed);
    }
    let signed: SignedEnvelope<DeliverBody> = unsigned
        .sign(sk)
        .map_err(|e| CliError::Envelope(Box::new(e)))?;
    signed.encode().map_err(|e| CliError::Envelope(Box::new(e)))
}

fn uuid_str_from_message_id(id: &MessageId) -> String {
    id.to_string()
}

fn now_timestamp() -> Timestamp {
    // RFC 3339 UTC with second precision.
    let now = time::OffsetDateTime::now_utc();
    // Use a compact RFC3339 format: YYYY-MM-DDThh:mm:ssZ.
    let s = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    Timestamp(s)
}

fn persist_post_send(
    tasks: &TaskDir,
    task_id: &str,
    alias: &str,
    mode: SendMode,
    peers: &mut crate::cli::config::Peers,
    peers_path: &Path,
    captured_fingerprint: Option<String>,
) -> Result<(), CliError> {
    let now_s = now_timestamp().0;
    match mode {
        SendMode::NewTask => {
            let rec =
                TaskRecord::new_requested(task_id.to_string(), alias.to_string(), now_s.clone());
            // last_send_at is also updated on the new task.
            let mut rec = rec;
            rec.last_send_at = Some(now_s);
            tasks.create(&rec)?;
        }
        SendMode::DeliverNonTerminal => {
            // Try to update existing record; if not found, create one for this
            // received task we're now replying to.
            match tasks.update(task_id, |mut r| {
                r.last_send_at = Some(now_s.clone());
                r
            }) {
                Ok(_) => {}
                Err(famp_taskdir::TaskDirError::NotFound { .. }) => {
                    // Create a record for this received task (we're the responder).
                    let mut rec = TaskRecord::new_committed(
                        task_id.to_string(),
                        alias.to_string(),
                        now_s.clone(),
                    );
                    rec.last_send_at = Some(now_s);
                    tasks.create(&rec)?;
                }
                Err(e) => return Err(CliError::TaskDir(e)),
            }
        }
        SendMode::DeliverTerminal => {
            // Try to update existing record; if not found, create one.
            //
            // Mirrors await_cmd/mod.rs's commit-receipt branch (post quick-260425-ho8):
            // try_update runs the FSM advance INSIDE the closure on a fresh-from-disk
            // record, persists only on Ok, and surfaces Err via eprintln. This closes
            // the B2-class "let _ = advance(...); let _ = update(...)" anti-pattern
            // that swallowed errors AND produced spurious writes on FSM Err.
            // (quick-260425-lny.)
            match tasks.try_update(task_id, |mut r| {
                r.last_send_at = Some(now_s.clone());
                fsm_glue::advance_terminal(&mut r).map(|_| r)
            }) {
                Ok(_) => {}
                Err(TryUpdateError::Store(famp_taskdir::TaskDirError::NotFound { .. })) => {
                    // Create and immediately mark terminal for this received task.
                    let mut rec = TaskRecord::new_committed(
                        task_id.to_string(),
                        alias.to_string(),
                        now_s.clone(),
                    );
                    rec.last_send_at = Some(now_s);
                    // COMMITTED → COMPLETED is a valid FSM transition.
                    fsm_glue::advance_terminal(&mut rec)?;
                    tasks.create(&rec)?;
                }
                Err(TryUpdateError::Closure(e)) => {
                    eprintln!(
                        "famp send: advance_terminal failed for task {task_id}: {e}"
                    );
                }
                Err(TryUpdateError::Store(e)) => {
                    eprintln!(
                        "famp send: failed to persist terminal-advance for task {task_id}: {e}"
                    );
                }
            }
        }
    }

    // TOFU capture: persist the leaf cert fingerprint on first contact.
    if let Some(fp) = captured_fingerprint {
        if let Some(entry) = peers.find_mut(alias) {
            entry.tls_fingerprint_sha256 = Some(fp);
            write_peers_atomic(peers_path, peers)?;
        }
    }
    Ok(())
}

// Silencer for unused imports after feature-gated paths.
#[allow(dead_code)]
fn _ack_body_silence(_: Option<AckBody>) {}
