<!-- GSD:project-start source:PROJECT.md -->
## Project

**FAMP — Federated Agent Messaging Protocol (Reference Implementation)**

A Rust reference implementation of FAMP (Federated Agent Messaging Protocol) v0.5 — a protocol defining semantics for communication among autonomous AI agents within a trusted federation. The implementation provides a conformance-grade library covering identity, causality, negotiation, commitment, delegation, and provenance across three protocol layers, plus a reference HTTP transport binding.

**Core Value:** **A byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one.** If canonicalization or signature verification disagrees, nothing else matters.

### Constraints

- **Tech stack**: Rust (stable, latest). `ed25519-dalek` for signatures, `serde` + custom canonicalizer for RFC 8785 JCS, `proptest` + `stateright` for state-machine model checking, `axum` or `hyper` for HTTP transport reference.
- **Tech stack (deferred)**: No Python/TS bindings in v1; keep FFI surface clean but unwired.
- **Transport**: HTTP/1.1 + JSON over TLS as reference wire; in-process `MemoryTransport` for tests. Other transports live behind the `Transport` trait.
- **Conformance target**: Staged conformance is supported — each milestone tags conformance level achieved; vector pack ships in v1.0 alongside federation gateway.
- **Spec fidelity**: v0.5.1 fork is the authority for this implementation. All diffs from v0.5 documented with reviewer rationale.
- **Security**: Every message signed (INV-10); unsigned messages rejected. Ed25519 non-negotiable. Domain separation prefix added in v0.5.1 fork.
- **Developer onboarding**: Rust toolchain install is Phase 0; assume zero prior Rust experience.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## TL;DR — The Winners
| # | Concern | Choice | Version | Confidence |
|---|---|---|---|---|
| 1 | Ed25519 | **`ed25519-dalek`** | `2.2.0` | HIGH |
| 2 | RFC 8785 JCS | **`serde_jcs`** (with in-house conformance gate; fork to `famp-canonical` if it fails) | `0.2.0` | **MEDIUM** ⚠️ |
| 3 | JSON | **`serde` + `serde_json`** (no SIMD) | `serde 1.0.228`, `serde_json 1.0.149` | HIGH |
| 4 | UUIDv7 | **`uuid`** with `v7` + `serde` features | `1.23.0` | HIGH |
| 5 | Base64url unpadded | **`base64`** (`URL_SAFE_NO_PAD` engine) | `0.22.1` | HIGH |
| 6 | SHA-256 | **`sha2`** | `0.11.0` | HIGH |
| 7 | HTTP server | **`axum`** (on tokio/hyper) | `0.8.8` | HIGH |
| 8 | HTTP client | **`reqwest`** (rustls backend) | `0.13.2` | HIGH |
| 9 | TLS | **`rustls`** (via `rustls-platform-verifier`) | `0.23.38` | HIGH |
| 10 | Async runtime | **`tokio`** (`full` features for bin, narrow for libs) | `1.51.1` | HIGH |
| 11 | Errors | **`thiserror`** in libs, **`anyhow`** only in CLI/tests | `thiserror 2.0.18`, `anyhow 1.0.102` | HIGH |
| 12 | Property testing | **`proptest`** | `1.11.0` | HIGH |
| 12 | FSM model checking | **`stateright`** | `0.31.0` | MEDIUM (see §5) |
| 12 | Snapshot / vectors | **`insta`** | `1.47.2` | HIGH |
| 13 | Task runner | **`just`** (Justfile) + **`cargo-nextest`** | nextest `0.9.132` | HIGH |
| 14 | Lint / format | `clippy` + `rustfmt` (shipped with rustup; strict settings below) | rust `1.87`+ | HIGH |
| 15 | CI | GitHub Actions with `dtolnay/rust-toolchain` + `Swatinem/rust-cache` + `taiki-e/install-action` | — | HIGH |
## 1. Ed25519 signing — `ed25519-dalek 2.2.0`
- Pure-Rust, RustCrypto ecosystem (same org as `sha2`, `curve25519-dalek`), 35M recent downloads.
- API matches exactly what FAMP §7.1 needs: `SigningKey::from_bytes(&[u8; 32])`, `sign(msg)` → `Signature` (64 bytes), `VerifyingKey::verify_strict(msg, &sig)`.
- `verify_strict` rejects non-canonical / small-order-point signatures — **this is what you want for protocol-level non-repudiation.** Plain `verify` is legacy-tolerant and should not be used.
- Key/sig wire format is raw bytes (32 pub, 64 sig), which matches the spec's "raw 32-byte pub / 64-byte sig, base64url" decision.
- Works on stable Rust. No C deps. Reproducible builds.
- **`ring`** — fast, FIPS-ish, but opinionated: exposes only high-level API, won't compose cleanly with the RustCrypto `signature::Signer` trait we'll use in `famp-crypto`'s trust abstraction. Also has its own cbindgen-generated asm that complicates cross-compilation. Reserve for a future FIPS profile, not v1.
- **`ed25519-compact`** — smaller, no_std, but single-maintainer and less audited.
- **`RustCrypto/ed25519`** is the *trait* crate; `ed25519-dalek` is the implementation. You need both (dalek pulls in the trait).
## 2. Canonical JSON / RFC 8785 JCS — `serde_jcs 0.2.0` **with a safety net**
- `serde_jcs 0.2.0` — published 2026-03-25, 34 direct dependents, self-labeled "unstable", maintained by `l1h3r`. Implements RFC 8785 via Serde. Uses `ryu-js` for JSON number serialization (required by RFC 8785, which mandates ECMAScript `Number.prototype.toString` semantics for numbers — a notorious corner case).
- No other widely-maintained Rust JCS crate exists. `json-canon` is abandoned. `canonical-json` crates exist for OLPC/Matrix canonical JSON (a *different, incompatible* spec) — **do not use them**.
- RustCrypto does not provide one.
- It is literally the only serde-integrated RFC 8785 implementation.
- The "unstable" label is about API churn, not correctness. 0.2.0 shipping two weeks before our research is a good sign, not a bad one.
- Forking is a 500-line job if needed (sort keys, RFC 8785 number formatter, UTF-8 pass-through). The `ryu-js` dep does the hardest part.
## 3. Serde + JSON — `serde 1.0.228` + `serde_json 1.0.149`
- `serde_json` is the reference implementation, maintained by `dtolnay`, and is what `serde_jcs` is built on. Any non-serde_json path means rewriting canonicalization.
- Preserves `Number` precision as `serde_json::Number`; supports `arbitrary_precision` feature if needed (NOT recommended for FAMP — it changes canonicalization behavior).
- **`simd-json 0.17`** and **`sonic-rs 0.5.8`** are real speedups for *parsing throughput* in high-QPS ingest paths. FAMP is not that. Messages are ≤ a few KB; signature verification dominates CPU; SIMD JSON parsing is irrelevant.
- Neither has a JCS canonicalizer. Using them for parsing + `serde_json` for canonicalization risks a double-parse divergence. **One JSON library, one source of truth.**
- `sonic-rs` requires nightly for some features. Beginner-unfriendly.
## 4. UUIDv7 — `uuid 1.23.0`
- UUIDv7 (time-ordered) is the right choice for FAMP conversation/task IDs: database-friendly index locality, debuggable (timestamp is visible in the first 48 bits), still globally unique with 74 bits of entropy.
- The `uuid` crate is the canonical Rust implementation. `v7` feature stabilized in 1.11; 1.23 is current.
- `serde` feature gives `Serialize`/`Deserialize` as the canonical hyphenated string form, which is what the spec shows.
- Don't invent your own "timestamp + random hex" scheme. RFC 9562 UUIDv7 is already the answer.
- Don't use `v4` (random only) — you lose time ordering for free.
## 5. Base64url unpadded — `base64 0.22.1`
- `base64` is the canonical Rust base64 crate; `URL_SAFE_NO_PAD` matches JOSE/JWT/Matrix conventions and what the spec shows (`base64url` unpadded). The 0.22 release stabilized the `Engine` API — any tutorial using `base64::encode_config(...)` is pre-0.21 and will not compile.
- **Strict decoding by default:** `URL_SAFE_NO_PAD` rejects trailing padding AND mixed alphabets. Critical for signature integrity — a non-canonical base64 input must not round-trip.
- Good crate, more general, but `base64` is smaller, more idiomatic, and is what every other Rust crypto crate in the ecosystem uses. Fewer surprises for a beginner.
## 6. SHA-256 — `sha2 0.11.0`
- RustCrypto's standard hash crate, same org as `ed25519-dalek`. Pure Rust, stable, auditable.
- Version 0.11.0 stabilized in March 2026 after ~9 months of RCs — it's the current major for the RustCrypto traits rework. All new code should target 0.11 directly; do not start on 0.10.x.
- Matches spec artifact-id scheme: `sha256:<hex>`.
- Same reason as ed25519: `ring` is opinionated, couples hashing to a fixed crypto universe, and doesn't compose with the `digest` trait used by the rest of the RustCrypto ecosystem.
## 7. HTTP server — `axum 0.8.8`
- De facto standard 2025–2026 Rust web framework. Built on `hyper` 1.x + `tokio`. Maintained by the `tokio-rs` org itself.
- Handler-function ergonomics with extractors — closest thing to "Flask but type-checked" in Rust. Much friendlier than `actix-web`'s actor model for a beginner.
- Composes with `tower` middleware stack, which is how you implement FAMP's signature-verification middleware as a reusable layer.
- `0.8` is current; `0.8.8` shipped December 2025. Stable API since 0.7.
- **`actix-web`** — fast, mature, but unsafe internals, heavier learning curve (actor model), and smaller middleware ecosystem. Not the modern default.
- **`warp`** — filter-combinator model. Cool, but filter type errors are famously horrible to debug. Not beginner-friendly.
- **`hyper` directly** — you *can* write a server on raw hyper 1.x, but you'll reinvent routing and extractors. `axum` is the thin layer that makes hyper ergonomic.
- **`rocket`** — requires nightly historically; usage declining.
## 8. HTTP client — `reqwest 0.13.2`
- Highest-level Rust HTTP client. Built on `hyper` + `tokio`. Same org as axum/tokio ecosystem.
- Disable default features to avoid pulling `native-tls` (which pulls OpenSSL on Linux — a portability nightmare).
- `rustls-tls-native-roots` uses the OS trust store but the `rustls` TLS stack. Best of both worlds.
- `hyper-util` is a low-level client builder. You'd reimplement connection pooling, redirects, and timeouts. `reqwest` wraps all that cleanly.
- Use hyper-util only if you need fine control over the connection pool or zero-copy body streaming. FAMP doesn't.
## 9. TLS — `rustls 0.23.38`
- `rustls` is now the default TLS in the Rust ecosystem. Pure-Rust, no OpenSSL, memory-safe.
- `0.23.38` shipped 2026-04-12 (literally today). Version 0.23 has been the stable line since early 2024; point releases are security-only.
- **Crypto provider choice:** `ring` (default, battle-tested) vs `aws-lc-rs` (FIPS-targeted, newer). **Pick `ring` for v1.** Switch to `aws-lc-rs` only if a federation requires FIPS.
- Add `rustls-platform-verifier = "0.5"` for OS-trust-store integration on client side. This replaces the old `rustls-native-certs` dance.
- For dev: self-signed certs via `rcgen = "0.14"`.
- For federation deployments: each federation provides its own trust anchor list — document this as a `Vec<TrustAnchor>` loaded at startup.
## 10. Async runtime — `tokio 1.51.1`
- Only serious choice. `async-std` is effectively abandoned. `smol` is niche. `tokio` is what axum/reqwest/hyper/rustls all assume.
- 1.51 is current stable; 1.x has upheld semver since 2020 — no breaking changes.
## 11. Error handling — `thiserror 2.0.18` (libs) + `anyhow 1.0.102` (bins/tests only)
- `thiserror` generates `Display` + `From` + `Error` impls for typed errors. Libraries must expose typed errors so callers can `match` — this is doubly true for FAMP because the spec's "reject with `unauthorized`" is compiler-checked when errors are enums.
- `anyhow::Error` is an untyped boxed error — great for `main()`, terrible for library APIs. Never return `anyhow::Result` from a `famp-*` crate.
- `thiserror 2.x` is the current major (released early 2025); purely additive changes from 1.x.
#[derive(Debug, thiserror::Error)]
## 12. Testing
### `proptest 1.11.0` — property-based tests
### `stateright 0.31.0` — state-machine model checking
### `insta 1.47.2` — snapshot testing
### Test runner: `cargo-nextest 0.9.132`
## 13. Workspace / build tooling
### Cargo workspace (no `cargo-workspaces` needed)
# famp/Cargo.toml (root)
# Pin once, reference from member crates via { workspace = true }
### `just` — task runner
# Justfile
## 14. Lint / format
### `rustfmt`
### `clippy` — strict settings
# Noisy pedantic lints we don't want:
## 15. CI — GitHub Actions
# .github/workflows/ci.yml
- `dtolnay/rust-toolchain@stable` — canonical toolchain installer
- `Swatinem/rust-cache@v2` — target-dir caching (huge speedup)
- `taiki-e/install-action@v2` — binary installer that avoids compiling `cargo-nextest` from source every run
- `rustsec/audit-check@v2` — `cargo audit` advisory scanner
## Installation commands (Phase 0 bootstrap)
# Rust toolchain
# Dev tools (one-time global installs)
# Pin toolchain for reproducibility
## Alternatives summary
| Recommended | Alternative | When alternative wins |
|---|---|---|
| `ed25519-dalek` | `ring` | FIPS compliance required |
| `ed25519-dalek` | `ed25519-compact` | `no_std` embedded target |
| `serde_jcs` | Write `famp-canonical` from scratch | Conformance gate fails on RFC 8785 vectors (you'll know Phase 2) |
| `serde_json` | `simd-json` / `sonic-rs` | >10k msg/sec ingest; never relevant for FAMP |
| `axum` | `actix-web` | Actor-model UI apps; not protocol servers |
| `axum` | Raw `hyper` | You need custom HTTP/2 flow control |
| `reqwest` | `hyper-util` client | Fine-grained connection pool control |
| `rustls` | `native-tls` | OpenSSL-only federation requirement |
| `tokio` | `smol` / `async-std` | Never, in 2026 |
| `stateright` | Hand-written exhaustive BFS | `stateright` goes unmaintained |
| `just` | `make` / `cargo-make` | Never (just is strictly better) |
## What NOT to Use
| Avoid | Why | Use instead |
|---|---|---|
| `openssl` crate | C FFI, build complexity, supply-chain surface | `rustls` + RustCrypto |
| `native-tls` | Same | `rustls` |
| `rustls-native-certs` | Deprecated path | `rustls-platform-verifier` |
| `canonical-json` crates on crates.io (e.g. `canonical_json`, `olpc-cjson`) | Implement **OLPC/Matrix** canonical JSON, **not** RFC 8785. Will silently produce wrong signatures. | `serde_jcs` (wrapped in `famp-canonical`) |
| `ed25519-dalek 1.x` tutorials | `Keypair` API renamed to `SigningKey` in 2.x | Only use docs.rs for 2.2 |
| `base64::encode` / `base64::decode` free functions | Removed in 0.21+ | `URL_SAFE_NO_PAD.encode()` engine API |
| `serde_json` `arbitrary_precision` feature | Changes number canonicalization; breaks JCS | Default serde_json |
| `serde_json` `preserve_order` feature | Canonical output must be order-independent | Default serde_json |
| `quickcheck` | Older, less ergonomic than proptest | `proptest` |
| `async-std` / `smol` | Not compatible with axum/reqwest/hyper 1.x ecosystem | `tokio` |
| `cargo test` for FSM work | No process isolation; slower | `cargo nextest run` |
| `--no-verify` / `continue-on-error` in CI | Hides the one bit that matters (byte-exact canonicalization) | Fix the underlying issue |
## Version compatibility notes
| A | B | Note |
|---|---|---|
| `sha2 0.11` | `digest 0.11` | Must upgrade together; `cargo tree -d` to check for `sha2 0.10` duplicates pulled by transitive deps |
| `ed25519-dalek 2.x` | `rand_core 0.6` | dalek 2.x still uses rand_core 0.6 API; do not upgrade to `rand_core 0.9` in crates that share keys with dalek |
| `axum 0.8` | `hyper 1.x` / `http 1.x` | axum 0.7+ is on the hyper-1.x train; do not mix with `hyper 0.14` code from older tutorials |
| `reqwest 0.12+` | `hyper 1.x` | Same transition as axum; reqwest 0.11 is hyper 0.14, incompatible |
| `rustls 0.23` | `tokio-rustls 0.26` | Paired upgrade; check before bumping either |
| `serde_jcs 0.2` | `serde_json 1.x` | Fine today; monitor for divergence if either does a major bump |
## Rust-beginner friction map
| Area | Beginner difficulty | Notes |
|---|---|---|
| `ed25519-dalek` | ★☆☆ Easy | Constructor + `sign`/`verify`, done |
| `serde` derive | ★☆☆ Easy | `#[derive(Serialize, Deserialize)]`, done — until it isn't (lifetimes) |
| `serde_jcs` | ★★☆ Medium | API is small; understanding *why* we gate it with RFC 8785 vectors is the hard part |
| `axum` extractors | ★★☆ Medium | Trait bounds in error messages are intimidating at first |
| `tokio` async | ★★★ Hard | `.await`, `Send` bounds, spawning — the #1 Rust learning cliff |
| `thiserror` | ★☆☆ Easy | Derive macro; follow the pattern |
| `proptest` | ★★☆ Medium | Strategy composition takes practice |
| **`stateright`** | ★★★ Hard | Budget a full day to understand the Actor/Model abstraction |
| Lifetimes in public APIs | ★★★ Hard | Defer by using owned types (`String`, `Vec<u8>`) in crate boundaries; optimize to `&str`/`&[u8]` later |
## Confidence assessment per choice
| Dimension | Confidence | Source |
|---|---|---|
| Crate selections (items 1, 3–11, 13–15) | HIGH | crates.io live API 2026-04-12 + ecosystem consensus |
| Version numbers | HIGH | Fetched live from crates.io API on research date |
| `serde_jcs` correctness for RFC 8785 edge cases | **MEDIUM** | Single-maintainer, self-labeled "unstable"; MUST be gated by conformance test vectors |
| `stateright` long-term maintenance | MEDIUM | ~9 months since last release as of research date; still the only option |
| `base64 0.22` staleness | HIGH | Last release April 2024 but feature-complete; no risk |
| CI action versions | MEDIUM | Not individually re-verified today; widely used, stable |
## Sources
- **crates.io JSON API** (live fetch 2026-04-12) — authoritative version/update-date data for every crate listed
- **lib.rs/crates/serde_jcs** — dependent count (34 direct), maintainer identity, "unstable" label, ryu-js dependency note
- **RFC 8785** (IETF, JSON Canonicalization Scheme) — test vectors for conformance gate: <https://datatracker.ietf.org/doc/html/rfc8785>
- **RFC 9562** (UUID) — UUIDv7 specification
- **FAMP v0.5 spec §7.1, §14.3, INV-10** — signature-over-canonical-JSON requirement
- **docs.rs** (implicit) — for every crate, the "latest" URL is canonical
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

**FAMP today is local-first** (v0.9): a UDS-backed broker for same-host agent
messaging. **FAMP at v1.0 is federated**: cross-host messaging via
`famp-gateway` wrapping the local bus. See [ARCHITECTURE.md](ARCHITECTURE.md)
for the full layered model (Layer 0 protocol primitives -> Layer 1 local bus ->
Layer 2 federation gateway).

In v0.8 the federation transport used `famp listen` HTTPS daemons with
TOFU-pinned peers; v0.9 replaces this with the local bus. Every federation
wire envelope stayed Ed25519-signed over canonical JSON under the
`FAMP-sig-v1\0` domain prefix (INV-10). 5-state task FSM (`famp-fsm`):
REQUESTED -> COMMITTED -> {COMPLETED | FAILED | CANCELLED}, terminals
absorbing.

Note: as of v0.8.x (the session-bound MCP identity bridge phase), the
`famp mcp` server reads identity from session state via `famp_register`,
not from `FAMP_HOME`. The v0.8 federation transport used `FAMP_HOME` per
identity; v0.9's local bus collapses this distinction.

**v0.9 shipping path:** collapse same-host agents onto a single
UDS-backed broker; drop crypto on the local path; treat federation
(cross-host) as a v1.0 gateway that wraps the bus. IRC-style channels,
durable per-name mailboxes, stable MCP tool surface across v0.8 / v0.9 / v1.0.

**v1.0 readiness trigger (named):** v1.0 federation milestone fires
when Sofer (or a named equivalent) runs FAMP from a different machine
and exchanges a signed envelope. If 4 weeks pass after v0.9.0 ships
with no movement on this trigger, federation framing is reconsidered.
Concrete forcing function for the local-case-black-hole risk; the
conformance vector pack ships at the same trigger (deferred from
v0.5.1 wrap, see `.planning/WRAP-V0-5-1-PLAN.md` DEFERRED banner).

Full write-up in [`ARCHITECTURE.md`](ARCHITECTURE.md) and the design spec
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
Pre-v0.9 scaffolding moved to
[`docs/history/v0.9-prep-sprint/famp-local/famp-local`](docs/history/v0.9-prep-sprint/famp-local/famp-local).

**When working here:** protocol-primitive crates (`famp-canonical`,
`famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope`) are
transport-neutral and reused across both v0.9 and v1.0. Transport crates
(`famp-transport-http`, `famp-keyring`) are v1.0-federation internals —
don't conflate them with the primitive layer.
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
