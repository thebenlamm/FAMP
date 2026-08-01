//! Pre-verify ingress guard chain: cheap, stateful checks that run BEFORE
//! [`crate::verify::verify_inbound_any`] (INGR-01, INGR-05; D-05, D-09).
//!
//! Three things this module promises that [`crate::verify`] does not:
//!
//! 1. Every field this module reads is PEEKED, not verified.
//!    [`peek_guard_fields`] performs a raw, duplicate-key-rejecting parse
//!    of the wire bytes — cryptographically UNVERIFIED — and every
//!    downstream check in this module (starting with [`freshness_check`])
//!    acts on those peeked values BEFORE
//!    [`crate::verify::verify_inbound_any`] ever confirms a signature. A
//!    caller must never treat [`PeekedFields`] as trusted.
//! 2. Unlike `verify.rs` (documented "Pure, transport-agnostic" with no
//!    owned state), this module owns mutable state: [`IngressGuard`] is
//!    the process-lifetime anchor plan 02 extends with a replay cache and
//!    plan 03 extends with rate-limit buckets. That owned state is the
//!    reason this lives in its own module rather than as an addition to
//!    `verify.rs`'s stated purity contract.
//! 3. INGR-05's ordering guarantee: cheap gates (this module, via
//!    [`run_cheap_gates`]) precede signature verification
//!    (`verify_inbound_any`), and signature verification precedes ANY
//!    registry/local-bus mutation. Getting this backwards turns signature
//!    verification itself into the DoS amplifier (D-09) — a cheap-to-send
//!    request must never be able to force an expensive Ed25519 verify.
//!
//! [`peek_guard_fields`] is a gateway-local raw parse (D-13/D-15): it does
//! NOT add an accessor to `famp-envelope`'s frozen `EnvelopeView`, which
//! exposes only `from`/`to`/`class`/`body`/`task_id` and has no `ts`/
//! `nonce`/`expiry` accessor. Duplicating the parse here, scoped to this
//! crate, keeps `famp-envelope` byte-identical for the milestone (D-15).

use std::str::FromStr;

use famp::Principal;
use serde_json::{Map, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// INGR-01's clock-skew window, in seconds.
///
/// An envelope `ts` more than this many seconds from this gateway's own
/// wall clock — in EITHER direction — is rejected. Enforced symmetrically
/// (past and future) and INCLUSIVE at the boundary (exactly this many
/// seconds is still accepted; one second beyond is not).
///
/// This is a DIFFERENT knob from `egress.rs`'s `EXPIRY_WINDOW_MINUTES`:
/// that constant only formats an outbound, format-validated-only
/// `expiry` field and enforces nothing on receipt. `CLOCK_SKEW_WINDOW_SECS`
/// is the one INGR-01 window this gateway actually enforces against every
/// inbound envelope.
pub const CLOCK_SKEW_WINDOW_SECS: i64 = 300;

/// Rejections this module's cheap, pre-verify checks can produce.
///
/// Extended by plans 02 (replay) and 03 (audience/rate-limit) — this plan
/// defines exactly the two variants [`freshness_check`] and
/// [`peek_guard_fields`] need.
#[derive(Debug, thiserror::Error)]
pub enum GuardReject {
    /// The pre-verify raw peek of `from`/`to`/`ts` failed: malformed
    /// JSON, a duplicate object key (rejected by the same strict-parse
    /// contract `famp_envelope::peek_sender` uses), or a missing/
    /// non-string `from`/`to`/`ts`/`nonce`/`expiry` field. Never implies
    /// anything about whether a valid signature is present — this is a
    /// structural check, not a trust decision, so a perfectly-signed
    /// envelope with a malformed shape at this layer maps here too.
    /// Carries no data: the shape itself is malformed, so there is
    /// nothing safe to echo back.
    #[error(
        "envelope failed pre-verify shape check (malformed JSON, duplicate key, or missing/invalid from/to/ts)"
    )]
    BadEnvelopeShape,

    /// INGR-01: the envelope's peeked `ts` differs from this gateway's
    /// own wall-clock `now` by more than `skew_secs` seconds, in either
    /// direction. Does NOT imply the envelope is unsigned, forged, or
    /// otherwise invalid — a correctly-signed envelope with a stale `ts`
    /// maps here too (that is the entire point: this check runs BEFORE
    /// signature verification, per INGR-05/D-09). Carries only the two
    /// timestamps and the configured window size — never key bytes,
    /// signature material, or envelope body content.
    #[error("envelope timestamp {ts} is outside the {skew_secs}s clock-skew window of gateway now {now}")]
    StaleTimestamp {
        ts: String,
        now: String,
        skew_secs: i64,
    },

    /// INGR-02/D-06: a nonce previously recorded for the same peeked
    /// sender, within `REPLAY_CACHE_TTL_SECS` seconds, was submitted
    /// again. Does NOT imply the second submission is unsigned or
    /// forged — a correctly-signed replay of an already-accepted
    /// envelope maps here too (that is the entire point: this check
    /// runs BEFORE signature verification, per INGR-05/D-09, keyed on
    /// the peeked, not yet cryptographically confirmed, identity).
    /// Carries the nonce because a nonce is a public routing token,
    /// never a secret — this variant must never be extended to carry
    /// signature or key material.
    #[error("nonce '{nonce}' from sender '{principal}' was already seen within the replay window")]
    ReplayedNonce { principal: String, nonce: String },

    /// INGR-02: the peeked `nonce` field is absent or empty. Does NOT
    /// imply anything about the envelope's signature — a correctly
    /// signed envelope with no nonce maps here too. The replay cache is
    /// never asked to key on nothing: this rejection happens before
    /// [`ReplayCache::record`] is ever called with an empty key. Carries
    /// only the peeked sender name, never key bytes or signature
    /// material.
    #[error("sender '{principal}' submitted an envelope with an absent or empty nonce")]
    MissingNonce { principal: String },

    /// INGR-04/D-08/D-22: this gateway's configured `own_domain` does not
    /// match the peeked `to` authority. Does NOT imply anything about the
    /// envelope's signature — a correctly-signed envelope addressed to a
    /// domain this gateway does not own maps here too, by design (this
    /// runs BEFORE `verify_inbound_any`, per INGR-05/D-09). Only produced
    /// when `own_domain` is configured; an unset `own_domain` skips the
    /// domain half of [`audience_check`] entirely (D-22: audience binding
    /// here is DOMAIN-granularity only, never per-recipient — see this
    /// module's `audience_check` doc). Carries both the expected and the
    /// got authority — neither is secret, both are already public in the
    /// envelope's own `to` field.
    #[error("this gateway is not authoritative for domain '{got}' (configured own-domain: '{expected}')")]
    ForeignDomain { expected: String, got: String },

    /// INGR-04/D-08/D-22: the peeked `from` name is not in this gateway's
    /// `--backs` set (`GatewayRegistry`). Does NOT imply the envelope is
    /// unsigned or forged — a correctly-signed envelope from a principal
    /// this gateway simply does not proxy maps here too, by design (runs
    /// BEFORE `verify_inbound_any`, per INGR-05/D-09). Carries only the
    /// peeked sender name — never key bytes or signature material.
    #[error("sender principal '{principal}' is not backed by this gateway")]
    SenderNotBacked { principal: String },

    /// INGR-06/D-10: `sender` has exceeded `limit` admitted requests
    /// within the current `window_secs`-second fixed window. Does NOT
    /// imply anything about the envelope's signature, freshness, or
    /// replay status — a perfectly valid, first-ever-seen envelope from a
    /// sender who has simply exhausted their budget maps here too, by
    /// design (runs BEFORE `verify_inbound_any`, per INGR-05/D-09, and
    /// LAST among the cheap gates — see [`run_cheap_gates`]'s ordering
    /// comment for why). Carries the principal, the limit, and the window
    /// length so an operator can read the applied policy directly off the
    /// reject — never key bytes or signature material.
    #[error("sender '{principal}' exceeded {limit} requests per {window_secs}s")]
    RateLimited {
        principal: String,
        limit: u32,
        window_secs: i64,
    },
}

