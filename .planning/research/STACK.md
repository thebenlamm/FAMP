# Stack Research — FAMP v1.1 Open-Internet Federation

**Domain:** Adding public reachability, cross-person trust bootstrap, protocol-grade ingress, a signed peer directory, and a push-notification adapter to an already-shipped Rust protocol implementation (`famp-gateway`, axum 0.8 / rustls 0.23 / tokio, v1.0.0 tagged 2026-07-29).
**Researched:** 2026-07-30
**Confidence:** HIGH on all live-fetched crates.io version/date data; MEDIUM on library-fit judgment calls (Context7-sourced); LOW on hosting-price figures (fast-moving, third-party blog aggregation, not vendor-primary-sourced) — treat $/month numbers as directional, verify against the vendor's own pricing page before Phase 13 locks a decision.

> **Everything in the existing v1.0 stack (`ed25519-dalek 2.2`, `serde_jcs 0.2`, `axum 0.8.8`, `rustls 0.23.38`, `reqwest 0.13.2`, `tokio 1.51.1`, `thiserror 2`/`anyhow 1`, `proptest`/`stateright`/`insta`, `cargo-nextest`, `just`) is NOT re-researched here** — see `.planning/research/archive/v0.6/STACK.md` for that rationale. This file covers ONLY the delta for v1.1's five target features. Layer 0 crates (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`) stay frozen — every recommendation below either adds a dependency to Layer 1/2 crates (`famp-bus`, `famp-gateway`, `famp` binary) or, where flagged, adds **no dependency at all** and reuses what's already in the tree.

---

## Headline finding: three of the five feature areas need zero or near-zero new dependencies

Before the crate tables — the single most load-bearing fact for the roadmapper:

| Feature area | New crate needed? |
|---|---|
| 1. Public reachability | **YES** — one pick, but it's an infra/hosting decision more than a crate decision (see below) |
| 2. Cross-person trust bootstrap | **NO new crypto crate.** Optional one small wordlist/QR crate, low priority |
| 3. Protocol-grade ingress | **YES, two crates** (`moka`, `tower_governor`) — both small, both drop into the existing axum ingress |
| 4. Signed peer directory | **NO new crate.** Reuse `famp-canonical` + `famp-crypto` + the existing axum router |
| 5. SEED-002 push-notify adapter | **NO new crate.** `std`/`tokio::process` + one Cargo feature flag |

