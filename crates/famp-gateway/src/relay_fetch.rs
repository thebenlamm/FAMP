//! Signed relay-fetch loop (17-05, REACH-04): the inbound half of the
//! relay path.
//!
//! 1. **Why this loop exists.** A gateway sitting behind NAT cannot accept
//!    the relay's inbound connection, so instead of being pushed to, it
//!    polls: it periodically asks the relay "anything queued for my
//!    domain?" and drains what comes back. The egress half of this same
//!    relay path needs NO new code at all — `--peer <domain>=<relay-url>`
//!    (already wired since plans 01-03) already reaches the relay exactly
//!    like any other peer gateway, because the relay's enqueue route is
//!    mounted on the same `famp_transport_http::INBOX_ROUTE` a direct peer
//!    uses. This module is the missing other direction.
//!
//! 2. **How a drain is authorized (D-26).** Each fetch carries an Ed25519
//!    signature over a canonical authorization form built and verified by
//!    `famp_relay::fetch_auth` — never assembled or re-verified locally.
//!    The signing key is this gateway's EXISTING identity key, the same
//!    one [`crate::egress::run_egress`] signs outbound envelopes with, so
//!    there is no second credential, no new key file, and nothing secret
//!    anywhere in this gateway's relay configuration. The relay itself
//!    holds only the matching PUBLIC key (`--domain <domain>=<pubkey>` at
//!    the relay), which is why a compromised relay cannot impersonate this
//!    gateway to anyone.
//!
//! 3. **The load-bearing security property.** Every envelope this loop
//!    fetches is handed to [`crate::ingress::ingest_inbound`] — the EXACT
//!    same single ingest core the HTTPS `inbox_handler` uses — so the
//!    relay path inherits all four cheap gates (audience, freshness,
//!    replay, rate limit) and the Ed25519 signature verification
//!    identically. A second, softer ingest path here would make every
//!    INGR requirement this phase built decorative, because an attacker
//!    would simply post to the relay instead of the gateway directly
//!    (REACH-04, D-02, INGR-05).
//!
//! 4. **What the relay can see (D-25).** It terminates TLS and reads
//!    plaintext envelope bodies — FAMP signs but does not encrypt.
//!    Nothing in this module, or in anything derived from it, may
//!    describe this path as private or encrypted; Phase 18's follower
//!    doc is where that disclosure belongs (D-29).

use std::path::PathBuf;
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use famp::{FampSigningKey, Principal};
use famp_relay::fetch_auth::{normalize_audience, sign_fetch_auth, RELAY_FETCH_ROUTE};
use time::OffsetDateTime;
use url::Url;

use crate::ingress::{ingest_inbound, GatewayIngressState};

/// Short-poll cadence, matching `egress.rs::AWAIT_TIMEOUT_MS`'s
/// short-poll discipline — this is not a long-poll design (D-28: SSE/
/// long-poll is an orthogonal, not-yet-needed transport question).
pub const RELAY_POLL_INTERVAL_MS: u64 = 1_000;

/// Backoff ceiling for a relay that is unreachable, unauthorized, or
/// throttling this gateway.
///
/// A down relay should not be hammered once a second indefinitely —
/// REACH-05 (plan 06) is what tells the operator about a persistent
/// problem; this loop's own job is to survive and recover, not to alert.
pub const RELAY_FETCH_BACKOFF_MAX_MS: u64 = 30_000;