/// Fields peeked from unverified inbound bytes, cheaply, before any
/// signature check runs.
///
/// **Every field here is cryptographically UNVERIFIED.** A caller must
/// never treat `PeekedFields.from` (or any other field) as the confirmed
/// sender — that confirmation only exists after
/// [`crate::verify::verify_inbound_any`] returns `Ok`. Name variables
/// derived from this type accordingly (e.g. `peeked_from`, never just
/// `from`, at any call site that also has a verified value in scope).
#[derive(Debug)]
pub struct PeekedFields {
    pub from: Principal,
    pub to: Principal,
    pub ts: String,
    pub nonce: Option<String>,
    pub expiry: Option<String>,
}

/// Peek `from`/`to`/`ts`/`nonce`/`expiry` out of raw, unverified inbound
/// bytes.
///
/// D-13/D-15: this is a deliberate, gateway-local duplicate of a strict
/// parse — NOT an extension of `famp-envelope`'s frozen `EnvelopeView`,
/// which exposes only `from`/`to`/`class`/`body`/`task_id` and has no
/// `ts`/`nonce`/`expiry` accessor. Extending the frozen crate (even with
/// a read-only accessor) would violate D-15; this duplication is the
/// cost of keeping that crate byte-identical. Uses
/// `famp::from_slice_strict` (`famp-canonical`'s duplicate-key-rejecting
/// parse, the same one `famp_envelope::peek_sender` builds on) so a
/// duplicate `from` key is rejected here exactly as it would be
/// post-verify.
pub fn peek_guard_fields(bytes: &[u8]) -> Result<PeekedFields, GuardReject> {
    let value: Value = famp::from_slice_strict(bytes).map_err(|_| GuardReject::BadEnvelopeShape)?;
    let obj: &Map<String, Value> = value.as_object().ok_or(GuardReject::BadEnvelopeShape)?;

    let from = required_principal(obj, "from")?;
    let to = required_principal(obj, "to")?;
    let ts = required_string(obj, "ts")?;
    let nonce = optional_string(obj, "nonce")?;
    let expiry = optional_string(obj, "expiry")?;

    Ok(PeekedFields {
        from,
        to,
        ts,
        nonce,
        expiry,
    })
}

fn required_string(obj: &Map<String, Value>, field: &'static str) -> Result<String, GuardReject> {
    obj.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(GuardReject::BadEnvelopeShape)
}

fn required_principal(
    obj: &Map<String, Value>,
    field: &'static str,
) -> Result<Principal, GuardReject> {
    let raw = required_string(obj, field)?;
    Principal::from_str(&raw).map_err(|_| GuardReject::BadEnvelopeShape)
}

fn optional_string(
    obj: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, GuardReject> {
    match obj.get(field) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(GuardReject::BadEnvelopeShape),
    }
}

/// INGR-01/D-05: reject an envelope whose peeked `ts` is more than
/// `skew_secs` seconds from `now`, in either direction, with the
/// boundary itself (exactly `skew_secs`) still accepted.
///
/// Both `ts` and `now` are parsed with `time::OffsetDateTime::parse`
/// against RFC 3339 — NEVER compared lexically as strings. REL-02
/// (`federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders`,
/// `famp-envelope/src/envelope.rs`) is the precedent this guards against:
/// a non-canonical-but-valid RFC 3339 offset form can lexically misorder
/// against a later UTC instant, so only a parsed-instant comparison is
/// trustworthy here.
///
/// A parse failure on `ts` maps to `BadEnvelopeShape` (the envelope
/// itself is malformed). A parse failure on `now` ALSO maps to
/// `BadEnvelopeShape` — fail closed — even though `now` is expected to
/// always be a `crate::clock::now_canonical_utc()` output and so cannot
/// legitimately fail; there is no reject variant more honest than
/// "something about this check's inputs is malformed."
pub fn freshness_check(ts: &str, now: &str, skew_secs: i64) -> Result<(), GuardReject> {
    let ts_instant =
        OffsetDateTime::parse(ts, &Rfc3339).map_err(|_| GuardReject::BadEnvelopeShape)?;
    let now_instant =
        OffsetDateTime::parse(now, &Rfc3339).map_err(|_| GuardReject::BadEnvelopeShape)?;

    let diff_secs = (now_instant - ts_instant).whole_seconds().abs();
    if diff_secs <= skew_secs {
        Ok(())
    } else {
        Err(GuardReject::StaleTimestamp {
            ts: ts.to_string(),
            now: now.to_string(),
            skew_secs,
        })
    }
}

