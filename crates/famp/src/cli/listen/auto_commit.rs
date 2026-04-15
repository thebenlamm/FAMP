//! Auto-commit handler — Phase 4 Plan 04-01 Task 2.
//!
//! When the listen daemon receives a signed `request` envelope addressed to
//! its own principal, it automatically fires a signed `commit` reply back to
//! the sender. This closes the REQUESTED → COMMITTED leg of the FSM without
//! any human intervention, enabling `famp await` on the originator side to
//! advance the local task record.
//!
//! ## Design
//!
//! `spawn_reply` is called by the router handler AFTER the inbox append has
//! committed (the 200 durability receipt is load-bearing and must not be
//! delayed by the outbound POST). The outbound POST is fire-and-forget: if
//! it fails, we log and return — the protocol is eventually-consistent in
//! v0.8 (rate-limiting and retry defer to Federation Profile v0.9+).
//!
//! ## T-04-02 mitigation
//!
//! Auto-commit only replies to principals found in `peers.toml`. An unknown
//! `req.from()` principal is logged and dropped — we never POST to an
//! arbitrary endpoint coaxed from an inbound envelope.
//!
//! ## T-04-03 mitigation
//!
//! The commit reply is signed via `FampSigningKey::sign` and carries
//! `Causality { rel: Commits, referenced: req.id() }` — the reply is
//! cryptographically bound to the original request id.

use std::path::PathBuf;
use std::sync::Arc;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::FampSigningKey;
use famp_envelope::{
    body::{commit::CommitBody, Bounds},
    AnySignedEnvelope, Causality, Relation, SignedEnvelope, Timestamp, UnsignedEnvelope,
};

use crate::cli::config::read_peers;
use crate::cli::send::client::post_envelope;

/// Context shared by all auto-commit dispatches from a single daemon instance.
///
/// Constructed once in `run_on_listener` and passed as `Arc<AutoCommitCtx>`
/// to the router state. The reqwest client uses the same TOFU-verifier pattern
/// as `famp send` (TOFU pinned per peer in peers.toml).
pub struct AutoCommitCtx {
    /// Daemon's own signing key — used to sign commit replies.
    pub signing_key: FampSigningKey,
    /// Daemon's self-principal — used as `from` on outbound commit envelopes.
    pub self_principal: Principal,
    /// Path to `peers.toml` — re-read on each dispatch (simple, consistent
    /// with T-04-05 acceptance: peers.toml is 0600-owner-only).
    pub peers_toml_path: PathBuf,
}

/// Fire-and-forget: build and POST a `commit` reply to `req.from()`.
///
/// Spawns a detached tokio task so the router handler returns immediately
/// with the 200 durability receipt. Errors are logged via `eprintln!`.
///
/// Bail-out conditions (all log-and-return, never panic):
/// - `req` is not a `Request`-class envelope addressed to `self_principal`
/// - `req.from()` principal is not in `peers.toml` (T-04-02)
/// - Signing or HTTP POST fails
pub fn spawn_reply(ctx: Arc<AutoCommitCtx>, req: Arc<AnySignedEnvelope>) {
    tokio::spawn(async move {
        if let Err(e) = send_reply(&ctx, &req).await {
            eprintln!("famp listen: auto-commit reply failed: {e}");
        }
    });
}

async fn send_reply(
    ctx: &AutoCommitCtx,
    req: &AnySignedEnvelope,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Only reply to Request-class envelopes addressed to self.
    let (req_from, req_to, req_id) = match req {
        AnySignedEnvelope::Request(e) => (e.from_principal(), e.to_principal(), *e.id()),
        _ => return Ok(()), // not a request — nothing to commit
    };
    if req_to != &ctx.self_principal {
        return Ok(()); // not addressed to us
    }

    // T-04-02: look up req.from() in peers.toml.
    let peers = read_peers(&ctx.peers_toml_path)?;
    let Some(peer) = peers
        .peers
        .iter()
        .find(|p| {
            p.principal.as_deref().unwrap_or("agent:localhost/self")
                == req_from.to_string().as_str()
        })
        .cloned()
    else {
        eprintln!("famp listen: auto-commit: no peer entry for from={req_from}, dropping");
        return Ok(());
    };

    // Build a minimal legal CommitBody. bounds requires ≥2 keys (§9.3).
    let commit_id = MessageId::new_v7();
    let ts = now_timestamp();
    let causality = Causality {
        rel: Relation::Commits,
        referenced: req_id,
    };
    let body = CommitBody {
        scope: serde_json::Value::Object(serde_json::Map::new()),
        scope_subset: None,
        bounds: Bounds {
            hop_limit: Some(16),
            recursion_depth: Some(4),
            deadline: None,
            budget: None,
            policy_domain: None,
            authority_scope: None,
            max_artifact_size: None,
            confidence_floor: None,
        },
        accepted_policies: vec![],
        delegation_permissions: None,
        reporting_obligations: None,
        terminal_condition: serde_json::Value::Object(serde_json::Map::new()),
        conditions: None,
        natural_language_summary: Some("auto-commit reply".to_string()),
    };

    let unsigned: UnsignedEnvelope<CommitBody> = UnsignedEnvelope::new(
        commit_id,
        ctx.self_principal.clone(),
        req_from.clone(),
        AuthorityScope::Advisory,
        ts,
        body,
    )
    .with_causality(causality);

    let signed: SignedEnvelope<CommitBody> = unsigned.sign(&ctx.signing_key)?;
    let bytes = signed.encode()?;

    // POST via TOFU-pinning client (same path as `famp send`).
    let recipient_seg = req_from.to_string();
    post_envelope(
        &peer.endpoint,
        &recipient_seg,
        bytes,
        peer.tls_fingerprint_sha256.clone(),
        &peer.alias,
    )
    .await
    .map_err(|e| {
        Box::new(std::io::Error::other(format!("post_envelope: {e}")))
            as Box<dyn std::error::Error + Send + Sync>
    })?;

    Ok(())
}

fn now_timestamp() -> Timestamp {
    let now = time::OffsetDateTime::now_utc();
    let s = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    Timestamp(s)
}