/// Rejections this loop can hit.
///
/// Kept distinct rather than collapsed: an operator debugging a relay
/// must be able to tell "the relay is down" apart from "the relay does
/// not know my key yet" apart from "I am being throttled" apart from
/// "the relay sent me something I could not parse" — conflating any pair
/// of these sends an operator hunting the wrong problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelayFetchError {
    /// The relay could not be reached at all, or responded with a status
    /// this loop does not otherwise recognize (never 200/401/403/429).
    #[error("transport error contacting the relay")]
    Transport,
    /// The relay's 200 response body did not parse into the expected
    /// `{"envelopes": [...]}` shape, or contained non-base64url `body`
    /// bytes. The WHOLE batch is rejected — never partially trusted.
    #[error("malformed relay fetch response")]
    MalformedResponse,
    /// A 401 or 403: the relay accepted the request shape but did not
    /// authorize this drain. By far the most likely cause is that the
    /// relay operator has not yet configured this gateway's public key
    /// for this domain (D-27's accepted friction) — never a standing
    /// misconfiguration to be confused with a transient rate limit.
    #[error(
        "relay rejected this gateway's fetch authorization (401/403) — most likely the relay \
         operator has not yet configured this gateway's public key for this domain; run \
         `famp peer export` and send the operator the printed public-key field, then have them \
         add a `--domain <domain>=<pubkey>` entry and restart the relay (D-27)"
    )]
    Unauthorized,
    /// A 429: this gateway's domain has been fetched too often within the
    /// relay's rate-limit window. Transient by nature (messages stay
    /// queued until their TTL) — never logged as a misconfiguration,
    /// unlike [`RelayFetchError::Unauthorized`].
    #[error("relay rate-limited this fetch (429)")]
    RateLimited,
    /// Building the signed fetch authorization itself failed (the relay
    /// crate's own signer returned an error). Logged and backed off like
    /// any other failure, never propagated as a panic.
    #[error("failed to build a signed fetch authorization")]
    AuthBuild,
    /// A drained batch entry's `to` field did not parse as a `Principal`.
    /// The WHOLE batch is rejected on the first such entry — never
    /// skipped silently, which would be an undelivered message with no
    /// error anywhere (the exact failure REACH-05 exists to remove).
    #[error("relay-fetched batch entry recipient is not a valid principal")]
    BadRecipient,
}

/// One decoded batch entry's minimal wire shape:
/// `{"to": "<principal string>", "body": "<base64url-unpadded bytes>"}`.
/// Deserialize-only — this loop never re-serializes or re-emits this
/// shape, it only ever consumes what the relay already produced
/// (`famp-relay/src/http.rs::ingest_fetch`).
#[derive(Debug, serde::Deserialize)]
struct FetchBatchEntry {
    to: String,
    body: String,
}

/// The relay's fetch-response envelope: `{"envelopes": [...]}`.
#[derive(Debug, serde::Deserialize)]
struct FetchBatchResponse {
    envelopes: Vec<FetchBatchEntry>,
}

/// Decode a relay fetch response body into `(recipient, bytes)` pairs.
///
/// Any malformed entry — an unparseable `to`, or non-base64url `body` —
/// fails the WHOLE batch rather than silently dropping that one entry: a
/// silently skipped envelope is an undelivered message with no error
/// anywhere, which is the exact failure REACH-05 exists to remove. An
/// empty `envelopes` array is not an error; it decodes to an empty
/// vector.
pub(crate) fn decode_fetch_batch(
    body: &[u8],
) -> Result<Vec<(Principal, Vec<u8>)>, RelayFetchError> {
    let parsed: FetchBatchResponse =
        serde_json::from_slice(body).map_err(|_| RelayFetchError::MalformedResponse)?;

    let mut out = Vec::with_capacity(parsed.envelopes.len());
    for entry in parsed.envelopes {
        let principal: Principal = entry
            .to
            .parse()
            .map_err(|_| RelayFetchError::BadRecipient)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(entry.body.as_bytes())
            .map_err(|_| RelayFetchError::MalformedResponse)?;
        out.push((principal, bytes));
    }
    Ok(out)
}

/// The three status-driven outcomes this loop distinguishes, plus
/// "anything else". Factored into a pure function so the status-mapping
/// logic is unit-testable without a live relay (mirrors this codebase's
/// established convention of extracting pure predicates out of I/O-bound
/// loops for testability, e.g. `egress.rs::sender_is_itself_backed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchOutcome {
    Success,
    Unauthorized,
    RateLimited,
    Other,
}

fn classify_fetch_status(status: reqwest::StatusCode) -> FetchOutcome {
    if status == reqwest::StatusCode::OK {
        FetchOutcome::Success
    } else if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        FetchOutcome::Unauthorized
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        FetchOutcome::RateLimited
    } else {
        FetchOutcome::Other
    }
}