/// INGR-02's replay-cache entry lifetime, in seconds.
///
/// Colocated with [`REPLAY_CACHE_MAX_PER_SENDER`] and [`ReplayCache`]
/// itself (the way `egress.rs`'s `EXPIRY_WINDOW_MINUTES` sits directly
/// above its consumer): all three knobs plus `CLOCK_SKEW_WINDOW_SECS`
/// must jointly satisfy [`replay_bounds_ok`], and drifting any one of
/// them without checking the others is exactly the silent-breakage
/// scenario D-06 exists to prevent.
pub const REPLAY_CACHE_TTL_SECS: i64 = 600;

/// INGR-02/INGR-08's per-sender replay-cache capacity.
///
/// This bounds memory per sender (T-17-07): once a single sender's
/// cache reaches this many live entries, [`ReplayCache::record`] evicts
/// the oldest before inserting. See [`replay_bounds_ok`] for the sizing
/// relation this constant must satisfy relative to
/// [`REPLAY_CACHE_TTL_SECS`] and the rate-limit constants plan 03 adds.
pub const REPLAY_CACHE_MAX_PER_SENDER: usize = 4096;

/// INGR-02/D-06: the TTL/skew/size relationship, stated as one
/// machine-checked inequality rather than a comment.
///
/// Returns `true` only when BOTH relations hold:
///
/// - **Relation (a):** `ttl_secs >= 2 * skew_secs`. An envelope with a
///   fixed `ts` is admissible over the whole wall-clock interval running
///   from `ts` minus the skew window to `ts` plus the skew window — a
///   span of twice the skew window. A cache entry that expires sooner
///   than that span would let a genuine replay slip through after
///   eviction but before the freshness gate closes, which defeats
///   tracking it at all.
/// - **Relation (b):** `max_per_sender >= ttl_secs * rate_max /
///   rate_window_secs`, i.e. the per-sender cap must be at least the
///   number of distinct nonces one sender could legitimately emit
///   within one TTL at the maximum admitted rate. If the cap is smaller,
///   the cache starts evicting live entries early under load, reopening
///   a replay window exactly when traffic is highest.
///
/// `rate_max`/`rate_window_secs` correspond to plan 03's
/// `RATE_LIMIT_MAX_PER_WINDOW`/`RATE_LIMIT_WINDOW_SECS`, which do not
/// exist yet this plan — callers (this module's own tests) pass them as
/// literals matching plan 03's planned shipped values (120 per 60s)
/// until plan 03 lands those constants for real.
///
/// Guards against a zero (or negative) `rate_window_secs` by returning
/// `false` rather than dividing by zero.
#[must_use]
pub const fn replay_bounds_ok(
    skew_secs: i64,
    ttl_secs: i64,
    max_per_sender: usize,
    rate_max: u32,
    rate_window_secs: i64,
) -> bool {
    if rate_window_secs <= 0 {
        return false;
    }
    let relation_a = ttl_secs >= 2 * skew_secs;
    let max_nonces_within_ttl = (ttl_secs as i128 * rate_max as i128) / rate_window_secs as i128;
    let relation_b = max_per_sender as i128 >= max_nonces_within_ttl;
    relation_a && relation_b
}

/// INGR-02/INGR-03/INGR-08: bounded, per-sender, in-memory nonce replay
/// cache.
///
/// The OUTER key is the peeked sender name (INGR-08/D-12 per-sender
/// scoping); the INNER key is the nonce. This outer key is load-bearing:
/// it is what makes one peer structurally unable to evict or collide
/// with another peer's entries. A flattened single-map design (nonce ->
/// instant, with no sender dimension) would silently reintroduce exactly
/// the cross-tenant failure INGR-08 exists to prevent — do not collapse
/// these two levels.
///
/// **INGR-03/D-07 durability decision:** this cache is IN-MEMORY ONLY
/// and is lost on process restart. The restart-reopened replay window is
/// nevertheless BOUNDED, at most `2 * CLOCK_SKEW_WINDOW_SECS` seconds,
/// because the INGR-01 freshness gate ([`freshness_check`]) rejects any
/// envelope outside that band regardless of what this cache remembers —
/// see `restart_reopened_window_is_bounded_by_the_freshness_check` in
/// this module's tests for the executable form of that bound. The
/// honest tradeoff (D-07/D-20): this buys zero new on-disk gateway state
/// (the gateway's first ever would be a materially larger lift) at the
/// cost of a bounded window in which a replay captured just before a
/// restart can be re-accepted just after one.
pub struct ReplayCache {
    entries: std::collections::HashMap<String, std::collections::HashMap<String, OffsetDateTime>>,
}

impl ReplayCache {
    /// An empty cache, as constructed on gateway startup (and therefore
    /// on every restart — see the INGR-03 doc above).
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// Record a `(sender, nonce)` sighting at `now`, rejecting a replay.
    ///
    /// - An empty `nonce` is rejected with [`GuardReject::MissingNonce`]
    ///   before the map is touched at all.
    /// - This sender's inner map is lazily swept first, dropping entries
    ///   whose age is at least `REPLAY_CACHE_TTL_SECS`.
    /// - If the nonce is still present after the sweep, this is a
    ///   genuine replay: [`GuardReject::ReplayedNonce`].
    /// - If the sender's inner map is at [`REPLAY_CACHE_MAX_PER_SENDER`]
    ///   after the sweep, the OLDEST entry by recorded instant is
    ///   evicted first, breaking a tie on identical instants by the
    ///   lexicographically smaller nonce — this tie rule is a tested
    ///   contract (`replay_cache_eviction_is_oldest_first_and_deterministic_on_ties`),
    ///   not an implementation detail free to vary with `HashMap`
    ///   iteration order.
    /// - Otherwise the sighting is inserted and this returns `Ok`.
    pub fn record(
        &mut self,
        sender: &str,
        nonce: &str,
        now: OffsetDateTime,
    ) -> Result<(), GuardReject> {
        if nonce.is_empty() {
            return Err(GuardReject::MissingNonce {
                principal: sender.to_string(),
            });
        }

        let inner = self.entries.entry(sender.to_string()).or_default();
        inner.retain(|_, recorded_at| (now - *recorded_at).whole_seconds() < REPLAY_CACHE_TTL_SECS);

        if inner.contains_key(nonce) {
            return Err(GuardReject::ReplayedNonce {
                principal: sender.to_string(),
                nonce: nonce.to_string(),
            });
        }

        if inner.len() >= REPLAY_CACHE_MAX_PER_SENDER {
            if let Some(oldest) = inner
                .iter()
                .min_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(b.0)))
                .map(|(k, _)| k.clone())
            {
                inner.remove(&oldest);
            }
        }

        inner.insert(nonce.to_string(), now);
        Ok(())
    }

    /// Sweep every sender's inner map of entries older than
    /// `REPLAY_CACHE_TTL_SECS`, and drop any sender whose inner map is
    /// left empty — so a churn of one-shot senders cannot grow the outer
    /// map without bound.
    pub fn sweep(&mut self, now: OffsetDateTime) {
        self.entries.retain(|_, inner| {
            inner.retain(|_, recorded_at| {
                (now - *recorded_at).whole_seconds() < REPLAY_CACHE_TTL_SECS
            });
            !inner.is_empty()
        });
    }

    /// The number of live entries recorded for `sender`. Test-only
    /// visibility need, `pub` for use from this module's `#[cfg(test)]`
    /// submodule.
    #[must_use]
    pub fn len_for(&self, sender: &str) -> usize {
        self.entries
            .get(sender)
            .map_or(0, std::collections::HashMap::len)
    }
}