Do not let the roadmap over-provision this milestone with dependencies. The two genuinely load-bearing new additions are the reachability model (#1) and the ingress pair (#3).

---

## 1. Public reachability

**This is an infra/ops decision, not primarily a library decision — the milestone correctly gates it behind a zero-code Phase-13 spike.** What follows is the honest menu with current versions, real cost, and named operator for each, so that spike has verified inputs instead of recalled ones.

### Option A — Self-hosted relay (RECOMMENDED starting point for the spike)

**Why this shape fits FAMP specifically:** every envelope is already Ed25519-signed end-to-end (INV-10) before it reaches any transport. A relay therefore does **not need to be trusted** — it is a dumb, untrusted store-and-forward pipe. That means a minimal FAMP relay is small and needs **zero new Rust dependencies**: it's an axum service (already in the stack) exposing `POST /relay/<principal>` (append to a bounded per-principal queue) and `GET /relay/<principal>` (drain), reusing `famp-inbox` (already exists, durable JSONL append-with-fsync) for storage and `famp-canonical`/`famp-envelope` for framing. Both peers dial **out** to the relay over HTTPS — neither peer needs an open inbound port, which is the real advantage over every tunnel option in Section C below.

- **What it looks like:** ~150–300 LoC axum service, no new crate. The interesting engineering is bounding queue depth/TTL per principal (reuse the `moka` pick from Section 3) and deciding whether the relay itself needs its own TOFU/domain identity (it doesn't need to sign anything, but its own TLS endpoint still needs a real cert — reuse `rustls`/`rcgen`/Let's Encrypt via the hosting provider, already in the stack's TLS story).
- **Hosting shapes, real cost, named operator (Ben, since he's the technical party):**

| Provider | Smallest viable instance | Monthly cost | Notes |
|---|---|---|---|
| **Fly.io** | `shared-cpu-1x`, 256MB | **~$2–2.32/mo** always-on; can drop under $1/mo with auto-stop-on-idle (acceptable for a relay only two named peers use) | No more permanent free tier (removed 2024); new accounts get $5 trial credit. Cheapest of the three for a tiny always-on box. |
| **AWS Lightsail** | Nano, 512MB/2vCPU/20GB | **$3.50/mo** IPv6-only, **$5/mo** with a public IPv4 | A relay needs a public IPv4 in practice (most home/mobile clients still need v4 reachability) → budget **$5/mo**. Simplest "just SSH in and run a binary" ops model if Ben is already AWS-familiar. |
| **Hetzner Cloud** | CX22 | **~€7.99/mo (~$8.50/mo)** as of the April 2026 price increase, with a further June 2026 increase reported on some instance families | Historically the cheapest VPS option; **the 2026 price hikes erode that advantage** — verify the live price on hetzner.com before deciding, third-party aggregator numbers disagreed by nearly 2x during this research pass. |
| **Cloudflare** | N/A as a VM | Not applicable here — Cloudflare doesn't sell a general-purpose VPS; its relevant offering is Tunnel (Option C) | Don't conflate "Cloudflare" the VPS option with Cloudflare Tunnel; they solve different problems. |

**Recommendation for the spike:** Fly.io if idle-stop is acceptable (cheapest, native to a "small always-on service" shape); Lightsail $5/mo tier if Ben wants boring/familiar ops. Either is operated by **Ben** — the follower does zero reachability setup, which is the strongest fit for "the follower's only job is the trust bootstrap and running the software."

### Option B — NAT traversal / hole-punching crates (avoid a relay by dialing direct)

| Crate | Version (crates.io, verified live) | Last published | Maintenance | Verdict |
|---|---|---|---|---|
| **`iroh`** | **1.0.3** | 2026-07-20 | Active, just reached stable 1.0 | **Most interesting single answer to this whole question** — see callout below |
| `quinn` | 0.11.11 | 2026-06-22 | Active | The QUIC engine iroh is built on; using it directly means hand-building the hole-punch/relay/addressing logic iroh already ships |
| `libp2p` | 0.56.0 | **2025-06-27 (~13 months stale as of this research date)** | Slowing | Has QUIC transport + DCUtR hole-punching + relay-v2, but large dependency surface and a release cadence that has visibly slowed. Don't adopt for a two-crate-team protocol library — the dependency weight isn't justified by what FAMP needs. |
| `webrtc` (webrtc-rs) | 0.17.2 | 2026-07-22 | Active, but **0.17.x is explicitly the final feature release of the Tokio-coupled implementation** — bugfix-only branch going forward | WebRTC's ICE/STUN/TURN semantics target browser audio/video interop; heavyweight and shape-mismatched for a signed-JSON-envelope protocol |
| `str0m` | 0.21.0 | 2026-06-27 | Active (algesten, sans-IO style) | Same shape-mismatch as webrtc-rs — it's a WebRTC/ICE library, not a generic hole-puncher |
| `stunclient` | 0.4.2 | 2025-12-09 | Low activity, does one thing | Just resolves your external IP/port via STUN — you'd still hand-roll TURN fallback and hole-punch orchestration yourself |
| `turn` (webrtc-rs family) | 0.17.2 | 2026-07-20 | Active | Pure-Rust TURN server/client — usable to build your own relay-with-hole-punch-assist, but this is reinventing what iroh already packages |

**iroh callout — read this before dismissing it.** iroh is a Rust p2p/QUIC library built specifically for FAMP's exact shape: endpoints are dialed **by public key** (not IP:port), it does hole-punching automatically, and it falls back to a relay transparently when direct connection fails — n0-computer runs 4 free public relays (2 US, 1 EU, 1 Asia, rate-limited for dev/test) and sells a managed dedicated-relay tier ("Iroh Services") with an uptime SLA if you outgrow the free ones. In practice ~9/10 connections go fully direct once the relay assists the initial hole-punch. **It covers both "public reachability" and "node addressing" in one maintained crate that just hit 1.0** — exactly what the question asked to flag.

**Why it's not the default pick despite that:** adopting iroh means replacing the wire transport under `famp-gateway` — its own QUIC-based `Endpoint` type, not an HTTPS POST to an axum router. The current `famp-transport-http` + `famp-gateway` (axum/rustls/reqwest) is shipped, tested, and proven live (Gate A, 2026-07-29). Swapping the transport layer this milestone is a bigger rearchitecture than "put a relay in front of the existing HTTPS ingress." **Recommendation:** let the Phase 13 spike weigh iroh explicitly as "replace the transport" against "keep the transport, add a self-hosted relay in front of it" — don't let it get dismissed for being unfamiliar; it is the single best-fit crate in the ecosystem for this problem, the cost is architectural, not maintenance risk.

**Do NOT build custom STUN/TURN/ICE plumbing from `stunclient` + `turn` by hand** — that's redoing engineering iroh already did, tested, and shipped at 1.0.

### Option C — Avoid a relay entirely: hosted tunnel services (what a non-technical follower experiences)

These require nothing new in `Cargo.toml` — they're external binaries/services the follower installs. Compared here because the milestone's acceptance criterion is specifically about follower experience, and this is the one place where each option's onboarding friction differs sharply.

| Option | Cost | Who operates it | Follower's setup burden | Verdict |
|---|---|---|---|---|
| **Tailscale Funnel** | Free (Funnel ships on all plans incl. free 6-user "Personal" tier) | The follower, on their own tailnet | Sign up (email/OAuth) → install client → `tailscale up` → `tailscale funnel <port> on`. No domain purchase — Tailscale auto-issues a `*.ts.net` HTTPS cert and stable subdomain. **Best-in-class follower UX of the three tunnel options.** | Undisclosed bandwidth cap on Funnel specifically; fine for a low-volume message bus |
| **Cloudflare Tunnel** | Free | The follower | `brew/apt install cloudflared` → Cloudflare dashboard → create tunnel → paste token. **Quick Tunnels** need no account/domain but issue an ephemeral `*.trycloudflare.com` URL that changes on restart — breaks a stable-peer-directory story. **Named/persistent tunnels** need the follower to own a domain and add it to Cloudflare DNS — real friction (domain purchase, ~$10+/yr) for a "non-technical, unassisted" follower. | Only recommend if the follower already owns a domain |
| **ngrok** | Free tier: 1GB/mo bandwidth, 1 free static dev-domain, **but sessions now cap at 2 hours** | The follower | Simple CLI, but the **2-hour session cap is disqualifying** for a persistent listener — the tunnel would need re-establishing every 2 hours, unacceptable for a "long-running agent process" | **Do not use** for anything but a one-off demo |
| Direct-dial + manual port-forward | Free (uses follower's own router) | The follower | Router admin UI + either a static public IP or dynamic-DNS client — genuinely the worst UX of any option here for a non-technical person | **Do not recommend** |

**Bottom line for Section 1:** the self-hosted relay (Option A, Ben-operated, ~$2–5/mo) removes the reachability burden from the follower entirely — the strongest fit for the milestone's "doc-only, unassisted" acceptance bar. Tailscale Funnel is the best fallback if the spike decides against operating infra. iroh is the most technically elegant single answer but carries a real transport-migration cost this milestone may not want to pay. ngrok and manual port-forwarding should be ruled out now, not re-litigated at the spike.

---

## 2. Cross-person trust bootstrap

**No new cryptography crate is the honest recommendation here.** The v1.0 mechanism (`famp peer export` → paste over Signal → `famp peer import`) doesn't fail because the crypto primitive is wrong — Ed25519 + TOFU is fine — it fails because a human can silently paste the wrong blob, or a MITM'd side channel can substitute one, and nothing forces a verification step. The fix is a **short authentication string (SAS)** built from primitives already in the stack (`sha2`, `base64`, `famp-crypto`), not a PAKE.

| Crate | Version | Last published | Fit for FAMP | Verdict |
|---|---|---|---|---|
| `opaque-ke` | 4.0.1 | 2026-03-27, active | OPAQUE (now RFC 9807) is an **augmented, asymmetric** PAKE — designed for "client authenticates to a server holding a password verifier." FAMP's bootstrap is flat peer-to-peer with no server role. Shape mismatch. | **Do not add.** |
| `spake2` (RustCrypto) | 0.4.0 | 2026-01-25, active (moved into `RustCrypto/PAKEs`, MSRV 1.85) | Symmetric PAKE — closer fit *if* you want two parties who share a low-entropy secret (e.g., a word Ben reads over the phone) to derive a strong shared key. Real option if the milestone wants to replace pubkey-paste with a PAKE-derived channel. | Candidate only if SAS-over-existing-pubkeys is rejected — see below for why SAS is simpler |
| `srp` | 0.6.0 | 2026-04-03, active | RFC 5054 SRP — same asymmetric/verifier-based shape mismatch as OPAQUE. | **Do not add.** |

**Recommended shape (no new crate):** after the existing `peer export`/`peer import` blob exchange, compute a short human-comparable fingerprint from both parties' Ed25519 public keys (e.g., `SHA-256(pubkey_a || pubkey_b)` truncated and encoded as a handful of words or digits), have both humans read it aloud/compare it over the same side channel (Signal call, not just paste) before the pin is trusted. This is the Signal/Matrix SAS pattern, and it needs nothing beyond `sha2` + `base64` (already workspace deps) plus, optionally, a wordlist for human-friendliness:

| Crate | Version | Last published | Verdict |
|---|---|---|---|
| `niceware` | 1.0.0 | **2022-01-21 (single release, ~4.5 years stale)** | The wordlist is a static, unchanging EFF-derived asset — "stale" here is the `base64`-style "feature-complete, not abandoned-and-broken" pattern, not a real risk. Usable, but see alternative. |
| *(no crate)* BIP-39 English wordlist | N/A | N/A | **Recommended instead of `niceware`:** the BIP-39 wordlist is 2048 static, public-domain words; vendoring it as a const array is ~20 lines you own and test once, exactly the "fork the primitive, own the correctness" call already made for `serde_jcs`/`famp-canonical` in v0.6. Avoids depending on a single-release, single-maintainer crate for something this small. |

**QR codes — low priority, likely defer.** FAMP is a CLI-to-CLI tool; a QR code only helps if there's a mobile-scan step in the trust flow, which isn't part of this milestone's scope. If a QR step is ever added:

| Crate | Version | Last published | Verdict |
|---|---|---|---|
| `qrcode-generator` | 6.0.0 | **2026-07-18 (most recently updated of the three)** | Supports QR + Micro QR + rMQR, pure Rust — pick this one if/when needed |
| `fast_qr` | 0.13.1 | 2025-06-13 | ~6–7x faster than `qrcode`, actively maintained, reasonable second choice |
| `qrcode` (qrcode-rust) | 0.14.1 | **2024-07-05 (~2 years stale)** | Largest download count/ecosystem familiarity but visibly slower-moving; don't default to it just because it's the most-linked name |

**What NOT to add:** `opaque-ke`, `srp` (wrong PAKE shape for a flat peer bootstrap); don't reach for a QR crate at all unless a mobile-scan flow is actually scoped.

---

## 3. Protocol-grade ingress

Two small crates, both drop cleanly into the existing axum ingress in `famp-gateway`.

### Bounded replay/nonce cache

| Crate | Version | Last published | Verdict |
|---|---|---|---|
| **`moka`** | **0.12.15** | 2026-03-22, active | **Recommended.** Sync and async (`moka::future::Cache`) variants; `CacheBuilder` gives `time_to_live` + `max_capacity` in one call — maps directly onto "bound by the envelope's `expiry` field, cap total memory." `future::Cache` clones cheaply and is meant to be shared across tokio tasks without an `Arc<Mutex<_>>` wrapper, which fits the gateway's `tokio::select!`-based ingress/egress loop. Single-flight `get_with`/`optionally_get_with` also gives cache-stampede protection for free if the revocation-list lookup (below) shares the same cache. |
| `lru` | 0.18.1 | 2026-07-09, active | Simpler, single-threaded, no built-in TTL — you'd hand-roll expiry on top. Only reach for this if `moka`'s concurrency machinery is judged as overkill for a two-peer gateway; it isn't. |
| `quick_cache` | 0.7.0 | 2026-06-27, active | Lighter-weight, benchmarks favorably in some workloads, has a "lifespan"/weigher mechanism rather than first-class TTL. Real alternative, but `moka` is the more battle-tested, more widely deployed choice and its API maps onto "nonce + expiry" more directly. |

**Recommendation:** `moka 0.12` for the replay/nonce cache. It's the standard choice in the Rust ecosystem for exactly this shape (bounded, TTL'd, concurrent-safe cache) and needs no custom eviction code.

### Rate limiting / DoS ordering

| Crate | Version | Last published | Verdict |
|---|---|---|---|
| **`tower_governor`** | **0.8.0** | 2025-08-14 (**~1 year stale** — flagged, not disqualifying) | **Recommended.** Wraps `governor` (below) as a `tower`/axum `GovernorLayer` — drops directly onto the existing axum router with one `.layer(...)` call. Ships `SmartIpKeyExtractor` (checks `x-forwarded-for`/`x-real-ip`/`forwarded` before falling back to peer IP — relevant if the relay from Section 1 sits in front of the gateway) and a custom `KeyExtractor` trait if you'd rather rate-limit by sender principal than by IP. Ships preset configs (`GovernorConfig::secure()` — 2 req/4s burst — explicitly aimed at auth-style endpoints, a good starting point for the ingress boundary). |
| `governor` (direct) | 0.10.4 | 2025-12-16, active | The underlying GCRA rate-limiter `tower_governor` wraps. Only reach for this directly if you need rate limiting somewhere `tower_governor`'s axum-specific glue doesn't reach (unlikely for this milestone — the whole surface is the axum ingress). |

**Maintenance note:** `tower_governor`'s ~1-year-stale release is the one release-cadence flag in this document worth carrying into the roadmap risk list — it's a thin, stable API wrapping an actively-maintained core (`governor`), so the risk is low, but it's not a crate with weekly activity. Re-verify it hasn't been superseded before locking the dependency.

### Key revocation list

**No crate — this is a domain-specific policy structure, not a generic data-structure problem.** Model it as a signed, versioned list of revoked `key_id`s using the same substrate as the signed peer directory (Section 4): `famp-canonical` + `famp-crypto` produce a signed JSON document; membership checks are a `HashSet<KeyId>` refreshed on a `moka` TTL (reusing the cache already being added above, rather than a third dependency).

**What NOT to add:** don't build a bespoke rate-limiter from scratch (`governor`'s GCRA implementation is exactly right and already audited-by-use); don't add a second cache crate alongside `moka` for the revocation list — one cache, two uses.

---

## 4. Signed peer directory

**Borrow the SHAPE from established transparency/trust-distribution formats — do not add any of these as a dependency.** v1.1 stays bilateral (per PROJECT.md's explicit non-scope), so the heavyweight multi-party formats below are architecturally overkill; only the lightweight ones are worth even shape-borrowing this milestone.

| Format | What to borrow | What NOT to do |
|---|---|---|
| **`.well-known` conventions** | **Most FAMP-native shape.** Serve a JCS-canonical, Ed25519-signed JSON document at `.well-known/famp/peers.json` (or similar) over the **existing axum ingress** — zero new dependencies, pure reuse of `famp-canonical` + `famp-crypto` + the router already in `famp-transport-http`. This directly closes the TRANS-05 gap ARCHITECTURE.md already flags as deferred (Agent-Card `.well-known` distribution). | Don't invent a bespoke discovery protocol when the HTTP convention already exists and the signing substrate is already built. |
| **DNSSEC/DANE** | The "bind an expected key fingerprint to a domain name via DNS" shape is directly applicable since v1.0 Phase 11 already made addressing domain-qualified (`agent:<domain>/<name>`). A `_famp._tcp.<domain>` TXT record publishing the expected gateway pubkey fingerprint upgrades blind TOFU into "verify against a DNS-published value" — genuinely low-cost (no new crate; DNS resolution is already transitively available via `reqwest`/`tokio`'s resolver stack). | Don't require full DNSSEC chain validation this milestone — that's a real dependency (a validating resolver) for marginal gain over a plain TXT-record cross-check at TOFU time. |
| **TUF (The Update Framework)** | Borrow the "versioned, signed, expiring metadata with role separation" idea in the abstract — useful if the directory ever needs rollback/freeze-attack protection. | **Do not add the `tuf` Rust crate.** It's built for software-update distribution (root/targets/timestamp/snapshot roles), not peer directories — the dependency weight isn't justified by a two-party trust model. |
| **Sigstore/Rekor, Certificate Transparency** | Borrow the "append-only, publicly auditable log" shape only as a forward note for a hypothetical multi-party v2 directory. | Not applicable at bilateral scale — don't scope either into v1.1. |

---

## 5. Push notification adapter (SEED-002: `famp watch --notify <cmd>`)

**Near-zero new dependencies — say so plainly, as instructed.** This is a subscriber on the broker's existing per-identity event stream (the same wake mechanism `famp_await`'s long-poll already uses inside `famp-bus`), except instead of returning control to the calling MCP tool, it `exec`s a command per envelope.

- **What's already in the tree and sufficient:** `tokio` (already a workspace dependency) — needs the `process` Cargo feature enabled on whichever crate/binary implements `famp watch` (a feature-flag change, not a new dependency) to get `tokio::process::Command` for non-blocking child-process spawns from the async broker-client loop. The existing UDS bus client (`famp-bus`) already delivers the envelope; no new wire logic.
- **Recommended design to avoid needing any new crate:** pass envelope metadata to the child process via **environment variables** (`FAMP_SENDER`, `FAMP_TASK_ID`, `FAMP_ENVELOPE_ID`, etc.) rather than string-interpolating envelope content into a shell command line. This sidesteps shell-injection risk entirely and means no shell-tokenizing crate (e.g. `shlex`) is needed — `<cmd>` is `exec`'d directly with `Command::new(argv[0]).args(&argv[1..])`, not passed through `/bin/sh -c`.
- **If the milestone insists on shell-string templating instead of env-vars:** that's the one place a small crate (`shlex`, or similar) would become relevant, purely for safe tokenization — but this is avoidable by design, and avoiding it is the safer default given the "inbound content is DATA, not instructions" security gate this same milestone already treats as blocking.

**What NOT to add:** no message-queue crate, no new async-runtime feature beyond `tokio`'s `process` flag, no shell-templating crate if envvar-passing is adopted (recommended).

---

## Consolidated new-dependency table (for `Cargo.toml`)

| Crate | Version | Feature area | Confidence |
|---|---|---|---|
| `moka` | `0.12.15` (features: `future` if used from the async gateway loop) | §3 replay/nonce cache (dual-purpose w/ §3 revocation cache) | MEDIUM (Context7-sourced fit judgment; version HIGH via crates.io) |
| `tower_governor` | `0.8.0` (pulls `governor 0.10.4`) | §3 rate limiting on the axum ingress | MEDIUM (fit) / HIGH (version) — flag the ~1yr-stale release in roadmap risk notes |
| *(infra decision, not a crate)* Fly.io / Lightsail relay, or `iroh 1.0.3`, or Tailscale Funnel | — | §1 public reachability | Gated on Phase 13 spike; iroh version HIGH-confidence, cost figures LOW-confidence (see top-of-file caveat) |
| *(optional, low priority)* BIP-39 wordlist (vendored, no crate) or `niceware 1.0.0` | — | §2 SAS fingerprint human-readability | LOW — defer decision until the SAS design is actually specced |
| *(optional, likely deferred)* `qrcode-generator 6.0.0` | — | §2 QR — only if a mobile-scan flow gets scoped | LOW — no evidence this milestone needs it |

No changes needed to Layer 0 (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`) — every recommendation above lands in `famp-bus`, `famp-gateway`, or the `famp` binary, consistent with the milestone's frozen-Layer-0 constraint. `cargo tree -i openssl` stays empty: `moka`, `tower_governor`/`governor`, `spake2`, `srp`, `opaque-ke`, `iroh`, and every QR/wordlist crate considered here are pure-Rust with no OpenSSL pull-through.

---

## What NOT to Use

| Avoid | Why | Use instead |
|---|---|---|
| `libp2p` | 13-month-stale release cadence as of this research date, and a dependency footprint far larger than a two-crate protocol library needs | `iroh` if a full P2P transport is wanted; otherwise the existing axum/rustls gateway + a relay |
| Hand-rolled STUN/TURN/ICE via `stunclient` + `turn` | Reinvents engineering `iroh` already shipped and hit 1.0 with | `iroh 1.0.3` if hole-punching is the chosen model |
| `webrtc` / `str0m` | Both are WebRTC/ICE libraries aimed at browser audio/video interop — shape-mismatched for a signed-JSON message protocol | `iroh` (QUIC, pubkey-addressed) or the relay approach |
| `opaque-ke`, `srp` | Asymmetric, verifier-based PAKEs designed for client-authenticates-to-server; FAMP's bootstrap is flat peer-to-peer | A SAS (short authentication string) built from `sha2`/`base64` already in the stack, or `spake2` if a PAKE is truly wanted |
| `ngrok` free tier | 2-hour session cap disqualifies it for a persistent listening service | Tailscale Funnel, or the self-hosted relay |
| Cloudflare Tunnel **Quick Tunnels** as the peer's stable address | Ephemeral URL changes every restart — breaks a stable-peer-directory story | Cloudflare Tunnel **named** tunnel (needs a domain) or Tailscale Funnel (no domain needed) |
| Manual port-forward + dynamic DNS | Worst onboarding friction of any reachability option for a non-technical follower | Any of the tunnel or relay options above |
| `tuf` (Rust crate) | Built for software-update distribution role-separation, not a bilateral peer directory | Borrow the "signed, versioned, expiring metadata" shape directly via `famp-canonical`/`famp-crypto`, no crate |
| Shell-string templating for `famp watch --notify <cmd>` | Reintroduces the "inbound content is instructions" risk this same milestone treats as a blocking security gate | Pass envelope metadata via environment variables to the child process |

---

## Version compatibility notes

| A | B | Note |
|---|---|---|
| `moka 0.12` (`future` feature) | `tokio 1.51` | Needs `tokio`'s `rt` feature at minimum for the async cache variant; already enabled per the v1.0 workspace `Cargo.toml`. |
| `tower_governor 0.8` | `axum 0.8` | Confirmed compatible — its own examples target `axum::serve` with `into_make_service_with_connect_info::<SocketAddr>()`, which the existing gateway ingress can adopt directly for IP-keyed limiting. |
| `iroh 1.0.3` | `axum`/`rustls`-based `famp-transport-http` | **Not composable as an add-on** — iroh brings its own QUIC `Endpoint` and would replace, not augment, the HTTPS transport. Treat as an either/or architectural choice, not a coexisting dependency. |
| `spake2 0.4.0` | MSRV 1.85 | Workspace `rust-version = "1.89"` (per `Cargo.toml`) already clears this. |

---

## Sources

- **crates.io API** (live JSON fetch, 2026-07-30) — authoritative version/`updated_at` data for `iroh`, `quinn`, `moka`, `governor`, `tower_governor`, `opaque-ke`, `spake2`, `srp`, `niceware`, `qrcode`, `fast_qr`, `qrcode-generator`, `webrtc`, `str0m`, `libp2p`, `lru`, `quick_cache`, `stunclient`, `turn`, `rcgen`, `webpki-roots`
- **Context7** (`/n0-computer/iroh`, `/moka-rs/moka`, `/benwis/tower-governor`) — API shape, relay/hole-punch behavior, cache builder semantics, axum integration examples
- **Web search** (2026-07-30, no year injected into queries; publication dates checked on results) — hosting pricing (Fly.io, Hetzner, AWS Lightsail), tunnel service setup/pricing (Cloudflare Tunnel, Tailscale Funnel, ngrok), PAKE/wordlist/QR maintenance-status corroboration
- `.planning/PROJECT.md` (v1.1 milestone scope) and `ARCHITECTURE.md` (existing Layer 0/1/2 crate boundaries) — constraint source for what's frozen vs. open this milestone
- `.planning/research/archive/v0.6/STACK.md` — prior stack decisions, not re-litigated

---
*Stack research for: FAMP v1.1 Open-Internet Federation*
*Researched: 2026-07-30*