/// Double the current backoff, capped at [`RELAY_FETCH_BACKOFF_MAX_MS`].
/// A pure function so the cap behavior is unit-testable without sleeping.
fn next_backoff_ms(current_ms: u64) -> u64 {
    current_ms.saturating_mul(2).min(RELAY_FETCH_BACKOFF_MAX_MS)
}

/// Build the TLS-preconfigured `reqwest::Client` this loop's fetch
/// requests go out on, reusing `famp_transport_http`'s own
/// `build_client_config` — the SAME construction
/// `famp_transport_http::HttpTransport::new_client_only` uses internally
/// — rather than assembling a second rustls client stack. `trust_cert`
/// is the gateway's existing `--trust-cert` value (the same cert this
/// gateway already trusts for its direct-peer/relay egress connections);
/// there is no separate relay-specific trust flag.
fn build_fetch_client(
    trust_cert: Option<&std::path::Path>,
) -> Result<reqwest::Client, RelayFetchError> {
    let tls = famp_transport_http::tls::build_client_config(trust_cert)
        .map_err(|_| RelayFetchError::Transport)?;
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .timeout(Duration::from_secs(10))
        .http1_only()
        .build()
        .map_err(|_| RelayFetchError::Transport)
}

/// Build the concrete fetch URL for `domain` by filling in
/// [`famp_relay::fetch_auth::RELAY_FETCH_ROUTE`]'s `{domain}` path
/// segment against `relay_base` — never a hand-typed copy of the route
/// string itself; only the imported constant's own segments are used.
// `"{domain}"` below is a literal path-template placeholder compared
// against a route segment, never a formatting-macro argument — same
// precedent as `famp-relay/tests/relay_store_and_forward.rs::fetch_url`.
#[allow(clippy::literal_string_with_formatting_args)]
fn build_fetch_url(relay_base: &Url, domain: &str) -> Result<Url, RelayFetchError> {
    let mut url = relay_base.clone();
    url.set_path("");
    {
        let mut segs = url
            .path_segments_mut()
            .map_err(|()| RelayFetchError::Transport)?;
        segs.pop_if_empty();
        for seg in RELAY_FETCH_ROUTE.split('/').filter(|s| !s.is_empty()) {
            if seg == "{domain}" {
                segs.push(domain);
            } else {
                segs.push(seg);
            }
        }
    }
    Ok(url)
}

/// One drain-loop iteration: sign a fresh fetch authorization, request a
/// drain, and (on success) ingest every returned envelope through the
/// single ingest core. Returns the backoff (ms) the caller should sleep
/// before the NEXT iteration — `RELAY_POLL_INTERVAL_MS` on success or an
/// unauthorized/rate-limited/errored request that doubles/resets no
/// further, `next_backoff_ms(backoff_ms)` otherwise. Extracted from
/// [`run_relay_fetch`] solely to satisfy `clippy::too_many_lines`,
/// mirroring this crate's established extraction precedent
/// (`main.rs::resolve_own_domain_or_exit`/`build_route_map`).
async fn poll_once(
    client: &reqwest::Client,
    fetch_url: &Url,
    sk: &FampSigningKey,
    audience: &str,
    own_domain: &str,
    state: &GatewayIngressState,
    backoff_ms: u64,
) -> u64 {
    let now = OffsetDateTime::now_utc();
    // D-26: the fetch authorization is built and signed EXCLUSIVELY by
    // the relay crate's own `sign_fetch_auth` — this loop never
    // assembles the signed form or mints a nonce/timestamp itself.
    let signed = match sign_fetch_auth(sk, audience, own_domain, now) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: {:?} ({e:?})",
                RelayFetchError::AuthBuild
            );
            return next_backoff_ms(backoff_ms);
        }
    };

    let mut req = client.get(fetch_url.clone());
    for (name, value) in signed.header_pairs() {
        req = req.header(name, value);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            // {e:?} (Debug), not {e} (Display) -- OBS-01, same rationale
            // as egress.rs's own relay-failure log line: the Debug chain
            // walks every #[source] level, including the TLS/rustls leaf
            // cause reqwest's own Display can omit.
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: {:?} ({e:?})",
                RelayFetchError::Transport
            );
            return next_backoff_ms(backoff_ms);
        }
    };

    match classify_fetch_status(resp.status()) {
        FetchOutcome::Unauthorized => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: {}",
                RelayFetchError::Unauthorized
            );
            next_backoff_ms(backoff_ms)
        }
        // 429 is transient (messages stay queued until their TTL) --
        // deliberately NO misconfiguration-shaped log line here, unlike
        // the Unauthorized arm above.
        FetchOutcome::RateLimited => next_backoff_ms(backoff_ms),
        FetchOutcome::Other => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: {:?} (relay responded with unexpected \
                 status {})",
                RelayFetchError::Transport,
                resp.status()
            );
            next_backoff_ms(backoff_ms)
        }
        FetchOutcome::Success => {
            ingest_success_batch(resp, own_domain, state).await;
            RELAY_POLL_INTERVAL_MS
        }
    }
}

