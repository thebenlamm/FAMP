//! Outbound drain-then-relay: mailbox -> wire (Phase 9 D-01/D-03).
//!
//! Awaits/drains each locally-backed remote principal's mailbox via its
//! own [`crate::ProxiedPrincipal::send_recv`] connection, mutates the
//! drained `serde_json::Value` in place to add the outer federation
//! fields (`from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry`),
//! signs the mutated value with the gateway's own persisted key, and
//! POSTs the resulting bytes to the remote gateway's inbox via
//! `famp_transport_http::HttpTransport`. Content-transparent: `task_id`,
//! `class`, and `body` are never rewritten (09-RESEARCH.md §3/§5).
//!
//! # Shared-connection contract (load-bearing for GW-02)
//!
//! The same backed [`crate::ProxiedPrincipal`] connection this module
//! drains from is ALSO used by 09-03's ingress handler to deliver
//! inbound envelopes via the same principal's stand-in connection. Both
//! paths go through the single `Arc<Mutex<GatewayRegistry>>` the gateway
//! process owns. [`run_egress`] therefore never holds the registry lock
//! (or a long-lived `&mut ProxiedPrincipal`) across a long-blocking
//! `Await` — each loop iteration re-acquires the lock, issues a SHORT
//! (~1s) `Await`, and releases the lock again before signing/POSTing, so
//! ingress can interleave a `Send` on the same connection without being
//! starved for longer than one poll interval.

use std::sync::Arc;

use famp::{sign_value, CryptoError, FampSigningKey, Principal, TrustedVerifyingKey};
use famp_bus::{BusMessage, BusReply};
use famp_transport::{Transport, TransportMessage};
use famp_transport_http::HttpTransport;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{Duration as FederationDuration, OffsetDateTime};
use tokio::sync::Mutex;

use crate::registry::GatewayRegistry;

/// Per-iteration `Await` timeout (ms) for [`run_egress`]'s drain loop.
/// Deliberately SHORT — see the shared-connection contract above; a long
/// timeout here would starve 09-03's ingress `Send` on the same
/// connection for the same duration.
const AWAIT_TIMEOUT_MS: u64 = 1_000;

/// How far past the drained envelope's own signing moment the federation
/// `expiry` field is set. Format-only contract (Phase 8 D-04) — not an
/// enforced freshness window this phase (INGRESS-01 is v1.1).
const EXPIRY_WINDOW_MINUTES: i64 = 5;

