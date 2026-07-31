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
use famp_transport_http::{HttpTransport, HttpTransportError};
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

/// F-2/T-11-25: the seven federation-owned fields — the gateway alone may
/// write these onto a relayed envelope. A locally-originated drained
/// envelope carrying ANY of these is rejected in [`relay_one`] BEFORE
/// [`sign_federation_fields`] runs (never signed), rather than silently
/// overwritten — the review's stated preference, so a misbehaving local
/// client learns it did something illegal. `signature` itself is a
/// separate, narrower defense-in-depth check (`already_signed` below);
/// it is not included here because `AnyBusEnvelope::decode`
/// (`famp-envelope/src/bus.rs`, BUS-11) already hard-rejects any local-bus
/// line carrying `signature` before it can ever reach a drain.
const FEDERATION_OWNED_FIELDS: [&str; 7] = [
    "from_domain",
    "to_domain",
    "sender_key_id",
    "nonce",
    "expiry",
    "capability",
    "approval",
];

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

    // F-2/T-11-25: unconditional insert, not `entry().or_insert_with()` —
    // the gateway's own derived values are authoritative by construction.
    // By the time this runs, `relay_one` has already rejected any
    // locally-originated envelope that carried one of these keys (see
    // `FEDERATION_OWNED_FIELDS`), so in practice this never overwrites a
    // client-supplied value; it is unconditional anyway as defense in
    // depth for any other future caller of this function.
    {
        let obj = value.as_object_mut().ok_or(EgressError::NotAnObject)?;
        obj.insert(
            "from_domain".to_string(),
            Value::String(from.authority().to_string()),
        );
        obj.insert(
            "to_domain".to_string(),
            Value::String(to.authority().to_string()),
        );
        obj.insert(
            "sender_key_id".to_string(),
            Value::String(famp_crypto::key_id(vk)),
        );
        obj.insert(
            "nonce".to_string(),
            Value::String(uuid::Uuid::now_v7().to_string()),
        );
        obj.insert("expiry".to_string(), Value::String(expiry));
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
    Ok(crate::clock::strip_subseconds(&raw))
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
    Transport(#[from] HttpTransportError),
    /// T-11-19: the drained envelope's `from` authority does not match
    /// this gateway's configured own-domain. Never signed — a locally-
    /// registered agent must not make the gateway sign as a domain it
    /// does not own. Only produced when own-domain IS configured
    /// (`relay_one`'s `own_domain: Option<&str>` is `Some`); when unset,
    /// this variant is never constructed and `from` is signed verbatim.
    #[error("egress 'from' domain mismatch: expected own-domain '{expected}', got '{got}'")]
    FromDomainMismatch { expected: String, got: String },
    /// T-11-25: the drained envelope carried one or more federation-owned
    /// fields (`FEDERATION_OWNED_FIELDS`) — a local client speaking the
    /// UDS protocol pre-populated metadata only the gateway may write.
    /// Never signed; rejected before `sign_federation_fields` runs.
    #[error("drained envelope carries gateway-owned federation field(s): {fields}")]
    ClientSuppliedFederationField { fields: String },
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
///
/// `own_domain` is this gateway's resolved own-domain (plan 02's single
/// source), `None` when unset. T-11-19: when `Some(d)`, a `from` whose
/// authority != `d` is rejected here — BEFORE `sign_federation_fields` —
/// so the gateway never signs an envelope claiming a domain it does not
/// own. When `None`, `from` is signed verbatim (current/legacy behavior;
/// keeps `e2e_cross_host_delivery.rs`'s own-domain-unset spawn green as
/// the regression control).
async fn relay_one(
    envelope: &mut Value,
    own_domain: Option<&str>,
    sk: &FampSigningKey,
    vk: &TrustedVerifyingKey,
    transport: &HttpTransport,
) -> Result<(), RelayError> {
    let from = parse_principal_field(envelope, "from")?;
    let to = parse_principal_field(envelope, "to")?;

    if let Some(expected) = own_domain {
        let got = from.authority();
        if got != expected {
            return Err(RelayError::FromDomainMismatch {
                expected: expected.to_string(),
                got: got.to_string(),
            });
        }
    }

    // T-11-25: reject — never sign — a drained envelope that already
    // carries one or more federation-owned fields. This runs on every
    // drained envelope exactly once (the drain loop never re-queues or
    // retries a failed relay — `run_egress`'s `Await` permanently
    // advances the mailbox cursor), so this pre-check can never reject a
    // legitimate second gateway pass; there is no such pass in this
    // codebase.
    if let Some(obj) = envelope.as_object() {
        let present: Vec<&str> = FEDERATION_OWNED_FIELDS
            .iter()
            .copied()
            .filter(|field| obj.contains_key(*field))
            .collect();
        if !present.is_empty() {
            return Err(RelayError::ClientSuppliedFederationField {
                fields: present.join(", "),
            });
        }
    }

    sign_federation_fields(envelope, &from, &to, sk, vk)?;
    let bytes = serde_json::to_vec(envelope)?;

    transport
        .send(TransportMessage {
            sender: from,
            recipient: to,
            bytes,
        })
        .await
        .map_err(RelayError::Transport)
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
    own_domain: Option<String>,
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
                eprintln!(
                    "famp-gateway: egress[{name}]: unexpected Await reply: {}",
                    other.variant_name()
                );
                Vec::new()
            }
            Err(e) => {
                eprintln!("famp-gateway: egress[{name}]: Await failed: {e}");
                Vec::new()
            }
        };

        // Phase 14 D-10/D-17/BUS-11: `AwaitOk.envelopes` now carries
        // `StampedEnvelope` — the `origin` stamp is a LOCAL bus concept
        // (D-01/D-02) and must NEVER cross onto the federation wire.
        // `WireEnvelope` is `deny_unknown_fields`
        // (`famp-envelope/src/wire.rs`), so leaking an extra key here
        // would make every relayed envelope undecodable at the far end —
        // the exact BUS-11 failure class this repo already hit once.
        // Unwrap to the INNER envelope before signing/relaying; the
        // stamp itself is discarded, on purpose, right here.
        for mut envelope in envelopes.into_iter().map(|stamped| stamped.envelope) {
            if let Err(e) =
                relay_one(&mut envelope, own_domain.as_deref(), &sk, &vk, &transport).await
            {
                // {e:?} (Debug), not {e} (Display) -- the Debug chain walks
                // every #[source] level, including the TLS/rustls leaf
                // cause reqwest's own Display can omit (OBS-01).
                eprintln!("famp-gateway: egress[{name}]: failed to relay envelope: {e:?}");
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

        let decoded = verify_inbound_any(&bytes, &keyring, "2026-07-27T00:00:00Z")
            .expect("signed bytes must verify_inbound_any");
        assert_eq!(decoded.class(), MessageClass::Request);

        // Single-byte body tamper must reject.
        let mut tampered: Value = serde_json::from_slice(&bytes).unwrap();
        tampered["body"]["scope"]["task"] = Value::String("tampered".to_string());
        let tampered_bytes = serde_json::to_vec(&tampered).unwrap();
        let result = verify_inbound_any(&tampered_bytes, &keyring, "2026-07-27T00:00:00Z");
        assert!(
            matches!(result, Err(RejectReason::InvalidSignature)),
            "tampered body must fail verification, got {result:?}"
        );
    }

    /// Phase 14 plan 14-02, T-14-09/BUS-11: `AwaitOk.envelopes` now
    /// carries `StampedEnvelope { origin, envelope }`. `run_egress`
    /// unwraps to `stamped.envelope` (discarding `origin`) BEFORE this
    /// signing step runs — the stamp is a local-bus-only concept and
    /// must never cross onto the federation wire, because
    /// `WireEnvelope` is `deny_unknown_fields` and would make the
    /// relayed envelope permanently undecodable at the far end (the
    /// exact BUS-11 failure class this repo already hit once). This
    /// test proves the CORRECT call shape (already-unwrapped inner
    /// envelope in, signed envelope out) never gains an "origin" key.
    #[test]
    fn egress_relayed_envelope_carries_no_origin_key() {
        let sk = FampSigningKey::from_bytes([22u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:hosta.test/erin".parse().unwrap();
        let to: Principal = "agent:hostb.test/frank".parse().unwrap();

        // Simulate `run_egress`'s `.into_iter().map(|stamped| stamped.envelope)`
        // unwrap: the drained AwaitOk element carries a Gateway origin
        // stamp; only the inner envelope reaches signing.
        let stamped = famp_bus::StampedEnvelope {
            origin: famp_bus::Origin::Gateway,
            envelope: plain_request_value(&from, &to),
        };
        let mut envelope = stamped.envelope;

        sign_federation_fields(&mut envelope, &from, &to, &sk, &vk)
            .expect("signing the unwrapped inner envelope must succeed");

        assert!(
            envelope.get("origin").is_none(),
            "a relayed cross-host envelope must never carry an 'origin' key: {envelope:?}"
        );
        let keys: Vec<&str> = envelope
            .as_object()
            .expect("signed envelope must be an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert!(
            !keys.contains(&"origin"),
            "signed envelope key set must not contain 'origin': {keys:?}"
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

    /// OBS-01: `RelayError::Transport` must hold the typed
    /// `HttpTransportError` as a preserved `#[source]` (not a flattened
    /// `String`), and the gateway-visible Display it produces (the text
    /// `run_egress`'s drain-loop `eprintln!` prints) must name the
    /// underlying cause, not only the fixed "transport send failed"
    /// prefix.
    #[test]
    fn relay_error_transport_display_names_underlying_cause() {
        use std::error::Error as _;

        let bad_url_reason = "not a url"
            .parse::<url::Url>()
            .expect_err("must fail to parse as a URL");
        let underlying_text = bad_url_reason.to_string();

        let transport_err = HttpTransportError::InvalidUrl(bad_url_reason);
        let relay_err: RelayError = transport_err.into();

        let displayed = relay_err.to_string();
        assert!(
            displayed.contains(&underlying_text),
            "RelayError::Transport Display {displayed:?} must contain the \
             underlying transport reason {underlying_text:?}"
        );

        // Error::source() must remain walkable -- this is the load-bearing
        // property the plan's prohibition protects (no e.to_string()
        // flatten into a String field, which would sever the chain).
        assert!(
            relay_err.source().is_some(),
            "RelayError::Transport must preserve a walkable #[source]"
        );
    }

    // --- T-11-19: egress own-domain enforcement ---------------------

    /// When own-domain is configured (`Some`), an envelope whose `from`
    /// authority does NOT match it is rejected typed
    /// `RelayError::FromDomainMismatch` BEFORE `sign_federation_fields`
    /// runs — the drained value must come back byte-identical (no
    /// `signature`/federation fields ever inserted).
    #[tokio::test]
    async fn relay_one_rejects_foreign_from_domain_when_own_domain_configured() {
        let sk = FampSigningKey::from_bytes([40u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:otherdomain.test/alice".parse().unwrap();
        let to: Principal = "agent:hostb.test/bob".parse().unwrap();
        let mut value = plain_request_value(&from, &to);
        let before = value.clone();

        let transport = HttpTransport::new_client_only(None).unwrap();

        let result = relay_one(&mut value, Some("hosta.test"), &sk, &vk, &transport).await;

        match result {
            Err(RelayError::FromDomainMismatch { expected, got }) => {
                assert_eq!(expected, "hosta.test");
                assert_eq!(got, "otherdomain.test");
            }
            other => panic!("expected FromDomainMismatch, got {other:?}"),
        }
        assert_eq!(
            value, before,
            "a rejected foreign-domain envelope must NOT be mutated: no signature, \
             no federation fields — sign_federation_fields must never have run"
        );
    }

    /// When own-domain is unset (`None`), `relay_one` signs the drained
    /// `from` authority verbatim regardless of its value — the pre-plan
    /// behavior, kept so `e2e_cross_host_delivery.rs`'s own-domain-unset
    /// spawn (the regression control) stays green. The subsequent
    /// transport failure (no peer registered in this unit test) proves
    /// signing happened BEFORE the unrelated transport-stage error, not
    /// that signing was skipped.
    #[tokio::test]
    async fn relay_one_signs_from_verbatim_when_own_domain_unset() {
        let sk = FampSigningKey::from_bytes([41u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:anydomain.test/alice".parse().unwrap();
        let to: Principal = "agent:hostb.test/bob".parse().unwrap();
        let mut value = plain_request_value(&from, &to);

        let transport = HttpTransport::new_client_only(None).unwrap();

        let result = relay_one(&mut value, None, &sk, &vk, &transport).await;

        assert!(
            matches!(result, Err(RelayError::Transport(_))),
            "expected a transport-stage failure (no peer registered in this unit \
             test) -- proving the domain check did NOT reject it, got {result:?}"
        );
        assert!(
            value.get("signature").is_some(),
            "own-domain unset must sign the drained value verbatim before the \
             (unrelated) transport failure"
        );
        assert_eq!(
            value["from_domain"],
            Value::String("anydomain.test".to_string())
        );
    }

    // --- T-11-25: client-supplied federation-field rejection ----------

    /// Each of the 7 federation-owned fields, when already present on a
    /// drained (locally-originated) envelope, is rejected by `relay_one`
    /// with the typed `ClientSuppliedFederationField` error naming it —
    /// BEFORE `sign_federation_fields` runs, so no signature is ever
    /// produced and the envelope is left otherwise unmutated.
    #[tokio::test]
    async fn relay_one_rejects_each_client_supplied_federation_field() {
        let sk = FampSigningKey::from_bytes([50u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:hosta.test/alice".parse().unwrap();
        let to: Principal = "agent:hostb.test/bob".parse().unwrap();
        let transport = HttpTransport::new_client_only(None).unwrap();

        for field in FEDERATION_OWNED_FIELDS {
            let mut value = plain_request_value(&from, &to);
            value.as_object_mut().unwrap().insert(
                field.to_string(),
                Value::String("attacker-supplied".to_string()),
            );
            let before = value.clone();

            let result = relay_one(&mut value, None, &sk, &vk, &transport).await;

            match &result {
                Err(RelayError::ClientSuppliedFederationField { fields }) => {
                    assert!(
                        fields.contains(field),
                        "expected rejected-fields list to name '{field}', got '{fields}'"
                    );
                }
                other => {
                    panic!("field '{field}': expected ClientSuppliedFederationField, got {other:?}")
                }
            }
            assert_eq!(
                value, before,
                "field '{field}': a rejected client-supplied-federation-field envelope must \
                 not be mutated — no signature, no derived fields ever inserted"
            );
            assert!(
                value.get("signature").is_none(),
                "field '{field}': no signature must be produced on this reject path"
            );
        }
    }

    /// Multiple client-supplied federation fields at once are all named in
    /// the single typed rejection (not just the first one found).
    #[tokio::test]
    async fn relay_one_names_all_present_client_supplied_fields_at_once() {
        let sk = FampSigningKey::from_bytes([51u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:hosta.test/alice".parse().unwrap();
        let to: Principal = "agent:hostb.test/bob".parse().unwrap();
        let transport = HttpTransport::new_client_only(None).unwrap();

        let mut value = plain_request_value(&from, &to);
        value.as_object_mut().unwrap().insert(
            "nonce".to_string(),
            Value::String("attacker-nonce".to_string()),
        );
        value.as_object_mut().unwrap().insert(
            "capability".to_string(),
            Value::String("attacker-capability".to_string()),
        );

        let result = relay_one(&mut value, None, &sk, &vk, &transport).await;
        match result {
            Err(RelayError::ClientSuppliedFederationField { fields }) => {
                assert!(fields.contains("nonce"), "got: {fields}");
                assert!(fields.contains("capability"), "got: {fields}");
            }
            other => panic!("expected ClientSuppliedFederationField, got {other:?}"),
        }
    }
}