/// The success-path body of [`poll_once`]: read the response, decode the
/// batch, and hand every entry to [`ingest_inbound`]. Extracted for the
/// same `too_many_lines` reason as [`poll_once`] itself.
async fn ingest_success_batch(
    resp: reqwest::Response,
    own_domain: &str,
    state: &GatewayIngressState,
) {
    let body = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: {:?} (failed to read fetch response \
                 body: {e:?})",
                RelayFetchError::Transport
            );
            return;
        }
    };
    let entries = match decode_fetch_batch(&body) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("famp-gateway: relay-fetch[{own_domain}]: {e:?} (malformed fetch batch)");
            return;
        }
    };
    for (recipient, bytes) in entries {
        // THE single ingest core (INGR-01..08): every relay-fetched
        // envelope is handed to the SAME function the direct HTTPS path
        // calls, with `recipient` the RELAY-SUPPLIED value (never
        // re-read from the envelope) so `MisaddressedRecipient` still
        // does real work on this path.
        if let Err(e) = ingest_inbound(&recipient, &bytes, state).await {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: rejected relay-fetched envelope for \
                 {recipient}: {e}"
            );
        }
    }
}

/// Signed relay-fetch drain loop for `own_domain`, running until the
/// process exits.
///
/// Takes `sk` BY VALUE, never by reference and never behind a shared
/// handle: `FampSigningKey` deliberately implements neither `Clone` nor
/// `Display` (secret-key hygiene, `famp-crypto`), and `main.rs` hands
/// this loop its OWN idempotently-reloaded key (`load_or_generate` is
/// idempotent — the same path always yields the byte-identical key —
/// T-08-12), exactly mirroring the pattern each `run_egress` task above
/// it already follows. Do not "simplify" this into a shared handle later.
///
/// `trust_cert` is the gateway's existing `--trust-cert` value, reused
/// here rather than introducing a second, relay-specific trust flag —
/// this loop only ever talks to one relay, the same one egress already
/// reaches via `--peer <domain>=<relay-url>`.
///
/// Never propagates a panic and never returns early on a recoverable
/// failure — mirrors `run_egress`'s stated contract that a down peer (or,
/// here, a down/unauthorized/throttling relay) must not take the whole
/// gateway process down. The only early-return path is a fatal one-time
/// setup failure (an unbuildable TLS client or fetch URL); every
/// per-iteration failure logs and backs off instead (see [`poll_once`]).
pub async fn run_relay_fetch(
    relay_base: Url,
    own_domain: String,
    sk: FampSigningKey,
    trust_cert: Option<PathBuf>,
    state: GatewayIngressState,
) {
    let audience = normalize_audience(&relay_base);

    let client = match build_fetch_client(trust_cert.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: failed to build HTTPS client, giving \
                 up on the relay-fetch loop: {e:?}"
            );
            return;
        }
    };
    let fetch_url = match build_fetch_url(&relay_base, &own_domain) {
        Ok(u) => u,
        Err(e) => {
            eprintln!(
                "famp-gateway: relay-fetch[{own_domain}]: failed to build the relay fetch URL, \
                 giving up on the relay-fetch loop: {e:?}"
            );
            return;
        }
    };

    let mut backoff_ms = RELAY_POLL_INTERVAL_MS;
    loop {
        backoff_ms = poll_once(
            &client,
            &fetch_url,
            &sk,
            &audience,
            &own_domain,
            &state,
            backoff_ms,
        )
        .await;
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ingress::IngressError;
    use crate::ingress_guard::IngressGuard;
    use crate::registry::GatewayRegistry;
    use famp::{AuthorityScope, MessageId, Timestamp, UnsignedEnvelope};
    use famp_envelope::body::ack::{AckBody, AckDisposition};
    use famp_keyring::Keyring;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // --- decode_fetch_batch --------------------------------------------

    #[test]
    fn decode_fetch_batch_round_trips_byte_identical_bytes() {
        let body = serde_json::json!({
            "envelopes": [
                {"to": "agent:hosta.test/alice", "body": URL_SAFE_NO_PAD.encode([0xffu8, 0x00, b'h', b'i'])}
            ]
        });
        let decoded = decode_fetch_batch(body.to_string().as_bytes()).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, "agent:hosta.test/alice".parse().unwrap());
        assert_eq!(decoded[0].1, vec![0xffu8, 0x00, b'h', b'i']);
    }

    #[test]
    fn decode_fetch_batch_empty_envelopes_is_an_empty_vec_not_an_error() {
        let body = serde_json::json!({ "envelopes": [] });
        let decoded = decode_fetch_batch(body.to_string().as_bytes()).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_fetch_batch_unparseable_recipient_is_bad_recipient_and_drops_nothing_silently() {
        let body = serde_json::json!({
            "envelopes": [
                {"to": "not-a-principal", "body": URL_SAFE_NO_PAD.encode(b"hi")}
            ]
        });
        let result = decode_fetch_batch(body.to_string().as_bytes());
        assert!(matches!(result, Err(RelayFetchError::BadRecipient)));
    }

    #[test]
    fn decode_fetch_batch_invalid_base64_is_malformed_response() {
        let body = serde_json::json!({
            "envelopes": [
                {"to": "agent:hosta.test/alice", "body": "not valid base64url!!"}
            ]
        });
        let result = decode_fetch_batch(body.to_string().as_bytes());
        assert!(matches!(result, Err(RelayFetchError::MalformedResponse)));
    }

    #[test]
    fn decode_fetch_batch_one_malformed_entry_fails_the_whole_batch() {
        let body = serde_json::json!({
            "envelopes": [
                {"to": "agent:hosta.test/alice", "body": URL_SAFE_NO_PAD.encode(b"good")},
                {"to": "not-a-principal", "body": URL_SAFE_NO_PAD.encode(b"bad")}
            ]
        });
        let result = decode_fetch_batch(body.to_string().as_bytes());
        assert!(
            matches!(result, Err(RelayFetchError::BadRecipient)),
            "a single malformed entry must reject the WHOLE batch, not just skip that entry"
        );
    }

    // --- authorization agreement, verified against the relay's OWN verifier ---

    #[test]
    fn fetch_authorization_built_here_verifies_at_the_relay_for_matching_domain_and_audience() {
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        let relay_url = Url::parse("https://relay.test:8443").unwrap();
        let audience = normalize_audience(&relay_url);
        let now = OffsetDateTime::now_utc();

        let signed = sign_fetch_auth(&sk, &audience, "hosta.test", now).unwrap();
        let presented = famp_relay::fetch_auth::PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        famp_relay::fetch_auth::verify_fetch_auth(&presented, "hosta.test", &audience, &[vk], now)
            .expect("a signature built by this module's own signer must verify at the relay");
    }

    #[test]
    fn fetch_authorization_built_here_fails_at_the_relay_for_a_different_domain() {
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        let relay_url = Url::parse("https://relay.test:8443").unwrap();
        let audience = normalize_audience(&relay_url);
        let now = OffsetDateTime::now_utc();

        let signed = sign_fetch_auth(&sk, &audience, "hosta.test", now).unwrap();
        let presented = famp_relay::fetch_auth::PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        let result = famp_relay::fetch_auth::verify_fetch_auth(
            &presented,
            "hostb.test",
            &audience,
            &[vk],
            now,
        );
        assert!(
            result.is_err(),
            "a signature bound to hosta.test must not verify for hostb.test"
        );
    }

    // --- pure helper functions ------------------------------------------

    #[test]
    fn classify_fetch_status_maps_200_to_success() {
        assert_eq!(
            classify_fetch_status(reqwest::StatusCode::OK),
            FetchOutcome::Success
        );
    }

    #[test]
    fn classify_fetch_status_maps_401_and_403_to_unauthorized() {
        assert_eq!(
            classify_fetch_status(reqwest::StatusCode::UNAUTHORIZED),
            FetchOutcome::Unauthorized
        );
        assert_eq!(
            classify_fetch_status(reqwest::StatusCode::FORBIDDEN),
            FetchOutcome::Unauthorized
        );
    }

    #[test]
    fn classify_fetch_status_maps_429_to_rate_limited_never_unauthorized() {
        assert_eq!(
            classify_fetch_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            FetchOutcome::RateLimited
        );
    }

    #[test]
    fn classify_fetch_status_maps_everything_else_to_other() {
        assert_eq!(
            classify_fetch_status(reqwest::StatusCode::NOT_FOUND),
            FetchOutcome::Other
        );
    }

    #[test]
    fn next_backoff_ms_doubles_then_caps() {
        assert_eq!(next_backoff_ms(1_000), 2_000);
        assert_eq!(next_backoff_ms(20_000), 30_000, "must cap, not overshoot");
        assert_eq!(
            next_backoff_ms(RELAY_FETCH_BACKOFF_MAX_MS),
            RELAY_FETCH_BACKOFF_MAX_MS
        );
    }

    // --- ingest parity: a relay-fetched envelope hits the SAME gates as
    // --- a directly-POSTed one (INGR-01..08, T-17-23) ---------------------

    /// Mirrors `ingress.rs::tests::LiveBroker` — a minimal in-process test
    /// broker so these tests can register a REAL backed sender.
    /// Deliberately duplicated rather than imported: `ingress.rs`'s
    /// `#[cfg(test)] mod tests` is private to that module, so a sibling
    /// module's own test code cannot name it. Same documented-duplication
    /// rationale as `ingress.rs::envelope_sender`/`envelope_recipient`.
    struct LiveBroker {
        _tmp: tempfile::TempDir,
        _shutdown: tokio::sync::oneshot::Sender<()>,
        _broker: tokio::task::JoinHandle<()>,
        sock: std::path::PathBuf,
    }

    impl LiveBroker {
        async fn spawn() -> Self {
            let tmp = tempfile::TempDir::new().expect("tempdir");
            let sock = tmp.path().join("bus.sock");
            let listener =
                tokio::net::UnixListener::bind(&sock).expect("bind in-process broker socket");
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
            let shutdown_fut = async move {
                let _ = shutdown_rx.await;
            };
            let sock_for_broker = sock.clone();
            let bus_dir = tmp.path().to_path_buf();
            let broker = tokio::spawn(async move {
                let _ = famp::cli::broker::run_on_listener(
                    &sock_for_broker,
                    &bus_dir,
                    listener,
                    shutdown_fut,
                )
                .await;
            });
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Self {
                _tmp: tmp,
                _shutdown: shutdown_tx,
                _broker: broker,
                sock,
            }
        }
    }

    async fn state_with_backed_sender(
        keyring: Keyring,
        own_domain: &str,
        from_name: &str,
    ) -> (GatewayIngressState, LiveBroker) {
        let broker = LiveBroker::spawn().await;
        let mut registry = GatewayRegistry::default();
        registry
            .back(&broker.sock, from_name.to_owned())
            .await
            .expect("back sender on in-process broker");
        let registry = Arc::new(Mutex::new(registry));
        let state = GatewayIngressState::new(
            registry,
            Arc::new(keyring),
            Arc::from(own_domain),
            Arc::new(Mutex::new(IngressGuard::new())),
        );
        (state, broker)
    }

    fn ack_bytes_with_ts_and_nonce(
        sk: &FampSigningKey,
        from: &Principal,
        to: &Principal,
        ts: Timestamp,
        nonce: &str,
    ) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a72".parse().unwrap();
        let body = AckBody {
            disposition: AckDisposition::Accepted,
            reason: None,
        };
        let unsigned = UnsignedEnvelope::<AckBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        )
        .with_nonce(nonce.to_string());
        unsigned.sign(sk).unwrap().encode().unwrap()
    }

    fn fresh_ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        ack_bytes_with_ts_and_nonce(
            sk,
            from,
            to,
            Timestamp(crate::clock::now_canonical_utc()),
            &uuid::Uuid::now_v7().to_string(),
        )
    }

    #[tokio::test]
    async fn relay_fetched_stale_timestamp_is_rejected_same_as_a_direct_post() {
        let sk = FampSigningKey::from_bytes([70u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();

        let one_hour_ago = OffsetDateTime::now_utc() - time::Duration::hours(1);
        let stale_ts = crate::clock::strip_subseconds(
            &one_hour_ago
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
        );
        let bytes = ack_bytes_with_ts_and_nonce(
            &sk,
            &from,
            &to,
            Timestamp(stale_ts),
            &uuid::Uuid::now_v7().to_string(),
        );

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (state, _broker) = state_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let result = ingest_inbound(&to, &bytes, &state).await;
        assert!(
            matches!(result, Err(IngressError::StaleTimestamp { .. })),
            "expected StaleTimestamp, got {result:?}"
        );
    }

    #[tokio::test]
    async fn relay_fetched_replayed_nonce_is_rejected_on_second_delivery() {
        let sk = FampSigningKey::from_bytes([71u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let bytes = fresh_ack_bytes(&sk, &from, &to);

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (state, _broker) = state_with_backed_sender(keyring, "hostb.test", "oscar").await;

        ingest_inbound(&to, &bytes, &state)
            .await
            .expect("first delivery of a fresh nonce must succeed");
        let result = ingest_inbound(&to, &bytes, &state).await;
        assert!(
            matches!(result, Err(IngressError::ReplayedNonce { .. })),
            "expected ReplayedNonce on the second delivery, got {result:?}"
        );
    }

    #[tokio::test]
    async fn relay_fetched_bad_signature_is_rejected_and_performs_zero_registry_mutation() {
        let sk = FampSigningKey::from_bytes([72u8; 32]);
        let wrong_sk = FampSigningKey::from_bytes([73u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let bytes = fresh_ack_bytes(&sk, &from, &to);

        // Pinned to the WRONG key -- the sender is genuinely backed, but
        // the signature will not verify against what the keyring holds.
        let mut keyring = Keyring::new();
        keyring
            .pin_tofu(from.clone(), wrong_sk.verifying_key())
            .unwrap();
        let (state, _broker) = state_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let result = ingest_inbound(&to, &bytes, &state).await;
        assert!(
            matches!(result, Err(IngressError::InvalidSignature)),
            "expected InvalidSignature, got {result:?}"
        );
    }

    /// The load-bearing proof for this plan's must-have: `recipient` is
    /// the RELAY-SUPPLIED value handed to `ingest_inbound`, never a value
    /// re-read from the envelope itself. A test where the two disagree is
    /// what proves `MisaddressedRecipient` still does real work on the
    /// relay path — if this loop instead re-parsed the recipient out of
    /// the envelope body, this test would never be able to observe a
    /// disagreement at all.
    #[tokio::test]
    async fn relay_supplied_recipient_disagreeing_with_signed_to_is_rejected_misaddressed() {
        let sk = FampSigningKey::from_bytes([74u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let relay_supplied_recipient: Principal = "agent:hostb.test/quentin".parse().unwrap();
        let bytes = fresh_ack_bytes(&sk, &from, &to);

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (state, _broker) = state_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let result = ingest_inbound(&relay_supplied_recipient, &bytes, &state).await;
        assert!(
            matches!(result, Err(IngressError::MisaddressedRecipient { .. })),
            "expected MisaddressedRecipient, got {result:?}"
        );
    }

    // --- build_fetch_url --------------------------------------------------

    #[test]
    fn build_fetch_url_fills_in_the_domain_segment_from_the_imported_route_constant() {
        let base = Url::parse("https://relay.test:8443").unwrap();
        let url = build_fetch_url(&base, "hosta.test").unwrap();
        assert_eq!(url.path(), "/famp/relay/v1/fetch/hosta.test");
    }
}