impl Default for ReplayCache {
    fn default() -> Self {
        Self::new()
    }
}

/// INGR-06's fixed-window rate-limit cap: the maximum number of requests
/// one sender may have admitted within any single `RATE_LIMIT_WINDOW_SECS`
/// window.
///
/// These two constants are the `rate_max`/`rate_window_secs` arguments
/// [`replay_bounds_ok`] takes — see [`SHIPPED_BOUNDS_OK`] below, which
/// binds this constant, [`RATE_LIMIT_WINDOW_SECS`], [`CLOCK_SKEW_WINDOW_SECS`],
/// [`REPLAY_CACHE_TTL_SECS`], and [`REPLAY_CACHE_MAX_PER_SENDER`] together
/// at COMPILE time, so retuning this value alone is checked against the
/// replay cache's size bound rather than drifting silently.
pub const RATE_LIMIT_MAX_PER_WINDOW: u32 = 120;

/// INGR-06's fixed-window length, in seconds. See
/// [`RATE_LIMIT_MAX_PER_WINDOW`]'s doc comment for the compile-time
/// binding this participates in.
pub const RATE_LIMIT_WINDOW_SECS: i64 = 60;

/// The D-06 TTL/skew/size/rate relationship, evaluated once against the
/// five constants this crate actually ships, and asserted below at COMPILE
/// time — not merely by a runtime test. Binding all five constants
/// together in one place means editing any single knob without checking
/// the others fails the BUILD, not just `cargo test`.
const SHIPPED_BOUNDS_OK: bool = replay_bounds_ok(
    CLOCK_SKEW_WINDOW_SECS,
    REPLAY_CACHE_TTL_SECS,
    REPLAY_CACHE_MAX_PER_SENDER,
    RATE_LIMIT_MAX_PER_WINDOW,
    RATE_LIMIT_WINDOW_SECS,
);
// `clippy::assertions_on_constants` fires here because the condition is a
// `const`, known at compile time — that is the ENTIRE POINT of this
// assertion (the standard Rust const-assertion idiom: a violating retune
// of any of the five bound constants makes `SHIPPED_BOUNDS_OK` resolve to
// `false`, which makes THIS line fail to compile). Suppressed narrowly,
// on this one line, not the module.
#[allow(clippy::assertions_on_constants)]
const _: () = assert!(SHIPPED_BOUNDS_OK);

/// INGR-06/INGR-08: fixed-window rate limiter, keyed on the
/// backing-confirmed peeked sender principal name.
///
/// **Why a fixed window, not a sliding one:** O(1) memory per sender over
/// a bounded key space (the operator-configured `--backs` set), at the
/// cost of admitting up to double the nominal rate across a window
/// boundary in the worst case (a burst just before a window closes plus a
/// burst just after it opens). That imprecision is at most one extra
/// window's budget, which is proportionate for a relay serving a small,
/// fixed, operator-configured peer set — not the general internet.
///
/// **Why the key is what it is (INGR-06/D-10) — three things:**
///
/// 1. The key is the peeked sender principal name AFTER
///    [`audience_check`] has confirmed it is in this gateway's `--backs`
///    set (`run_cheap_gates` runs audience binding before this gate — see
///    its ordering comment). The key space is therefore fixed at process
///    startup and enumerable, never attacker-mintable: an attacker cannot
///    manufacture a new bucket merely by claiming a new name.
/// 2. Source IP is deliberately rejected as a key: it is attacker-
///    rotatable and NAT-shared, and this gateway's TLS is channel
///    encryption only (D-08), giving no authenticated per-connection
///    identity to key on besides the envelope's claimed `from`.
/// 3. Residual risk (17-RESEARCH.md Pitfall 3, T-17-15), stated honestly
///    rather than hidden: an attacker holding no valid key can still burn
///    a real backed peer's rate-limit budget by CLAIMING their name in
///    the peeked `from` field, because INGR-05's ordering requires this
///    gate to run before signature verification ever proves the claim
///    false. This risk is bounded — by the small, fixed, operator-
///    configured backed set — and ACCEPTED, not closed. If spurious
///    rate-limit rejects are observed during REACH-04 acceptance, that
///    should be investigated as this exact risk materializing BEFORE the
///    threshold is loosened, not treated as a simple false-positive bug.
pub struct RateLimiter {
    buckets: std::collections::HashMap<String, (OffsetDateTime, u32)>,
}

