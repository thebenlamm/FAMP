//! Signed fetch authorization (Task 2, D-26): one signed form, one
//! verification routine, bounded freshness and replay state.
//!
//! What is signed: a small fixed-shape authorization request
//! ([`FetchAuthRequest`]), canonicalized with RFC 8785 JCS and signed
//! with the SAME `famp_crypto::sign_value` entry point every FAMP
//! envelope uses, under the same `FAMP-sig-v1\0` domain prefix and the
//! same strict `verify_strict` verification path. There is no second
//! verification routine in this codebase, and adding one would be the
//! bug (D-26).
//!
//! What it authorizes: draining ONE named domain's queue. Nothing else.
//! Per D-25, this is a CONFIDENTIALITY decision, not merely an
//! availability one — whoever can drain a queue can read it.
//!
//! What it is NOT: a decision about any message's trust. That stays
//! exclusively with the receiving gateway's inbound check against its
//! pinned keyring (D-02, unchanged). Verifying WHO MAY DRAIN and
//! verifying WHAT WAS SENT are different trust domains, and only the
//! first happens here.
//!
//! Why cross-protocol confusion is structurally impossible: the verifier
//! ([`verify_fetch_auth`]) NEVER accepts a signed form off the wire — it
//! reconstructs the form itself from the URL path, the presented
//! headers, and its own configuration. Combined with the `typ` constant
//! this makes an envelope signature unusable as a fetch authorization
//! and vice versa. [`FetchAuthRequest`] derives only the serialization
//! half of serde for exactly this reason: a type that cannot be built
//! from wire bytes cannot be smuggled in as one.

use famp_crypto::{FampSignature, FampSigningKey, TrustedVerifyingKey};
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// The fetch route this module's signed form authorizes a drain of.
/// `{domain}` is the same destination-domain segment the enqueue route
/// (`famp_transport_http::INBOX_ROUTE`) keys its queues by.
pub const RELAY_FETCH_ROUTE: &str = "/famp/relay/v1/fetch/{domain}";

/// The `typ` value stamped into every [`FetchAuthRequest`].
///
/// Distinguishes this signed form from any other value this codebase
/// ever signs with `famp_crypto::sign_value` (an envelope, a revocation
/// statement, …) — the reconstruct-don't-accept verification discipline
/// above is the primary defense; this constant is the cheap, visible
/// second layer.
pub const FETCH_AUTH_TYP: &str = "famp-relay-fetch-v1";

pub const RELAY_AUTH_AUD_HEADER: &str = "x-famp-relay-aud";
pub const RELAY_AUTH_TS_HEADER: &str = "x-famp-relay-ts";
pub const RELAY_AUTH_NONCE_HEADER: &str = "x-famp-relay-nonce";
pub const RELAY_AUTH_SIG_HEADER: &str = "x-famp-relay-sig";

/// A presented header value above this many bytes is rejected before it
/// is ever used as a map key.
///
/// The nonce becomes a `HashMap` key in
/// [`FetchAuthState::record_nonce`] — an unbounded key is a memory-
/// amplification vector.
pub const RELAY_AUTH_HEADER_MAX_BYTES: usize = 512;

/// A fetch authorization's `ts` must lie within this many seconds of the
/// relay's own wall clock, in either direction, inclusive.
///
/// This mirrors, by intent and by value, the gateway's INGR-01
/// clock-skew window, but it CANNOT import it: `famp-gateway` depends on
/// this crate, and the reverse edge would be a cycle. The duplication is
/// deliberate; the two knobs are independent by necessity, so a future
/// retune of one must be a conscious decision about the other.
pub const FETCH_FRESHNESS_WINDOW_SECS: i64 = 300;

/// How long a recorded nonce is remembered before [`FetchAuthState::sweep`]
/// reclaims it. See [`fetch_bounds_ok`] for the enforced relationship to
/// [`FETCH_FRESHNESS_WINDOW_SECS`].
pub const FETCH_NONCE_TTL_SECS: i64 = 600;

/// Per-domain cap on remembered nonces. See [`fetch_bounds_ok`] for the
/// enforced relationship to the rate limiter.
pub const FETCH_NONCE_MAX_PER_DOMAIN: usize = 4096;

/// Fixed-window rate limit: at most this many fetch attempts admitted per
/// domain per [`FETCH_RATE_WINDOW_SECS`].
pub const FETCH_RATE_MAX_PER_WINDOW: u32 = 120;
pub const FETCH_RATE_WINDOW_SECS: i64 = 60;