/// Errors produced while federation-signing a drained mailbox `Value`
/// ([`sign_federation_fields`]).
#[derive(Debug, thiserror::Error)]
pub enum EgressError {
    /// The drained value was not a JSON object (mailbox entries are
    /// always objects — this indicates an upstream broker bug, not a
    /// hostile-input path, since drained envelopes already passed the
    /// broker's own `AnyBusEnvelope::decode` gate).
    #[error("drained envelope is not a JSON object")]
    NotAnObject,
    /// Formatting the RFC 3339 `expiry` timestamp failed.
    #[error("failed to format federation expiry timestamp: {0}")]
    ExpiryFormat(#[source] time::error::Format),
    /// `famp::sign_value` failed (canonicalization or signing error).
    #[error("failed to sign envelope: {0}")]
    Sign(#[from] CryptoError),
}

/// Federation-sign a drained, plain local-bus `Value` in place (D-01/D-03
/// egress sign site).
///
/// Given a plain drained request-class `Value`
/// (`famp`/`id`/`from`/`to`/`scope`/`class`/`authority`/`ts`/`body`, per
/// `famp-envelope/src/bus.rs`'s BUS-11 "no signature, ever" contract),
/// inserts `from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry`,
/// and a `signature` — leaving `id`, `from`, `to`, `class`, and `body`
/// byte-identical (content-transparency, D-03).
///
/// Mutates the raw `serde_json::Value` directly rather than reconstructing
/// a typed `UnsignedEnvelope<B>` — `BusEnvelope<B>` exposes only `.body()`
/// and `.class()` publicly, so a typed round-trip is not reachable from a
/// drained `Value` without a much larger new `famp-envelope` accessor
/// surface (09-RESEARCH.md Pitfall 3). `famp::sign_value` accepts `&Value`
/// directly and canonicalizes internally (RFC 8785 JCS) — no
/// pre-canonicalization needed here.
///
/// Idempotent-safe (D-03 "if not already cross-host-signed"): if `value`
/// already carries a `signature`, this function is a no-op — it never
/// double-signs or re-derives federation fields on an already-relayed
/// envelope.
pub fn sign_federation_fields(
    value: &mut Value,
    from: &Principal,
    to: &Principal,
    sk: &FampSigningKey,
    vk: &TrustedVerifyingKey,
) -> Result<(), EgressError> {
    let already_signed = value
        .as_object()
        .ok_or(EgressError::NotAnObject)?
        .contains_key("signature");
    if already_signed {
        return Ok(());
    }

    let expiry = federation_expiry()?;

    {
        let obj = value.as_object_mut().ok_or(EgressError::NotAnObject)?;
        obj.entry("from_domain".to_string())
            .or_insert_with(|| Value::String(from.authority().to_string()));
        obj.entry("to_domain".to_string())
            .or_insert_with(|| Value::String(to.authority().to_string()));
        obj.entry("sender_key_id".to_string())
            .or_insert_with(|| Value::String(famp_crypto::key_id(vk)));
        obj.entry("nonce".to_string())
            .or_insert_with(|| Value::String(uuid::Uuid::now_v7().to_string()));
        obj.entry("expiry".to_string())
            .or_insert_with(|| Value::String(expiry));
    }

    // Sign AFTER inserting the federation fields but BEFORE inserting
    // `signature` itself — this mirrors exactly what the receiver's
    // decode path verifies against (strip `signature`, canonicalize the
    // rest; famp-envelope/src/envelope.rs `decode_value`, Pitfall P3/P8).
    let signature = sign_value(sk, &*value)?;

    value
        .as_object_mut()
        .ok_or(EgressError::NotAnObject)?
        .insert(
            "signature".to_string(),
            Value::String(signature.to_b64url()),
        );

    Ok(())
}

/// RFC 3339 UTC timestamp `EXPIRY_WINDOW_MINUTES` from now, trimmed to
/// second precision exactly like `crates/famp/src/cli/send/mod.rs`'s `ts`
/// formatting, so `Timestamp::shallow_validate` /
/// `SignedEnvelope::federation_format_ok` accept it.
fn federation_expiry() -> Result<String, EgressError> {
    let raw = (OffsetDateTime::now_utc() + FederationDuration::minutes(EXPIRY_WINDOW_MINUTES))
        .format(&Rfc3339)
        .map_err(EgressError::ExpiryFormat)?;
    Ok(strip_subseconds(&raw))
}

/// Strip any subsecond component `time`'s `Rfc3339` formatter may emit,
/// rebuilding as the trimmed `YYYY-MM-DDTHH:MM:SSZ` form — identical
/// trimming logic to `crates/famp/src/cli/send/mod.rs`.
fn strip_subseconds(ts: &str) -> String {
    let Some(dot_idx) = ts.find('.') else {
        return ts.to_string();
    };
    let tail_idx = ts[dot_idx..]
        .find(['Z', '+', '-'])
        .map_or(ts.len(), |i| dot_idx + i);
    let mut out = String::with_capacity(ts.len() - (tail_idx - dot_idx));
    out.push_str(&ts[..dot_idx]);
    out.push_str(&ts[tail_idx..]);
    out
}

/// Errors relaying one drained envelope. Never propagated past
/// [`run_egress`]'s per-envelope warn-and-continue handling — a single
/// malformed/unroutable envelope must not kill the drain loop
/// (availability, not trust: 09-RESEARCH.md §5's threat register).
#[derive(Debug, thiserror::Error)]
enum RelayError {
    #[error("drained envelope missing '{0}' field")]
    MissingField(&'static str),
    #[error("drained envelope '{0}' field is not a valid Principal: {1}")]
    BadPrincipal(&'static str, String),
    #[error(transparent)]
    Sign(#[from] EgressError),
    #[error("failed to serialize signed envelope: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("transport send failed: {0}")]
    Transport(String),
}

fn parse_principal_field(envelope: &Value, field: &'static str) -> Result<Principal, RelayError> {
    let raw = envelope
        .get(field)
        .and_then(Value::as_str)
        .ok_or(RelayError::MissingField(field))?;
    raw.parse::<Principal>()
        .map_err(|e| RelayError::BadPrincipal(field, e.to_string()))
}

/// Sign one drained envelope and POST it to its recipient's peer gateway.
async fn relay_one(
    envelope: &mut Value,
    sk: &FampSigningKey,
    vk: &TrustedVerifyingKey,
    transport: &HttpTransport,
) -> Result<(), RelayError> {
    let from = parse_principal_field(envelope, "from")?;
    let to = parse_principal_field(envelope, "to")?;

    sign_federation_fields(envelope, &from, &to, sk, vk)?;
    let bytes = serde_json::to_vec(envelope)?;

    transport
        .send(TransportMessage {
            sender: from,
            recipient: to,
            bytes,
        })
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))
}

/// Drain-sign-POST loop for one locally-backed remote principal (D-01/
/// D-03). Spawnable per backed principal from `main.rs` (09-04).
///
/// See the module-level shared-connection contract doc for why this
/// takes the shared `Arc<Mutex<GatewayRegistry>>` rather than a
/// standalone `&mut ProxiedPrincipal`: each iteration acquires the lock,
/// issues one SHORT `Await`, and releases the lock BEFORE signing/
/// POSTing — so ingress (09-03) is never blocked longer than one poll
/// interval on the same principal's connection (GW-02 not starved).
///
/// Runs until the principal is no longer backed by `registry` (logs and
/// returns) — never panics on a transport or bus error, since a
/// down peer or a single malformed envelope must not take the whole
/// gateway process down.
pub async fn run_egress(
    name: String,
    registry: Arc<Mutex<GatewayRegistry>>,
    transport: Arc<HttpTransport>,
    sk: FampSigningKey,
    vk: TrustedVerifyingKey,
) {
    loop {
        // The registry lock is held only long enough to acquire the
        // principal and issue the short `Await` — dropped explicitly
        // right after, BEFORE the match/sign/POST below, so ingress
        // (09-03) is never blocked on this connection longer than one
        // poll interval (the shared-connection contract this module's
        // docs describe).
        let reply = {
            let mut guard = registry.lock().await;
            let Some(principal) = guard.get_mut(&name) else {
                eprintln!("famp-gateway: egress[{name}]: no longer backed, stopping drain loop");
                return;
            };
            let reply = principal
                .send_recv(BusMessage::Await {
                    timeout_ms: AWAIT_TIMEOUT_MS,
                    task: None,
                })
                .await;
            drop(guard);
            reply
        };

        let envelopes = match reply {
            Ok(BusReply::AwaitOk { envelopes, .. }) => envelopes,
            Ok(BusReply::AwaitTimeout {}) => Vec::new(),
            Ok(other) => {
                eprintln!("famp-gateway: egress[{name}]: unexpected Await reply: {other:?}");
                Vec::new()
            }
            Err(e) => {
                eprintln!("famp-gateway: egress[{name}]: Await failed: {e}");
                Vec::new()
            }
        };

        for mut envelope in envelopes {
            if let Err(e) = relay_one(&mut envelope, &sk, &vk, &transport).await {
                eprintln!("famp-gateway: egress[{name}]: failed to relay envelope: {e}");
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::error::RejectReason;
    use crate::verify::verify_inbound_any;
    use famp::{AuthorityScope, MessageClass, MessageId, Timestamp, UnsignedEnvelope};
    use famp_envelope::body::{Bounds, Budget, RequestBody};
    use famp_keyring::Keyring;

    fn two_key_bounds() -> Bounds {
        Bounds {
            deadline: Some("2026-07-27T00:00:00Z".to_string()),
            budget: Some(Budget {
                amount: "100".to_string(),
                unit: "usd".to_string(),
            }),
            hop_limit: None,
            policy_domain: None,
            authority_scope: None,
            max_artifact_size: None,
            confidence_floor: None,
            recursion_depth: None,
        }
    }

    /// A plain, unsigned, non-federation local-bus mailbox `Value`
    /// (matches the shape `Await`/`Inbox` actually returns, per
    /// `famp-envelope/src/bus.rs` BUS-11 — no signature, ever).
    fn plain_request_value(from: &Principal, to: &Principal) -> Value {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a70".parse().unwrap();
        let ts = Timestamp("2026-07-23T00:00:00Z".to_string());
        let body = RequestBody {
            scope: serde_json::json!({"task": "translate"}),
            bounds: two_key_bounds(),
            natural_language_summary: None,
        };
        let unsigned = UnsignedEnvelope::<RequestBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        );
        // Sign-then-strip is the simplest way to produce the exact plain
        // WireEnvelope-shaped JSON the bus stores, without a public
        // "unsigned-to-value" accessor (there isn't one — BUS-11).
        let dummy_sk = FampSigningKey::from_bytes([9u8; 32]);
        let bytes = unsigned.sign(&dummy_sk).unwrap().encode().unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value.as_object_mut().unwrap().remove("signature");
        value
    }

    #[test]
    fn sign_federation_fields_round_trips_and_preserves_content() {
        let sk = FampSigningKey::from_bytes([20u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:hosta.test/alice".parse().unwrap();
        let to: Principal = "agent:hostb.test/bob".parse().unwrap();

        let mut value = plain_request_value(&from, &to);
        let before = value.clone();

        sign_federation_fields(&mut value, &from, &to, &sk, &vk)
            .expect("signing a plain drained value must succeed");

        // Content-transparency (D-03): from/id/body byte-identical.
        assert_eq!(value["from"], before["from"]);
        assert_eq!(value["id"], before["id"]);
        assert_eq!(value["body"], before["body"]);
        assert_eq!(value["class"], before["class"]);

        // Federation fields present.
        assert_eq!(
            value["from_domain"],
            Value::String(from.authority().to_string())
        );
        assert_eq!(
            value["to_domain"],
            Value::String(to.authority().to_string())
        );
        assert!(value.get("signature").is_some());

        let bytes = serde_json::to_vec(&value).unwrap();
        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), vk).unwrap();

        let decoded =
            verify_inbound_any(&bytes, &keyring).expect("signed bytes must verify_inbound_any");
        assert_eq!(decoded.class(), MessageClass::Request);

        // Single-byte body tamper must reject.
        let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
        tampered["body"]["scope"]["task"] = Value::String("tampered".to_string());
        let tampered_bytes = serde_json::to_vec(&tampered).unwrap();
        let result = verify_inbound_any(&tampered_bytes, &keyring);
        assert!(
            matches!(result, Err(RejectReason::InvalidSignature)),
            "tampered body must fail verification, got {result:?}"
        );
    }

    #[test]
    fn sign_federation_fields_is_idempotent() {
        let sk = FampSigningKey::from_bytes([21u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:hosta.test/carol".parse().unwrap();
        let to: Principal = "agent:hostb.test/dave".parse().unwrap();

        let mut value = plain_request_value(&from, &to);
        sign_federation_fields(&mut value, &from, &to, &sk, &vk).unwrap();
        let signed_once = value.clone();

        // Second call on an already-signed value must be a no-op (D-03
        // "if not already cross-host-signed").
        sign_federation_fields(&mut value, &from, &to, &sk, &vk).unwrap();
        assert_eq!(
            value, signed_once,
            "re-signing an already-signed value must be a no-op"
        );
    }
}