impl RateLimiter {
    /// An empty limiter, as constructed on gateway startup.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: std::collections::HashMap::new(),
        }
    }

    /// Admit (or reject) one request from `sender` at `now`.
    ///
    /// A sender with no existing bucket, or whose bucket's window start is
    /// at least `RATE_LIMIT_WINDOW_SECS` behind `now`, gets a fresh window
    /// (count reset to 1) and `Ok`. Otherwise the count is compared
    /// against [`RATE_LIMIT_MAX_PER_WINDOW`]: at or below the limit,
    /// incremented and `Ok`; above it, [`GuardReject::RateLimited`].
    pub fn admit(&mut self, sender: &str, now: OffsetDateTime) -> Result<(), GuardReject> {
        let bucket = self
            .buckets
            .entry(sender.to_string())
            .or_insert((now, 0_u32));

        if (now - bucket.0).whole_seconds() >= RATE_LIMIT_WINDOW_SECS {
            *bucket = (now, 1);
            return Ok(());
        }

        if bucket.1 < RATE_LIMIT_MAX_PER_WINDOW {
            bucket.1 += 1;
            Ok(())
        } else {
            Err(GuardReject::RateLimited {
                principal: sender.to_string(),
                limit: RATE_LIMIT_MAX_PER_WINDOW,
                window_secs: RATE_LIMIT_WINDOW_SECS,
            })
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-lifetime pre-verify guard state.
///
/// `started_at` is the anchor for INGR-03's restart-reopened replay-window
/// documentation (a fresh `IngressGuard` — and therefore a fresh, empty
/// `replay` cache — is exactly what a gateway restart produces).
/// Constructed once per gateway process (`build_gateway_router`) and
/// threaded through every inbound request via `GatewayIngressState`.
pub struct IngressGuard {
    started_at: OffsetDateTime,
    replay: ReplayCache,
    rate: RateLimiter,
}

impl IngressGuard {
    /// Capture the current instant as this guard's `started_at`, with a
    /// fresh, empty [`ReplayCache`] and [`RateLimiter`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: OffsetDateTime::now_utc(),
            replay: ReplayCache::new(),
            rate: RateLimiter::new(),
        }
    }

    /// The instant this guard (and therefore this gateway process's
    /// pre-verify guard state) was constructed.
    #[must_use]
    pub const fn started_at(&self) -> OffsetDateTime {
        self.started_at
    }
}

impl Default for IngressGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Input to [`run_cheap_gates`]: the peeked (unverified) fields plus the
/// process-level context every cheap gate needs.
///
/// `own_domain` and `sender_is_backed` are read by [`audience_check`],
/// the first gate [`run_cheap_gates`] runs (INGR-04). The whole-request
/// context a gate chain needs is assembled once, at the call site, rather
/// than re-derived per gate.
pub struct GuardInput<'a> {
    pub peeked: &'a PeekedFields,
    pub now: &'a str,
    pub own_domain: Option<&'a str>,
    pub sender_is_backed: bool,
}

/// INGR-04/D-08 audience binding: checked in order, `own_domain` first
/// (when configured) then `sender_is_backed`.
///
/// **D-22 limitation, stated here explicitly because this is the one
/// function that enforces it:** this checks the recipient DOMAIN
/// (`peeked.to.authority()` against `own_domain`) and the SENDER's
/// backing (`sender_is_backed`) — it does NOT check whether `to` is
/// actually a principal this gateway serves. `GatewayRegistry` (see
/// `registry.rs`) has exactly one meaning of "backs": the fixed,
/// pre-configured set of REMOTE principal names a `famp-gateway` process
/// was launched to proxy via `--backs`. It has never tracked local
/// recipients, so a per-recipient check is not available to add here
/// without a different, larger change (tracked as INGR-09, out of this
/// plan's scope). Audience binding delivered by this function is
/// DOMAIN-granularity only.
///
/// When `input.own_domain` is `None`, the domain half is skipped
/// entirely — the backing half still runs unconditionally.
pub fn audience_check(input: &GuardInput<'_>) -> Result<(), GuardReject> {
    if let Some(expected) = input.own_domain {
        let got = input.peeked.to.authority();
        if got != expected {
            return Err(GuardReject::ForeignDomain {
                expected: expected.to_string(),
                got: got.to_string(),
            });
        }
    }
    if !input.sender_is_backed {
        return Err(GuardReject::SenderNotBacked {
            principal: input.peeked.from.name().to_string(),
        });
    }
    Ok(())
}