/// Bound on configured keys per domain (enforced at relay startup in
/// `main.rs`).
///
/// Multiple keys per domain is deliberate — make-before-break key
/// rotation support, the story Phase 15 already built for keys and that
/// a shared secret would have needed reinvented (D-26 reason 4). Each
/// configured key is one more signature verification an unauthorized
/// caller can force, so the list is bounded: an unbounded list would
/// turn the fetch route into the very verification amplifier INGR-05
/// exists to prevent.
pub const RELAY_MAX_KEYS_PER_DOMAIN: usize = 4;

/// The freshness/TTL/size relation, as executable code (D-06's
/// discipline applied to this box, mirroring plan 02's
/// `replay_bounds_ok` for the gateway).
///
/// True only when both relations hold:
///
/// (a) `nonce_ttl_secs >= 2 * freshness_secs` — a request with a fixed
///     timestamp is admissible across a span of twice the freshness
///     window (once for a clock arbitrarily far behind, once for a
///     clock arbitrarily far ahead), and a nonce entry expiring sooner
///     than that would let a genuine replay through after eviction but
///     before the freshness gate would have closed on its own.
///
/// (b) `max_nonces >= nonce_ttl_secs * rate_max / rate_window_secs` — a
///     cap below the number of nonces the rate limiter can admit within
///     one TTL starts evicting live entries under load, reopening the
///     replay window exactly when traffic is highest.
///
/// Returns `false` on a non-positive rate window or negative
/// freshness/TTL input (nothing meaningful to bound).
#[must_use]
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
pub const fn fetch_bounds_ok(
    freshness_secs: i64,
    nonce_ttl_secs: i64,
    max_nonces: usize,
    rate_max: u32,
    rate_window_secs: i64,
) -> bool {
    if freshness_secs < 0 || nonce_ttl_secs < 0 || rate_window_secs <= 0 {
        return false;
    }

    let ttl_covers_freshness = nonce_ttl_secs >= 2 * freshness_secs;

    // Widen every operand to i128 for the cap inequality so the
    // ttl * rate multiplication cannot overflow before the division
    // runs. The `as` casts below are widenings of values already proven
    // non-negative by the guard above (or, for `max_nonces`/`rate_max`,
    // unsigned by type) into a strictly wider signed type — safe by
    // construction, which is why the pedantic sign/wrap/lossless casts
    // are allowed on this function specifically rather than crate-wide.
    let ttl = nonce_ttl_secs as i128;
    let rate = rate_max as i128;
    let window = rate_window_secs as i128;
    let max_n = max_nonces as i128;
    let admitted_per_ttl = (ttl * rate) / window;
    let cap_covers_admitted = max_n >= admitted_per_ttl;

    ttl_covers_freshness && cap_covers_admitted
}

// D-06: a violating retune must fail to COMPILE, not merely fail a test
// run someone forgot to execute.
const _: () = assert!(fetch_bounds_ok(
    FETCH_FRESHNESS_WINDOW_SECS,
    FETCH_NONCE_TTL_SECS,
    FETCH_NONCE_MAX_PER_DOMAIN,
    FETCH_RATE_MAX_PER_WINDOW,
    FETCH_RATE_WINDOW_SECS,
));

/// The signed form itself. Field declaration order is not part of the
/// contract — JCS sorts keys before signing, so do not "fix" the
/// ordering below thinking it matters.
///
/// Derives `Serialize` only, deliberately never `Deserialize` — see this
/// module's doc comment for why that omission is load-bearing rather
/// than an oversight: a type that cannot be constructed from wire bytes
/// cannot be smuggled in as one.
#[derive(Debug, serde::Serialize)]
pub struct FetchAuthRequest<'a> {
    pub typ: &'a str,
    pub aud: &'a str,
    pub domain: &'a str,
    pub nonce: &'a str,
    pub ts: &'a str,
}

/// A freshly minted, signed fetch authorization, ready to be sent as the
/// four `x-famp-relay-*` headers via [`SignedFetchAuth::header_pairs`].
///
/// The gateway's fetch client calls only [`sign_fetch_auth`] and this
/// type's `header_pairs` — it never assembles the signed form itself,
/// which is what keeps one definition of the contract.
#[derive(Debug, Clone)]
pub struct SignedFetchAuth {
    pub aud: String,
    pub ts: String,
    pub nonce: String,
    pub signature_b64url: String,
}