/// The single ordered call site every cheap gate is added to.
///
/// Final order (this plan completes it): [`audience_check`], then
/// [`freshness_check`], then the replay check ([`ReplayCache::record`] on
/// `guard.replay`), then the rate limit ([`RateLimiter::admit`] on
/// `guard.rate`) — matching the order 17-CONTEXT.md's D-08/D-05/D-06/D-10
/// discuss them. Returns on the FIRST reject; later gates never run once
/// an earlier one has failed (INGR-05: cheap-before-expensive applies
/// within this chain too, not just relative to signature verification).
///
/// **Rate limiting runs LAST among the cheap gates on purpose.** An
/// envelope that is foreign-domain, from an unbacked sender, stale, or a
/// replay should be rejected for THAT reason rather than consuming a
/// rate-limit slot — only traffic that is otherwise admissible should
/// count against a peer's budget. Placing the rate check first would let
/// a flood of malformed/foreign/stale junk exhaust a legitimate peer's
/// budget before any of those cheaper, more specific rejects even had a
/// chance to fire.
///
/// Replay and rate-limit both key on the PEEKED sender name
/// (`input.peeked.from`), not a verified identity — consistent with every
/// other gate in this chain running before `verify_inbound_any` ever
/// confirms a signature. By the time either gate runs, [`audience_check`]
/// has already confirmed the peeked sender is backed (see
/// [`RateLimiter`]'s doc comment for why that ordering is exactly what
/// makes the rate-limit key non-attacker-mintable).
pub fn run_cheap_gates(
    input: &GuardInput<'_>,
    guard: &mut IngressGuard,
) -> Result<(), GuardReject> {
    audience_check(input)?;

    freshness_check(input.peeked.ts.as_str(), input.now, CLOCK_SKEW_WINDOW_SECS)?;

    let nonce = input
        .peeked
        .nonce
        .as_deref()
        .ok_or_else(|| GuardReject::MissingNonce {
            principal: input.peeked.from.name().to_string(),
        })?;

    let parsed_now =
        OffsetDateTime::parse(input.now, &Rfc3339).map_err(|_| GuardReject::BadEnvelopeShape)?;

    guard
        .replay
        .record(input.peeked.from.name(), nonce, parsed_now)?;

    guard.rate.admit(input.peeked.from.name(), parsed_now)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
mod tests {
    use super::*;

    fn to_canonical_rfc3339(instant: OffsetDateTime) -> String {
        crate::clock::strip_subseconds(&instant.format(&Rfc3339).unwrap())
    }

    #[test]
    fn freshness_check_ts_equal_now_is_ok() {
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        assert!(freshness_check(&now, &now, CLOCK_SKEW_WINDOW_SECS).is_ok());
    }

    #[test]
    fn freshness_check_boundary_is_inclusive_in_both_directions() {
        // Every timestamp below is derived from the SAME `base` instant so
        // the deltas are exact, whole-second differences with zero clock
        // drift between calls (no repeated `OffsetDateTime::now_utc()`
        // calls that could straddle a second boundary).
        let base = OffsetDateTime::now_utc();
        let now = to_canonical_rfc3339(base);

        let past_boundary =
            to_canonical_rfc3339(base - time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS));
        assert!(
            freshness_check(&past_boundary, &now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "exactly -{CLOCK_SKEW_WINDOW_SECS}s must be accepted (inclusive lower bound)"
        );

        let future_boundary =
            to_canonical_rfc3339(base + time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS));
        assert!(
            freshness_check(&future_boundary, &now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "exactly +{CLOCK_SKEW_WINDOW_SECS}s must be accepted (inclusive upper bound)"
        );

        let past_beyond =
            to_canonical_rfc3339(base - time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS + 1));
        assert!(
            matches!(
                freshness_check(&past_beyond, &now, CLOCK_SKEW_WINDOW_SECS),
                Err(GuardReject::StaleTimestamp { .. })
            ),
            "one second beyond the past boundary must be rejected"
        );

        let future_beyond =
            to_canonical_rfc3339(base + time::Duration::seconds(CLOCK_SKEW_WINDOW_SECS + 1));
        assert!(
            matches!(
                freshness_check(&future_beyond, &now, CLOCK_SKEW_WINDOW_SECS),
                Err(GuardReject::StaleTimestamp { .. })
            ),
            "one second beyond the future boundary must be rejected"
        );
    }

    #[test]
    fn freshness_check_rejects_unparseable_ts_as_bad_envelope_shape() {
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        let result = freshness_check("not-a-timestamp", &now, CLOCK_SKEW_WINDOW_SECS);
        assert!(matches!(result, Err(GuardReject::BadEnvelopeShape)));
    }

    /// REL-02 precedent
    /// (`federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders`,
    /// `famp-envelope/src/envelope.rs`): a non-canonical, valid RFC 3339
    /// offset form can represent an instant that a naive string/date
    /// comparison would misjudge. Here `ts`
    /// ("2026-07-26T23:00:00-01:00", i.e. local 23:00 one hour BEHIND
    /// UTC) is the SAME UTC instant as `now` ("2026-07-27T00:00:00Z") —
    /// diff 0 — but its leading date component ("...26T23...") lexically
    /// sorts as an entire calendar day earlier than `now`'s
    /// ("...27T00..."). Only a parsed-instant comparison gets this right.
    #[test]
    fn freshness_check_uses_parsed_instant_not_lexical_order() {
        let now = "2026-07-27T00:00:00Z";
        let ts = "2026-07-26T23:00:00-01:00";
        assert!(
            ts < now,
            "the offset form must actually lexically sort before `now` for this to be a real test"
        );
        assert!(
            freshness_check(ts, now, CLOCK_SKEW_WINDOW_SECS).is_ok(),
            "ts and now are the SAME UTC instant once parsed -- must not be treated as \
             stale merely because the raw strings misorder"
        );
    }

    #[test]
    fn peek_guard_fields_rejects_duplicate_from_key() {
        let bytes = br#"{"from":"agent:local/alice","from":"agent:local/eve","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z"}"#;
        let result = peek_guard_fields(bytes);
        assert!(matches!(result, Err(GuardReject::BadEnvelopeShape)));
    }

    #[test]
    fn peek_guard_fields_extracts_all_fields_on_well_formed_envelope() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z","nonce":"abc123","expiry":"2026-07-27T00:05:00Z"}"#;
        let peeked = peek_guard_fields(bytes).expect("well-formed envelope must peek");
        assert_eq!(peeked.from.to_string(), "agent:local/alice");
        assert_eq!(peeked.to.to_string(), "agent:local/bob");
        assert_eq!(peeked.ts, "2026-07-27T00:00:00Z");
        assert_eq!(peeked.nonce.as_deref(), Some("abc123"));
        assert_eq!(peeked.expiry.as_deref(), Some("2026-07-27T00:05:00Z"));
    }

    #[test]
    fn peek_guard_fields_nonce_and_expiry_absent_are_none() {
        let bytes =
            br#"{"from":"agent:local/alice","to":"agent:local/bob","ts":"2026-07-27T00:00:00Z"}"#;
        let peeked =
            peek_guard_fields(bytes).expect("envelope without nonce/expiry must still peek");
        assert!(peeked.nonce.is_none());
        assert!(peeked.expiry.is_none());
    }

    #[test]
    fn peek_guard_fields_rejects_missing_ts() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob"}"#;
        assert!(matches!(
            peek_guard_fields(bytes),
            Err(GuardReject::BadEnvelopeShape)
        ));
    }

    // --- Task 2 (17-02): ReplayCache / replay_bounds_ok ---

    #[test]
    fn empty_cache_accepts_first_sighting_and_rejects_immediate_replay() {
        let mut cache = ReplayCache::new();
        let now = OffsetDateTime::now_utc();
        assert_eq!(cache.len_for("agent:hosta.test/alice"), 0);
        assert!(cache
            .record("agent:hosta.test/alice", "first-nonce", now)
            .is_ok());
        assert!(matches!(
            cache.record("agent:hosta.test/alice", "first-nonce", now),
            Err(GuardReject::ReplayedNonce { .. })
        ));
    }

    #[test]
    fn replay_cache_rejects_second_sighting_of_same_nonce() {
        let mut cache = ReplayCache::new();
        let now = OffsetDateTime::now_utc();
        assert!(cache
            .record("agent:hosta.test/alice", "nonce-1", now)
            .is_ok());
        assert!(matches!(
            cache.record("agent:hosta.test/alice", "nonce-1", now),
            Err(GuardReject::ReplayedNonce { .. })
        ));
    }

    #[test]
    fn replay_cache_rejects_empty_nonce_before_touching_the_map() {
        let mut cache = ReplayCache::new();
        let now = OffsetDateTime::now_utc();
        assert!(matches!(
            cache.record("agent:hosta.test/alice", "", now),
            Err(GuardReject::MissingNonce { .. })
        ));
        assert_eq!(
            cache.len_for("agent:hosta.test/alice"),
            0,
            "an empty-nonce rejection must never insert into the map"
        );
    }

    #[test]
    fn replay_cache_record_after_ttl_elapsed_is_ok_again() {
        let mut cache = ReplayCache::new();
        let base = OffsetDateTime::now_utc();
        assert!(cache
            .record("agent:hosta.test/alice", "nonce-1", base)
            .is_ok());
        let after_ttl = base + time::Duration::seconds(REPLAY_CACHE_TTL_SECS + 1);
        assert!(
            cache
                .record("agent:hosta.test/alice", "nonce-1", after_ttl)
                .is_ok(),
            "an entry older than the TTL must be treated as expired, not replayed"
        );
    }

    #[test]
    fn per_sender_scoping_allows_identical_nonce_from_two_senders() {
        let mut cache = ReplayCache::new();
        let now = OffsetDateTime::now_utc();
        assert!(cache
            .record("agent:hosta.test/alice", "shared-nonce", now)
            .is_ok());
        assert!(cache
            .record("agent:hosta.test/bob", "shared-nonce", now)
            .is_ok());
        assert_eq!(cache.len_for("agent:hosta.test/alice"), 1);
        assert_eq!(cache.len_for("agent:hosta.test/bob"), 1);
    }

    #[test]
    fn per_sender_flood_evicts_only_the_flooding_senders_entries() {
        let mut cache = ReplayCache::new();
        let base = OffsetDateTime::now_utc();

        assert!(cache
            .record("agent:hosta.test/bob", "bob-nonce", base)
            .is_ok());

        for i in 0..=REPLAY_CACHE_MAX_PER_SENDER {
            let nonce = format!("alice-nonce-{i:05}");
            // Nanosecond deltas keep the whole flood microseconds wide,
            // nowhere near REPLAY_CACHE_TTL_SECS, so no in-flight sweep
            // removes an entry while the flood is still filling.
            let ts = base + time::Duration::nanoseconds(i as i64);
            assert!(cache.record("agent:hosta.test/alice", &nonce, ts).is_ok());
        }

        assert_eq!(
            cache.len_for("agent:hosta.test/alice"),
            REPLAY_CACHE_MAX_PER_SENDER,
            "sender A must be capped at its own limit, never grow past it"
        );
        assert_eq!(
            cache.len_for("agent:hosta.test/bob"),
            1,
            "sender A's flood must not touch sender B's entries"
        );
        assert!(
            matches!(
                cache.record("agent:hosta.test/bob", "bob-nonce", base),
                Err(GuardReject::ReplayedNonce { .. })
            ),
            "sender B's single entry must still be replay-detectable after A's flood"
        );
    }

    #[test]
    fn replay_cache_eviction_is_oldest_first_and_deterministic_on_ties() {
        let mut cache = ReplayCache::new();
        let base = OffsetDateTime::now_utc();
        let sender = "agent:hosta.test/alice";

        // Two entries sharing the OLDEST instant, distinct nonces.
        assert!(cache.record(sender, "zzz-tie", base).is_ok());
        assert!(cache.record(sender, "aaa-tie", base).is_ok());

        // Fill to N entries at strictly later, strictly increasing
        // instants than the tie pair -- nanosecond deltas so the whole
        // fill stays microseconds wide, nowhere near the TTL, and no
        // in-flight sweep removes anything while filling.
        for i in 0..(REPLAY_CACHE_MAX_PER_SENDER - 2) {
            let nonce = format!("later-{i:05}");
            let ts = base + time::Duration::nanoseconds(i as i64 + 1);
            assert!(cache.record(sender, &nonce, ts).is_ok());
        }
        assert_eq!(cache.len_for(sender), REPLAY_CACHE_MAX_PER_SENDER);

        // One more insert triggers eviction: the oldest instant wins,
        // and on that tie the lexicographically smaller nonce
        // ("aaa-tie") must be the one evicted -- never HashMap
        // iteration order.
        let trigger_ts = base + time::Duration::seconds(1);
        assert!(cache.record(sender, "trigger", trigger_ts).is_ok());
        assert_eq!(cache.len_for(sender), REPLAY_CACHE_MAX_PER_SENDER);

        assert!(
            matches!(
                cache.record(sender, "zzz-tie", base),
                Err(GuardReject::ReplayedNonce { .. })
            ),
            "zzz-tie must still be present -- it was not the one evicted"
        );
        assert!(
            cache.record(sender, "aaa-tie", base).is_ok(),
            "aaa-tie must have been evicted -- re-recording it must succeed as first-seen"
        );
    }

    #[test]
    fn replay_cache_shipped_constants_satisfy_the_inequality() {
        // Plan 03's shipped rate-limit constants (RATE_LIMIT_MAX_PER_WINDOW
        // = 120, RATE_LIMIT_WINDOW_SECS = 60) don't exist in this module
        // yet -- passed as literals matching plan 03's planned values, per
        // `replay_bounds_ok`'s own doc comment.
        assert!(replay_bounds_ok(
            CLOCK_SKEW_WINDOW_SECS,
            REPLAY_CACHE_TTL_SECS,
            REPLAY_CACHE_MAX_PER_SENDER,
            120,
            60,
        ));
    }

    #[test]
    fn replay_cache_inequality_rejects_misconfigured_constants() {
        // Relation (a): ttl one second below 2 * skew must fail.
        assert!(!replay_bounds_ok(
            CLOCK_SKEW_WINDOW_SECS,
            2 * CLOCK_SKEW_WINDOW_SECS - 1,
            REPLAY_CACHE_MAX_PER_SENDER,
            120,
            60,
        ));

        // Relation (b): max_per_sender one below ttl * rate_max /
        // rate_window_secs must fail.
        let required = REPLAY_CACHE_TTL_SECS * 120 / 60;
        assert!(!replay_bounds_ok(
            CLOCK_SKEW_WINDOW_SECS,
            REPLAY_CACHE_TTL_SECS,
            (required - 1) as usize,
            120,
            60,
        ));

        // Zero rate_window_secs must not panic (divide-by-zero) -- it must
        // simply return false.
        assert!(!replay_bounds_ok(
            CLOCK_SKEW_WINDOW_SECS,
            REPLAY_CACHE_TTL_SECS,
            REPLAY_CACHE_MAX_PER_SENDER,
            120,
            0,
        ));
    }

    /// Task 2 (17-02, INGR-03/D-07): the executable form of the
    /// restart-reopened-window bound. A fresh `IngressGuard` (simulating
    /// a restart) has an empty replay cache that would happily accept a
    /// nonce it has never seen -- but `freshness_check` rejects the same
    /// envelope regardless, because its `ts` is older than
    /// `2 * CLOCK_SKEW_WINDOW_SECS`. The cache's emptiness never widens
    /// the accepted window; the freshness gate is what bounds it.
    #[test]
    fn restart_reopened_window_is_bounded_by_the_freshness_check() {
        let mut guard = IngressGuard::new();
        let sender = "agent:hosta.test/oscar";
        assert_eq!(guard.replay.len_for(sender), 0);

        let base = OffsetDateTime::now_utc();
        let now = to_canonical_rfc3339(base);
        let too_old_instant = base - time::Duration::seconds(2 * CLOCK_SKEW_WINDOW_SECS + 1);
        let too_old = to_canonical_rfc3339(too_old_instant);

        // The empty cache would accept this as a first-seen nonce.
        assert!(guard
            .replay
            .record(sender, "replay-nonce", too_old_instant)
            .is_ok());

        // The freshness gate rejects it regardless of what the cache
        // remembers -- this is the executable bound on the
        // restart-reopened window.
        assert!(matches!(
            freshness_check(&too_old, &now, CLOCK_SKEW_WINDOW_SECS),
            Err(GuardReject::StaleTimestamp { .. })
        ));
    }

    // --- Task 1 (17-03, INGR-04): audience_check ---

    fn peeked_fields(from: &str, to: &str) -> PeekedFields {
        PeekedFields {
            from: from.parse().unwrap(),
            to: to.parse().unwrap(),
            ts: to_canonical_rfc3339(OffsetDateTime::now_utc()),
            nonce: Some("nonce-1".to_string()),
            expiry: None,
        }
    }

    #[test]
    fn audience_check_passes_when_domain_matches_and_sender_is_backed() {
        let peeked = peeked_fields("agent:hosta.test/oscar", "agent:hostb.test/peggy");
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        let input = GuardInput {
            peeked: &peeked,
            now: &now,
            own_domain: Some("hostb.test"),
            sender_is_backed: true,
        };
        assert!(audience_check(&input).is_ok());
    }

    #[test]
    fn audience_check_rejects_foreign_domain_naming_both_values() {
        let peeked = peeked_fields("agent:hosta.test/oscar", "agent:otherdomain.test/peggy");
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        let input = GuardInput {
            peeked: &peeked,
            now: &now,
            own_domain: Some("hostb.test"),
            sender_is_backed: true,
        };
        match audience_check(&input) {
            Err(GuardReject::ForeignDomain { expected, got }) => {
                assert_eq!(expected, "hostb.test");
                assert_eq!(got, "otherdomain.test");
            }
            other => panic!("expected ForeignDomain naming both values, got {other:?}"),
        }
    }

    #[test]
    fn audience_check_rejects_sender_not_backed_when_domain_matches() {
        let peeked = peeked_fields("agent:hosta.test/oscar", "agent:hostb.test/peggy");
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());
        let input = GuardInput {
            peeked: &peeked,
            now: &now,
            own_domain: Some("hostb.test"),
            sender_is_backed: false,
        };
        assert!(matches!(
            audience_check(&input),
            Err(GuardReject::SenderNotBacked { .. })
        ));
    }

    /// D-22: `own_domain: None` skips ONLY the domain half — the backing
    /// half is still enforced unconditionally.
    #[test]
    fn audience_check_with_own_domain_none_skips_domain_half_but_still_enforces_backing() {
        let peeked = peeked_fields("agent:hosta.test/oscar", "agent:otherdomain.test/peggy");
        let now = to_canonical_rfc3339(OffsetDateTime::now_utc());

        let input_backed = GuardInput {
            peeked: &peeked,
            now: &now,
            own_domain: None,
            sender_is_backed: true,
        };
        assert!(
            audience_check(&input_backed).is_ok(),
            "own_domain unset must skip the domain half regardless of `to`'s authority"
        );

        let input_unbacked = GuardInput {
            peeked: &peeked,
            now: &now,
            own_domain: None,
            sender_is_backed: false,
        };
        assert!(
            matches!(
                audience_check(&input_unbacked),
                Err(GuardReject::SenderNotBacked { .. })
            ),
            "own_domain unset must NOT skip the backing half"
        );
    }

    // --- Task 1 (17-03, INGR-06): RateLimiter ---

    #[test]
    fn rate_limiter_first_request_is_never_limited_and_window_resets() {
        let mut limiter = RateLimiter::new();
        let sender = "agent:hosta.test/alice";
        let now = OffsetDateTime::now_utc();
        assert!(
            limiter.admit(sender, now).is_ok(),
            "first request must always be admitted"
        );

        let after_window = now + time::Duration::seconds(RATE_LIMIT_WINDOW_SECS + 1);
        assert!(
            limiter.admit(sender, after_window).is_ok(),
            "a sender whose window has fully elapsed must restart from a clean count"
        );
    }

    #[test]
    fn rate_limiter_admits_exactly_the_limit_then_rejects() {
        let mut limiter = RateLimiter::new();
        let sender = "agent:hosta.test/alice";
        let now = OffsetDateTime::now_utc();
        for i in 0..RATE_LIMIT_MAX_PER_WINDOW {
            assert!(
                limiter.admit(sender, now).is_ok(),
                "request {i} (within RATE_LIMIT_MAX_PER_WINDOW) must be admitted"
            );
        }
        assert!(
            matches!(
                limiter.admit(sender, now),
                Err(GuardReject::RateLimited { .. })
            ),
            "the request one past RATE_LIMIT_MAX_PER_WINDOW must be rejected"
        );
    }

    #[test]
    fn rate_limiter_counts_are_per_sender() {
        let mut limiter = RateLimiter::new();
        let now = OffsetDateTime::now_utc();
        let alice = "agent:hosta.test/alice";
        let bob = "agent:hosta.test/bob";

        for _ in 0..RATE_LIMIT_MAX_PER_WINDOW {
            assert!(limiter.admit(alice, now).is_ok());
        }
        assert!(matches!(
            limiter.admit(alice, now),
            Err(GuardReject::RateLimited { .. })
        ));

        assert!(
            limiter.admit(bob, now).is_ok(),
            "exhausting sender A's budget must not limit sender B (INGR-08 cross-tenant isolation)"
        );
    }
}