impl SignedFetchAuth {
    #[must_use]
    pub const fn header_pairs(&self) -> [(&'static str, &str); 4] {
        [
            (RELAY_AUTH_AUD_HEADER, self.aud.as_str()),
            (RELAY_AUTH_TS_HEADER, self.ts.as_str()),
            (RELAY_AUTH_NONCE_HEADER, self.nonce.as_str()),
            (RELAY_AUTH_SIG_HEADER, self.signature_b64url.as_str()),
        ]
    }
}

/// The four header values as presented on an inbound request, borrowed
/// straight from the request's header map. Not yet validated in any
/// way — [`verify_fetch_auth`] is what validates them.
#[derive(Debug, Clone, Copy)]
pub struct PresentedFetchAuth<'a> {
    pub aud: &'a str,
    pub ts: &'a str,
    pub nonce: &'a str,
    pub signature_b64url: &'a str,
}

/// Rejections a fetch authorization can produce — spanning both the
/// route handler's own checks (`MissingHeader`, `RateLimited`) and
/// [`verify_fetch_auth`]'s pure checks.
///
/// The whole fetch route maps a single enum onto its
/// distinct-status-per-cause HTTP surface. No variant carries key bytes,
/// signature bytes, or any body content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FetchAuthReject {
    /// One of the four `x-famp-relay-*` headers was absent. Produced by
    /// the route handler before a [`PresentedFetchAuth`] can even be
    /// constructed.
    #[error("missing fetch-authorization header '{header}'")]
    MissingHeader { header: &'static str },

    /// A presented header exceeds [`RELAY_AUTH_HEADER_MAX_BYTES`]. Runs
    /// FIRST among [`verify_fetch_auth`]'s own checks — the nonce
    /// becomes a map key, and an unbounded key is a memory-amplification
    /// vector.
    #[error("fetch-authorization header '{header}' exceeds the maximum allowed length")]
    HeaderTooLong { header: &'static str },

    /// The presented `aud` does not equal this relay's own normalized
    /// public URL. Carries both values because this is the one
    /// misconfiguration a follower will actually hit.
    #[error("fetch audience mismatch: expected '{expected}', presented '{presented}'")]
    AudienceMismatch { expected: String, presented: String },

    /// The presented `ts` does not parse as RFC 3339. Never a panic,
    /// never a silent Ok.
    #[error("fetch timestamp is not a parseable RFC 3339 value")]
    MalformedTimestamp,

    /// The presented `ts` parses but lies outside
    /// [`FETCH_FRESHNESS_WINDOW_SECS`] of the relay's own clock.
    #[error("fetch timestamp is outside the freshness window")]
    StaleFetch,

    /// The presented signature string is not valid strict unpadded
    /// URL-safe base64. A signature that decodes under a lax decoder but
    /// not this one is a bug at the caller, never a reason to relax the
    /// decoder (famp-crypto's own documented pitfall, inherited here).
    #[error("fetch signature is not valid strict unpadded base64url")]
    MalformedSignature,

    /// The signature decoded but did not verify against any configured
    /// key for `domain`. Carries the domain and NOTHING else — never key
    /// bytes, never signature bytes.
    #[error("fetch signature did not verify for domain '{domain}'")]
    BadSignature { domain: String },

    /// The presented nonce was already recorded for this domain within
    /// [`FETCH_NONCE_TTL_SECS`] — a replay of a previously-successful
    /// authorization.
    #[error("fetch authorization nonce was already used for this domain")]
    ReplayedNonce,

    /// This domain's fetch rate limit was exceeded for the current
    /// window.
    #[error("fetch rate limit exceeded for this domain")]
    RateLimited,
}

/// The canonical whole-second UTC form `YYYY-MM-DDTHH:MM:SSZ`, built
/// positionally exactly like `famp-gateway`'s `clock::now_canonical_utc`
/// (a deliberate, gateway-crate-boundary-respecting duplicate — see this
/// module's doc comment on why `famp-gateway`'s own helpers cannot be
/// imported here). Infallible: no fallible-clock branch anywhere on this
/// signing path.
fn format_ts(now: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Scheme + lowercased host + explicit non-default port only — no path,
/// no query, no trailing slash.
///
/// Both sides call THIS function (the relay on its own `--public-url` at
/// startup, the gateway on its `--relay-fetch` URL), so audience
/// comparison is exact-string equality over one shared normalization
/// instead of two hand-written ones that drift. A follower hitting a
/// spurious authorization failure over a trailing slash would be the
/// worst possible failure against the unassisted-follower bar.
///
/// Relies on `url`'s own behavior that an explicitly-written default
/// port for the scheme is not reported by `Url::port()` — this is why an
/// operator writing the port explicitly and one omitting it produce the
/// same audience.
#[must_use]
pub fn normalize_audience(url: &url::Url) -> String {
    let scheme = url.scheme();
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match url.port() {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    }
}

/// Mint and sign a fresh fetch authorization for `domain`, addressed to
/// `aud` (the relay's normalized public URL).
///
/// Takes `now` as a parameter rather than reading the clock internally —
/// this codebase's established preference for time-as-input, so
/// freshness behavior is testable without sleeping.
///
/// # Errors
///
/// Only (structurally unreachable for this fixed all-`&str` shape, which
/// cannot contain a non-finite float or exceed any recursion bound) if
/// RFC 8785 canonicalization itself fails. Mapped onto
/// [`FetchAuthReject::MalformedSignature`] because no other variant
/// describes "the signing machinery itself failed"; this branch is not
/// exercised by any test because it cannot be triggered by this
/// function's fixed input shape.
pub fn sign_fetch_auth(
    sk: &FampSigningKey,
    aud: &str,
    domain: &str,
    now: OffsetDateTime,
) -> Result<SignedFetchAuth, FetchAuthReject> {
    let nonce = uuid::Uuid::now_v7().to_string();
    let ts = format_ts(now);
    let request = FetchAuthRequest {
        typ: FETCH_AUTH_TYP,
        aud,
        domain,
        nonce: &nonce,
        ts: &ts,
    };
    let signature =
        famp_crypto::sign_value(sk, &request).map_err(|_| FetchAuthReject::MalformedSignature)?;
    Ok(SignedFetchAuth {
        aud: aud.to_owned(),
        ts,
        nonce,
        signature_b64url: signature.to_b64url(),
    })
}

/// Verify a presented fetch authorization for `domain` against
/// `expected_aud` and the domain's configured `keys`, as of `now`.
///
/// Order is a contract later edits must preserve:
///
/// 1. every presented header value at or under
///    [`RELAY_AUTH_HEADER_MAX_BYTES`], else [`FetchAuthReject::HeaderTooLong`]
///    — first because the nonce becomes a map key downstream, and an
///    unbounded key is a memory-amplification vector;
/// 2. `presented.aud` equals `expected_aud`, else
///    [`FetchAuthReject::AudienceMismatch`];
/// 3. `presented.ts` parses as RFC 3339 and lies within
///    [`FETCH_FRESHNESS_WINDOW_SECS`] of `now` in either direction,
///    inclusive, else [`FetchAuthReject::MalformedTimestamp`] or
///    [`FetchAuthReject::StaleFetch`] — parsed-instant comparison only,
///    never a lexical string comparison (the REL-02 precedent: a
///    non-canonical offset can lexically misorder against a later
///    instant);
/// 4. the signature string decodes as a `FampSignature`, else
///    [`FetchAuthReject::MalformedSignature`];
/// 5. only now, reconstruct [`FetchAuthRequest`] — `typ` from the
///    constant, `domain` from the caller's argument (which the route
///    handler takes from the URL PATH, never a header — this is what
///    binds a signature to ONE queue), `aud`/`nonce`/`ts` from the
///    presented headers VERBATIM (never reformatted — re-serializing a
///    timestamp or re-encoding a nonce would change the canonical bytes
///    and break a perfectly good signature) — and try
///    `famp_crypto::verify_value` against each configured key in turn,
///    returning `Ok` on the first that verifies and
///    [`FetchAuthReject::BadSignature`] if none do.
pub fn verify_fetch_auth(
    presented: &PresentedFetchAuth<'_>,
    domain: &str,
    expected_aud: &str,
    keys: &[TrustedVerifyingKey],
    now: OffsetDateTime,
) -> Result<(), FetchAuthReject> {
    for (header, value) in [
        (RELAY_AUTH_AUD_HEADER, presented.aud),
        (RELAY_AUTH_TS_HEADER, presented.ts),
        (RELAY_AUTH_NONCE_HEADER, presented.nonce),
        (RELAY_AUTH_SIG_HEADER, presented.signature_b64url),
    ] {
        if value.len() > RELAY_AUTH_HEADER_MAX_BYTES {
            return Err(FetchAuthReject::HeaderTooLong { header });
        }
    }

    if presented.aud != expected_aud {
        return Err(FetchAuthReject::AudienceMismatch {
            expected: expected_aud.to_owned(),
            presented: presented.aud.to_owned(),
        });
    }

    let ts_instant = OffsetDateTime::parse(presented.ts, &Rfc3339)
        .map_err(|_| FetchAuthReject::MalformedTimestamp)?;
    // `presented.ts` was minted by `format_ts`, which floors to whole
    // seconds. `now` (in production, a fresh `OffsetDateTime::now_utc()`
    // call) carries sub-second precision. Comparing the two directly
    // would make the diff's rounding direction depend on `now`'s
    // fractional phase, silently shifting the inclusive boundary by up
    // to a second. Floor `now` to whole seconds too, the same way
    // `format_ts` does, so both sides of the comparison have identical
    // precision and the boundary is exact.
    let now_secs = now.replace_nanosecond(0).unwrap_or(now);
    let diff_secs = (now_secs - ts_instant).whole_seconds().abs();
    if diff_secs > FETCH_FRESHNESS_WINDOW_SECS {
        return Err(FetchAuthReject::StaleFetch);
    }

    let signature = FampSignature::from_b64url(presented.signature_b64url)
        .map_err(|_| FetchAuthReject::MalformedSignature)?;

    let request = FetchAuthRequest {
        typ: FETCH_AUTH_TYP,
        aud: presented.aud,
        domain,
        nonce: presented.nonce,
        ts: presented.ts,
    };
    for key in keys {
        if famp_crypto::verify_value(key, &request, &signature).is_ok() {
            return Ok(());
        }
    }
    Err(FetchAuthReject::BadSignature {
        domain: domain.to_owned(),
    })
}

/// Process-lifetime pre-drain replay-cache and rate-limit state, one
/// instance shared across every fetch this relay serves.
///
/// The outer key on `nonces` is the DOMAIN — load-bearing for the same
/// reason the gateway's own per-sender replay scoping is: it is what
/// makes one domain structurally unable to evict or collide with
/// another's entries. A flattened map would silently reintroduce that.
#[derive(Default)]
pub struct FetchAuthState {
    nonces: HashMap<String, HashMap<String, OffsetDateTime>>,
    buckets: HashMap<String, (OffsetDateTime, u32)>,
}

impl FetchAuthState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record `nonce` as seen for `domain` as of `now`. `Ok` on a first
    /// sighting, [`FetchAuthReject::ReplayedNonce`] on a second sighting
    /// inside [`FETCH_NONCE_TTL_SECS`].
    ///
    /// Called ONLY after [`verify_fetch_auth`] has already succeeded —
    /// see this module's `http` sibling for the divergence this creates
    /// from the gateway's INGR-05 ordering (there, the replay check runs
    /// BEFORE signature verification; here it runs after). The reason:
    /// the gateway's replay cache is keyed on a name drawn from its
    /// fixed `--backs` set, whereas THIS cache is reachable by an
    /// ANONYMOUS caller — recording before verification would let
    /// anyone at all fill or evict a legitimate domain's nonce entries.
    /// Freshness and the rate limit still run before verification, so
    /// the cheap-before-expensive property that matters for DoS is
    /// preserved; only the cache WRITE is deferred behind proof of
    /// authorization. A considered divergence with a named reason, not
    /// an oversight.
    pub fn record_nonce(
        &mut self,
        domain: &str,
        nonce: &str,
        now: OffsetDateTime,
    ) -> Result<(), FetchAuthReject> {
        let entries = self.nonces.entry(domain.to_owned()).or_default();
        entries
            .retain(|_, recorded_at| (now - *recorded_at).whole_seconds() <= FETCH_NONCE_TTL_SECS);

        if entries.contains_key(nonce) {
            return Err(FetchAuthReject::ReplayedNonce);
        }

        if entries.len() >= FETCH_NONCE_MAX_PER_DOMAIN {
            // Oldest by recorded instant, ties broken by the
            // lexicographically smaller nonce, so eviction never depends
            // on map iteration order.
            let evict = entries
                .iter()
                .fold(None::<(&str, OffsetDateTime)>, |acc, (k, v)| match acc {
                    None => Some((k.as_str(), *v)),
                    Some((bk, bv)) if *v < bv || (*v == bv && k.as_str() < bk) => {
                        Some((k.as_str(), *v))
                    }
                    Some(existing) => Some(existing),
                })
                .map(|(k, _)| k.to_owned());
            if let Some(k) = evict {
                entries.remove(&k);
            }
        }

        entries.insert(nonce.to_owned(), now);
        Ok(())
    }

    /// Fixed-window per-domain rate limiter.
    ///
    /// The rate-limit key is the DOMAIN from the URL path, whose key
    /// space is fixed at process startup by the operator's `--domain`
    /// arguments — an unconfigured domain is already rejected before
    /// this runs. That is the same not-attacker-rotatable property D-10
    /// demands, reached by a different route than the gateway's (which
    /// keys on sender identity rather than a URL segment).
    ///
    /// Residual, stated honestly: an anonymous caller CAN burn a
    /// configured domain's verification budget by hammering that
    /// domain's fetch route, which delays the legitimate gateway's next
    /// successful drain — messages stay queued until their TTL, so this
    /// is added latency rather than loss, and plan 05's fetch loop backs
    /// off rather than treating it as a fatal misconfiguration.
    pub fn admit(&mut self, domain: &str, now: OffsetDateTime) -> Result<(), FetchAuthReject> {
        let bucket = self.buckets.entry(domain.to_owned()).or_insert((now, 0));
        if (now - bucket.0).whole_seconds() >= FETCH_RATE_WINDOW_SECS {
            *bucket = (now, 0);
        }
        if bucket.1 >= FETCH_RATE_MAX_PER_WINDOW {
            return Err(FetchAuthReject::RateLimited);
        }
        bucket.1 += 1;
        Ok(())
    }

    /// Drop expired nonce entries across every domain and remove any
    /// domain map left empty, mirroring [`crate::queue::RelayQueues::sweep`].
    pub fn sweep(&mut self, now: OffsetDateTime) {
        for entries in self.nonces.values_mut() {
            entries.retain(|_, recorded_at| {
                (now - *recorded_at).whole_seconds() <= FETCH_NONCE_TTL_SECS
            });
        }
        self.nonces.retain(|_, entries| !entries.is_empty());
    }

    #[must_use]
    pub fn nonce_len_for(&self, domain: &str) -> usize {
        self.nonces.get(domain).map_or(0, HashMap::len)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn keypair() -> (FampSigningKey, TrustedVerifyingKey) {
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    #[test]
    fn normalize_audience_ignores_trailing_slash_case_and_default_port() {
        let a = url::Url::parse("https://Example.com:443/").unwrap();
        let b = url::Url::parse("https://example.com").unwrap();
        assert_eq!(normalize_audience(&a), normalize_audience(&b));

        let different_port = url::Url::parse("https://example.com:8443").unwrap();
        assert_ne!(normalize_audience(&a), normalize_audience(&different_port));

        let different_host = url::Url::parse("https://other.example.com").unwrap();
        assert_ne!(normalize_audience(&a), normalize_audience(&different_host));
    }

    #[test]
    fn sign_then_verify_round_trips_ok() {
        let (sk, vk) = keypair();
        let t = now();
        let aud = "https://relay.test";
        let signed = sign_fetch_auth(&sk, aud, "hosta.test", t).unwrap();
        let presented = PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        verify_fetch_auth(&presented, "hosta.test", aud, &[vk], t).unwrap();
    }

    #[test]
    fn signature_for_domain_a_fails_for_domain_b() {
        let (sk, vk) = keypair();
        let t = now();
        let aud = "https://relay.test";
        let signed = sign_fetch_auth(&sk, aud, "hosta.test", t).unwrap();
        let presented = PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        let result = verify_fetch_auth(&presented, "hostb.test", aud, &[vk], t);
        assert!(matches!(result, Err(FetchAuthReject::BadSignature { .. })));
    }

    #[test]
    fn audience_mismatch_names_both_values() {
        let (sk, vk) = keypair();
        let t = now();
        let signed = sign_fetch_auth(&sk, "https://relay-a.test", "hosta.test", t).unwrap();
        let presented = PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        match verify_fetch_auth(&presented, "hosta.test", "https://relay-b.test", &[vk], t) {
            Err(FetchAuthReject::AudienceMismatch {
                expected,
                presented,
            }) => {
                assert_eq!(expected, "https://relay-b.test");
                assert_eq!(presented, "https://relay-a.test");
            }
            other => panic!("expected AudienceMismatch, got {other:?}"),
        }
    }

    #[test]
    fn key_not_in_list_fails() {
        let (sk, _vk) = keypair();
        let (_other_sk, other_vk) = keypair();
        let t = now();
        let aud = "https://relay.test";
        let signed = sign_fetch_auth(&sk, aud, "hosta.test", t).unwrap();
        let presented = PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        let result = verify_fetch_auth(&presented, "hosta.test", aud, &[other_vk], t);
        assert!(matches!(result, Err(FetchAuthReject::BadSignature { .. })));
    }

    #[test]
    fn signer_as_second_of_two_configured_keys_verifies() {
        let (sk, vk) = keypair();
        let (_decoy_sk, decoy_vk) = keypair();
        let t = now();
        let aud = "https://relay.test";
        let signed = sign_fetch_auth(&sk, aud, "hosta.test", t).unwrap();
        let presented = PresentedFetchAuth {
            aud: &signed.aud,
            ts: &signed.ts,
            nonce: &signed.nonce,
            signature_b64url: &signed.signature_b64url,
        };
        verify_fetch_auth(&presented, "hosta.test", aud, &[decoy_vk, vk], t)
            .expect("signer as the SECOND configured key must still verify (rotation)");
    }

    #[test]
    fn timestamp_boundary_is_inclusive_both_directions() {
        let (sk, vk) = keypair();
        let t = now();
        let aud = "https://relay.test";

        for delta in [-FETCH_FRESHNESS_WINDOW_SECS, FETCH_FRESHNESS_WINDOW_SECS] {
            let ts_time = t + time::Duration::seconds(delta);
            let signed = sign_fetch_auth(&sk, aud, "hosta.test", ts_time).unwrap();
            let presented = PresentedFetchAuth {
                aud: &signed.aud,
                ts: &signed.ts,
                nonce: &signed.nonce,
                signature_b64url: &signed.signature_b64url,
            };
            verify_fetch_auth(&presented, "hosta.test", aud, std::slice::from_ref(&vk), t)
                .unwrap_or_else(|e| {
                    panic!("delta {delta}s must be inside the inclusive window: {e:?}")
                });
        }

        for delta in [
            -(FETCH_FRESHNESS_WINDOW_SECS + 1),
            FETCH_FRESHNESS_WINDOW_SECS + 1,
        ] {
            let ts_time = t + time::Duration::seconds(delta);
            let signed = sign_fetch_auth(&sk, aud, "hosta.test", ts_time).unwrap();
            let presented = PresentedFetchAuth {
                aud: &signed.aud,
                ts: &signed.ts,
                nonce: &signed.nonce,
                signature_b64url: &signed.signature_b64url,
            };
            let result =
                verify_fetch_auth(&presented, "hosta.test", aud, std::slice::from_ref(&vk), t);
            assert!(
                matches!(result, Err(FetchAuthReject::StaleFetch)),
                "delta {delta}s must be stale, got {result:?}"
            );
        }
    }

    #[test]
    fn unparseable_timestamp_is_malformed_never_panic_never_ok() {
        let (_sk, vk) = keypair();
        let presented = PresentedFetchAuth {
            aud: "https://relay.test",
            ts: "not-a-timestamp",
            nonce: "n1",
            signature_b64url: "AAAA",
        };
        let result =
            verify_fetch_auth(&presented, "hosta.test", "https://relay.test", &[vk], now());
        assert!(matches!(result, Err(FetchAuthReject::MalformedTimestamp)));
    }

    #[test]
    fn malformed_signature_after_staleness_already_passed() {
        let (_sk, vk) = keypair();
        let t = now();
        let fresh_ts = t.format(&Rfc3339).unwrap();
        let presented = PresentedFetchAuth {
            aud: "https://relay.test",
            ts: &fresh_ts,
            nonce: "n1",
            signature_b64url: "not-valid-base64url!!!",
        };
        let result = verify_fetch_auth(&presented, "hosta.test", "https://relay.test", &[vk], t);
        assert!(matches!(result, Err(FetchAuthReject::MalformedSignature)));
    }

    #[test]
    fn header_too_long_names_header_before_used_as_map_key() {
        let (_sk, vk) = keypair();
        let too_long = "x".repeat(RELAY_AUTH_HEADER_MAX_BYTES + 1);
        let presented = PresentedFetchAuth {
            aud: "https://relay.test",
            ts: "2026-07-27T00:00:00Z",
            nonce: &too_long,
            signature_b64url: "AAAA",
        };
        let result =
            verify_fetch_auth(&presented, "hosta.test", "https://relay.test", &[vk], now());
        assert!(matches!(
            result,
            Err(FetchAuthReject::HeaderTooLong {
                header: RELAY_AUTH_NONCE_HEADER
            })
        ));
    }

    #[test]
    fn record_nonce_ok_first_replayed_second_same_domain() {
        let mut state = FetchAuthState::new();
        let t = now();
        state.record_nonce("hosta.test", "n1", t).unwrap();
        let result = state.record_nonce("hosta.test", "n1", t);
        assert!(matches!(result, Err(FetchAuthReject::ReplayedNonce)));
    }

    #[test]
    fn record_nonce_same_nonce_different_domain_both_ok() {
        let mut state = FetchAuthState::new();
        let t = now();
        state.record_nonce("hosta.test", "n1", t).unwrap();
        state
            .record_nonce("hostb.test", "n1", t)
            .expect("same nonce under a different domain must not collide");
    }

    #[test]
    fn nonce_flood_evicts_oldest_tie_broken_lexically_leaves_other_domain_intact() {
        let mut state = FetchAuthState::new();
        let t0 = now();
        // Every initial entry shares the SAME recorded instant `t0` —
        // deliberately, so filling the domain to the cap cannot also
        // trigger `record_nonce`'s own TTL sweep (which runs on every
        // call): 4096 entries spread one second apart would span more
        // than `FETCH_NONCE_TTL_SECS`, sweeping the earliest ones before
        // the cap is ever reached. Sharing one instant isolates the CAP
        // eviction behavior under test from the TTL sweep, and as a
        // side effect exercises the tie-break rule directly, since every
        // entry now ties on timestamp.
        for i in 0..FETCH_NONCE_MAX_PER_DOMAIN {
            let nonce = format!("n{i:06}");
            state.record_nonce("hosta.test", &nonce, t0).unwrap();
        }
        state.record_nonce("hostb.test", "untouched", t0).unwrap();
        assert_eq!(
            state.nonce_len_for("hosta.test"),
            FETCH_NONCE_MAX_PER_DOMAIN
        );

        let overflow_ts = t0;
        state
            .record_nonce("hosta.test", "overflow", overflow_ts)
            .unwrap();
        assert_eq!(
            state.nonce_len_for("hosta.test"),
            FETCH_NONCE_MAX_PER_DOMAIN
        );
        assert!(
            state
                .record_nonce("hosta.test", "n000000", overflow_ts)
                .is_ok(),
            "the oldest nonce ('n000000') must have been evicted, so it is not a replay"
        );
        assert_eq!(
            state.nonce_len_for("hostb.test"),
            1,
            "a different domain's entries must be untouched"
        );
    }

    #[test]
    fn admit_allows_exactly_max_then_rejects_then_resets_after_window() {
        let mut state = FetchAuthState::new();
        let t0 = now();
        for _ in 0..FETCH_RATE_MAX_PER_WINDOW {
            state.admit("hosta.test", t0).unwrap();
        }
        assert!(matches!(
            state.admit("hosta.test", t0),
            Err(FetchAuthReject::RateLimited)
        ));

        let after_window = t0 + time::Duration::seconds(FETCH_RATE_WINDOW_SECS);
        state
            .admit("hosta.test", after_window)
            .expect("rate limit must reset after the window elapses");
    }

    #[test]
    fn fetch_bounds_ok_true_for_shipped_constants() {
        assert!(fetch_bounds_ok(
            FETCH_FRESHNESS_WINDOW_SECS,
            FETCH_NONCE_TTL_SECS,
            FETCH_NONCE_MAX_PER_DOMAIN,
            FETCH_RATE_MAX_PER_WINDOW,
            FETCH_RATE_WINDOW_SECS,
        ));
    }

    #[test]
    fn fetch_bounds_ok_false_when_nonce_ttl_below_twice_freshness() {
        assert!(!fetch_bounds_ok(
            300,
            599, // one below 2 * 300
            FETCH_NONCE_MAX_PER_DOMAIN,
            FETCH_RATE_MAX_PER_WINDOW,
            FETCH_RATE_WINDOW_SECS,
        ));
    }

    #[test]
    fn fetch_bounds_ok_false_when_cap_below_ttl_times_admitted_rate() {
        // ttl(600) * rate(120) / window(60) = 1200; one below that fails.
        assert!(!fetch_bounds_ok(300, 600, 1199, 120, 60));
        assert!(fetch_bounds_ok(300, 600, 1200, 120, 60));
    }

    #[test]
    fn fetch_bounds_ok_false_on_zero_rate_window() {
        assert!(!fetch_bounds_ok(
            300,
            600,
            FETCH_NONCE_MAX_PER_DOMAIN,
            120,
            0
        ));
    }
}
