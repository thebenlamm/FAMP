# Stack Research — FAMP v0.5 Rust Reference Implementation

**Domain:** Signed-message protocol library (crypto + canonical JSON + async HTTP + FSM model checking)
**Researched:** 2026-04-12
**Overall confidence:** HIGH (versions verified live against crates.io API on research date)

> **Versioning convention below.** For mature 1.x+ crates we pin to the current minor (`^1.51`). For pre-1.0 crates, a minor bump is a breaking change, so we pin to the current minor explicitly (`=0.8.8`-style is overkill; `^0.8.8` behaves correctly under Cargo semver rules for 0.x). The roadmap should generate a `Cargo.toml` from this file.

---

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

---

## 1. Ed25519 signing — `ed25519-dalek 2.2.0`

**Pick:** `ed25519-dalek = "2.2"`, features `["rand_core", "zeroize"]` (optionally `"pem"` for Agent Card import).

**Why:**
- Pure-Rust, RustCrypto ecosystem (same org as `sha2`, `curve25519-dalek`), 35M recent downloads.
- API matches exactly what FAMP §7.1 needs: `SigningKey::from_bytes(&[u8; 32])`, `sign(msg)` → `Signature` (64 bytes), `VerifyingKey::verify_strict(msg, &sig)`.
- `verify_strict` rejects non-canonical / small-order-point signatures — **this is what you want for protocol-level non-repudiation.** Plain `verify` is legacy-tolerant and should not be used.
- Key/sig wire format is raw bytes (32 pub, 64 sig), which matches the spec's "raw 32-byte pub / 64-byte sig, base64url" decision.
- Works on stable Rust. No C deps. Reproducible builds.

**Why not alternatives:**
- **`ring`** — fast, FIPS-ish, but opinionated: exposes only high-level API, won't compose cleanly with the RustCrypto `signature::Signer` trait we'll use in `famp-crypto`'s trust abstraction. Also has its own cbindgen-generated asm that complicates cross-compilation. Reserve for a future FIPS profile, not v1.
- **`ed25519-compact`** — smaller, no_std, but single-maintainer and less audited.
- **`RustCrypto/ed25519`** is the *trait* crate; `ed25519-dalek` is the implementation. You need both (dalek pulls in the trait).

**Beginner note:** `SigningKey` in dalek 2.x is the renamed `Keypair` from 1.x — any tutorial older than 2023 uses the old name. Stick to docs.rs for `2.2.0`.

---

## 2. Canonical JSON / RFC 8785 JCS — `serde_jcs 0.2.0` **with a safety net**

**This is the single highest-risk dependency in the project.** INV-10 + §14.3 mean a one-byte disagreement breaks the entire protocol. Read this section carefully.

**Pick:** `serde_jcs = "0.2"` *plus* an in-house conformance test suite that runs the official RFC 8785 test vectors against it on every CI build. **If it fails any vector, we fork it into `famp-canonical` immediately.**

**State of the ecosystem (verified 2026-04-12):**
- `serde_jcs 0.2.0` — published 2026-03-25, 34 direct dependents, self-labeled "unstable", maintained by `l1h3r`. Implements RFC 8785 via Serde. Uses `ryu-js` for JSON number serialization (required by RFC 8785, which mandates ECMAScript `Number.prototype.toString` semantics for numbers — a notorious corner case).
- No other widely-maintained Rust JCS crate exists. `json-canon` is abandoned. `canonical-json` crates exist for OLPC/Matrix canonical JSON (a *different, incompatible* spec) — **do not use them**.
- RustCrypto does not provide one.

**Why `serde_jcs` despite "unstable":**
- It is literally the only serde-integrated RFC 8785 implementation.
- The "unstable" label is about API churn, not correctness. 0.2.0 shipping two weeks before our research is a good sign, not a bad one.
- Forking is a 500-line job if needed (sort keys, RFC 8785 number formatter, UTF-8 pass-through). The `ryu-js` dep does the hardest part.

**Mandatory safety net (must be in Phase 2):**

1. Create `famp-canonical` as a wrapper crate, **not** a direct dep re-export. All FAMP code imports `famp_canonical::to_canonical_bytes()`. This gives us one spot to swap implementations.
2. In `famp-canonical/tests/rfc8785_vectors.rs`, import the official RFC 8785 test vectors (from <https://github.com/cyberphone/json-canonicalization/tree/master/testdata>) as git submodule or vendored fixtures.
3. CI gate: any canonicalization byte-diff from vectors = red build, no exceptions.
4. Additional custom vectors for FAMP-specific shapes: nested maps with non-ASCII keys, numbers at `Number.MAX_SAFE_INTEGER` boundary, empty arrays/objects, null values, duplicate-key rejection (JCS disallows).
5. **Fuzz test** (proptest): generate arbitrary `serde_json::Value`, canonicalize, parse canonical output, re-canonicalize, assert byte equality (idempotency).

**If `serde_jcs` fails any of the above**, the fallback is:

> **`famp-canonical` becomes a from-scratch implementation.** ~500 LoC: walk `serde_json::Value`, sort object keys by UTF-16 code unit, reject duplicates, format numbers via `ryu-js`, escape strings per RFC 8785 §3.2.2.2, no whitespace. Write the code in Phase 2 either way — the wrapper crate + tests exist regardless.

**Beginner note:** This is the one place where "the crate exists, let's use it" is not safe. You will review JSON number edge cases yourself. The spec requires byte-exactness across implementations; we do not get to trust the upstream on faith.

Confidence: **MEDIUM** (the dep itself). The *approach* (wrapper + conformance gate + documented fallback) is HIGH confidence.

---

## 3. Serde + JSON — `serde 1.0.228` + `serde_json 1.0.149`

**Pick:** `serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`.

**Why:**
- `serde_json` is the reference implementation, maintained by `dtolnay`, and is what `serde_jcs` is built on. Any non-serde_json path means rewriting canonicalization.
- Preserves `Number` precision as `serde_json::Number`; supports `arbitrary_precision` feature if needed (NOT recommended for FAMP — it changes canonicalization behavior).

**Why NOT SIMD alternatives:**
- **`simd-json 0.17`** and **`sonic-rs 0.5.8`** are real speedups for *parsing throughput* in high-QPS ingest paths. FAMP is not that. Messages are ≤ a few KB; signature verification dominates CPU; SIMD JSON parsing is irrelevant.
- Neither has a JCS canonicalizer. Using them for parsing + `serde_json` for canonicalization risks a double-parse divergence. **One JSON library, one source of truth.**
- `sonic-rs` requires nightly for some features. Beginner-unfriendly.

**`serde_json` features to enable:** *none* beyond default. Explicitly **disable** `arbitrary_precision` and `preserve_order` (they change semantics — canonical output must not depend on input key order).

---

## 4. UUIDv7 — `uuid 1.23.0`

**Pick:** `uuid = { version = "1.23", features = ["v7", "serde"] }`. Add `"fast-rng"` if you want `getrandom` → `rand` acceleration.

**Why:**
- UUIDv7 (time-ordered) is the right choice for FAMP conversation/task IDs: database-friendly index locality, debuggable (timestamp is visible in the first 48 bits), still globally unique with 74 bits of entropy.
- The `uuid` crate is the canonical Rust implementation. `v7` feature stabilized in 1.11; 1.23 is current.
- `serde` feature gives `Serialize`/`Deserialize` as the canonical hyphenated string form, which is what the spec shows.

**Don't do:**
- Don't invent your own "timestamp + random hex" scheme. RFC 9562 UUIDv7 is already the answer.
- Don't use `v4` (random only) — you lose time ordering for free.

---

## 5. Base64url unpadded — `base64 0.22.1`

**Pick:** `base64 = "0.22"`, use the `URL_SAFE_NO_PAD` engine:

```rust
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
let s = URL_SAFE_NO_PAD.encode(sig_bytes);
let bytes = URL_SAFE_NO_PAD.decode(&s)?;
```

**Why:**
- `base64` is the canonical Rust base64 crate; `URL_SAFE_NO_PAD` matches JOSE/JWT/Matrix conventions and what the spec shows (`base64url` unpadded). The 0.22 release stabilized the `Engine` API — any tutorial using `base64::encode_config(...)` is pre-0.21 and will not compile.
- **Strict decoding by default:** `URL_SAFE_NO_PAD` rejects trailing padding AND mixed alphabets. Critical for signature integrity — a non-canonical base64 input must not round-trip.

**Why not `data-encoding`:**
- Good crate, more general, but `base64` is smaller, more idiomatic, and is what every other Rust crypto crate in the ecosystem uses. Fewer surprises for a beginner.

**Beginner note:** `base64` hasn't been updated since April 2024 — this is fine. It is feature-complete and the API is stable. "Stale" ≠ "unmaintained" for small focused crates.

---

## 6. SHA-256 — `sha2 0.11.0`

**Pick:** `sha2 = "0.11"`.

```rust
use sha2::{Sha256, Digest};
let hash = Sha256::digest(bytes);  // GenericArray<u8, 32>
let hex = format!("sha256:{:x}", hash);
```

**Why:**
- RustCrypto's standard hash crate, same org as `ed25519-dalek`. Pure Rust, stable, auditable.
- Version 0.11.0 stabilized in March 2026 after ~9 months of RCs — it's the current major for the RustCrypto traits rework. All new code should target 0.11 directly; do not start on 0.10.x.
- Matches spec artifact-id scheme: `sha256:<hex>`.

**Why not `ring`:**
- Same reason as ed25519: `ring` is opinionated, couples hashing to a fixed crypto universe, and doesn't compose with the `digest` trait used by the rest of the RustCrypto ecosystem.

**Version compatibility note:** `sha2 0.11` requires `digest 0.11`. If another crate in your tree pulls in `sha2 0.10`, you'll get two versions compiled side-by-side. Run `cargo tree -d` after every dependency add to catch duplicates.

---

## 7. HTTP server — `axum 0.8.8`

**Pick:** `axum = "0.8"`, `tower = "0.5"`, `tower-http = "0.6"` (for `TraceLayer`, `CorsLayer`, `LimitLayer`).

**Why:**
- De facto standard 2025–2026 Rust web framework. Built on `hyper` 1.x + `tokio`. Maintained by the `tokio-rs` org itself.
- Handler-function ergonomics with extractors — closest thing to "Flask but type-checked" in Rust. Much friendlier than `actix-web`'s actor model for a beginner.
- Composes with `tower` middleware stack, which is how you implement FAMP's signature-verification middleware as a reusable layer.
- `0.8` is current; `0.8.8` shipped December 2025. Stable API since 0.7.

**Why not alternatives:**
- **`actix-web`** — fast, mature, but unsafe internals, heavier learning curve (actor model), and smaller middleware ecosystem. Not the modern default.
- **`warp`** — filter-combinator model. Cool, but filter type errors are famously horrible to debug. Not beginner-friendly.
- **`hyper` directly** — you *can* write a server on raw hyper 1.x, but you'll reinvent routing and extractors. `axum` is the thin layer that makes hyper ergonomic.
- **`rocket`** — requires nightly historically; usage declining.

**Middleware stack for `famp-transport`:**
1. `tower_http::trace::TraceLayer` — structured request logging
2. `tower_http::limit::RequestBodyLimitLayer` — cap at e.g. 1 MB
3. Custom `axum::middleware::from_fn` — verify Ed25519 signature *before* routing (fail-closed per INV-10)
4. Router with one `POST /famp/v1/envelope` handler

---

## 8. HTTP client — `reqwest 0.13.2`

**Pick:** `reqwest = { version = "0.13", default-features = false, features = ["json", "rustls-tls-native-roots", "http2"] }`

**Why:**
- Highest-level Rust HTTP client. Built on `hyper` + `tokio`. Same org as axum/tokio ecosystem.
- Disable default features to avoid pulling `native-tls` (which pulls OpenSSL on Linux — a portability nightmare).
- `rustls-tls-native-roots` uses the OS trust store but the `rustls` TLS stack. Best of both worlds.

**Why not `hyper-util` directly:**
- `hyper-util` is a low-level client builder. You'd reimplement connection pooling, redirects, and timeouts. `reqwest` wraps all that cleanly.
- Use hyper-util only if you need fine control over the connection pool or zero-copy body streaming. FAMP doesn't.

---

## 9. TLS — `rustls 0.23.38`

**Pick:** indirectly via `reqwest`'s `rustls-tls-native-roots` feature and `axum`'s `axum-server = { version = "0.7", features = ["tls-rustls"] }` (or `rustls-acme` if you want automatic ACME certs — probably overkill for v1).

```toml
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
```

**Why:**
- `rustls` is now the default TLS in the Rust ecosystem. Pure-Rust, no OpenSSL, memory-safe.
- `0.23.38` shipped 2026-04-12 (literally today). Version 0.23 has been the stable line since early 2024; point releases are security-only.
- **Crypto provider choice:** `ring` (default, battle-tested) vs `aws-lc-rs` (FIPS-targeted, newer). **Pick `ring` for v1.** Switch to `aws-lc-rs` only if a federation requires FIPS.
- Add `rustls-platform-verifier = "0.5"` for OS-trust-store integration on client side. This replaces the old `rustls-native-certs` dance.

**Certificate management for the reference HTTP transport:**
- For dev: self-signed certs via `rcgen = "0.14"`.
- For federation deployments: each federation provides its own trust anchor list — document this as a `Vec<TrustAnchor>` loaded at startup.

---

## 10. Async runtime — `tokio 1.51.1`

**Pick:** `tokio = { version = "1.51", features = ["full"] }` in binaries/tests; `tokio = { version = "1.51", features = ["rt", "macros"] }` in library crates (narrower feature surface).

**Why:**
- Only serious choice. `async-std` is effectively abandoned. `smol` is niche. `tokio` is what axum/reqwest/hyper/rustls all assume.
- 1.51 is current stable; 1.x has upheld semver since 2020 — no breaking changes.

**Library hygiene rule:** Don't enable `features = ["full"]` in library crates. It forces every downstream to compile every tokio module. Enable only what each crate needs.

---

## 11. Error handling — `thiserror 2.0.18` (libs) + `anyhow 1.0.102` (bins/tests only)

**Pick:** `thiserror = "2"` in every library crate. `anyhow = "1"` only in CLI binaries and integration tests.

**Why:**
- `thiserror` generates `Display` + `From` + `Error` impls for typed errors. Libraries must expose typed errors so callers can `match` — this is doubly true for FAMP because the spec's "reject with `unauthorized`" is compiler-checked when errors are enums.
- `anyhow::Error` is an untyped boxed error — great for `main()`, terrible for library APIs. Never return `anyhow::Result` from a `famp-*` crate.
- `thiserror 2.x` is the current major (released early 2025); purely additive changes from 1.x.

**FAMP error enum skeleton (for `famp-core`):**
```rust
#[derive(Debug, thiserror::Error)]
pub enum FampError {
    #[error("invalid canonical form")]
    Canonical(#[from] famp_canonical::Error),
    #[error("signature verification failed")]
    Unauthorized,
    #[error("stale message: {reason}")]
    Stale { reason: String },
    #[error("invariant {name} violated")]
    InvariantViolation { name: &'static str },
    // ... one variant per spec §24 error code
}
```

---

## 12. Testing

### `proptest 1.11.0` — property-based tests
**Use for:** canonical-JSON idempotency, signature round-trip, envelope encode/decode fuzzing, FSM transition generators. Standard Rust choice; `quickcheck` is the older, less ergonomic alternative — skip it.

### `stateright 0.31.0` — state-machine model checking
**Use for:** `famp-fsm` exhaustive exploration of conversation + task FSMs to validate INV-5 (single terminal state) against arbitrary event interleavings.

**Confidence: MEDIUM.** `stateright` is *the* Rust model checker, but its last release was 2025-07-27 (~9 months stale as of research date). It still works; the API is stable; small community. **Risk:** if it goes unmaintained before our release, we'd need to either pin a git SHA and maintain our own patches, or replace it with custom brute-force BFS over the FSM state space (a few hundred LoC — doable but painful). Treat it as "fine for v1, monitor for v2."

**Beginner note:** `stateright` has a learning curve. Budget explicit research time in the `famp-fsm` phase; don't expect to be productive on it day 1.

### `insta 1.47.2` — snapshot testing
**Use for:** the conformance test vectors (published JSON fixtures). Every canonical serialization, every signature, every FSM transition table — snapshot it, commit the snapshot, treat the `.snap` files as the published cross-implementation contract.

```bash
cargo install cargo-insta  # gives `cargo insta review` TUI
```

### Test runner: `cargo-nextest 0.9.132`
Faster than `cargo test`, per-test process isolation (important for anything that touches global state — e.g., env vars for TLS cert paths), better JUnit output for CI.

```bash
cargo install cargo-nextest --locked
```

---

## 13. Workspace / build tooling

### Cargo workspace (no `cargo-workspaces` needed)
Stdlib `Cargo.toml` workspace is sufficient. Use `workspace.dependencies` to pin versions in one place:

```toml
# famp/Cargo.toml (root)
[workspace]
resolver = "2"
members = [
    "crates/famp-canonical",
    "crates/famp-crypto",
    "crates/famp-core",
    "crates/famp-envelope",
    "crates/famp-identity",
    "crates/famp-causality",
    "crates/famp-fsm",
    "crates/famp-negotiate",
    "crates/famp-delegate",
    "crates/famp-provenance",
    "crates/famp-extensions",
    "crates/famp-transport",
]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "Apache-2.0"
repository = "https://github.com/..."

[workspace.dependencies]
# Pin once, reference from member crates via { workspace = true }
ed25519-dalek = { version = "2.2", features = ["rand_core", "zeroize"] }
serde         = { version = "1.0.228", features = ["derive"] }
serde_json    = "1.0.149"
serde_jcs     = "0.2.0"
sha2          = "0.11"
base64        = "0.22"
uuid          = { version = "1.23", features = ["v7", "serde"] }
thiserror     = "2.0"
tokio         = { version = "1.51", features = ["rt", "macros"] }
axum          = "0.8.8"
reqwest       = { version = "0.13.2", default-features = false, features = ["json", "rustls-tls-native-roots", "http2"] }
rustls        = { version = "0.23", default-features = false, features = ["ring", "std", "tls12"] }
proptest      = "1.11"
stateright    = "0.31"
insta         = { version = "1.47", features = ["json"] }
```

**`cargo-workspaces`** (the external crate) is for publishing multi-crate releases to crates.io with one command. Useful for v1.0 release day. Not needed during development. Skip until we're ready to publish.

### `just` — task runner
`Justfile` in the repo root:

```makefile
# Justfile
default:
    @just --list

fmt:            cargo fmt --all
check:          cargo check --workspace --all-targets
lint:           cargo clippy --workspace --all-targets -- -D warnings
test:           cargo nextest run --workspace
test-vectors:   cargo nextest run --workspace --features conformance-vectors
fuzz-canon:     cargo test -p famp-canonical --release -- --ignored proptest
model-check:    cargo test -p famp-fsm --release -- --ignored stateright
ci:             just fmt check lint test
```

**Why `just` over `make`:** no tab-sensitivity, cross-platform, saves newcomers a day of frustration.

---

## 14. Lint / format

### `rustfmt`
Use default settings. Add a minimal `rustfmt.toml` only for:
```toml
edition = "2024"
max_width = 100
imports_granularity = "Crate"  # nightly-only; skip if you want stable-only
```

### `clippy` — strict settings
Add to the workspace root `Cargo.toml`:
```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
all        = { level = "deny", priority = -1 }
pedantic   = { level = "warn", priority = -1 }
nursery    = { level = "warn", priority = -1 }
# Noisy pedantic lints we don't want:
module_name_repetitions = "allow"
must_use_candidate      = "allow"
```

Per-crate lints inherit from the workspace via `[lints] workspace = true`.

**`unsafe_code = "forbid"`** is non-negotiable for a security protocol library. If any future need for unsafe arises, it's a design review moment.

---

## 15. CI — GitHub Actions

Minimal matrix for v1 (one OS, one toolchain to start; expand post-validation):

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        toolchain: [stable]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive  # for RFC 8785 test vectors
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest,just
      - run: just fmt --check
      - run: just lint
      - run: just test
      - run: just test-vectors  # RFC 8785 conformance gate

  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v2
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Key actions (versions verified in common use):**
- `dtolnay/rust-toolchain@stable` — canonical toolchain installer
- `Swatinem/rust-cache@v2` — target-dir caching (huge speedup)
- `taiki-e/install-action@v2` — binary installer that avoids compiling `cargo-nextest` from source every run
- `rustsec/audit-check@v2` — `cargo audit` advisory scanner

**Windows is deliberately absent from v1.** Add when there's a concrete user need; it will expose path/line-ending issues in canonicalization tests that we don't need on the critical path.

---

## Installation commands (Phase 0 bootstrap)

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustup component add rustfmt clippy

# Dev tools (one-time global installs)
cargo install cargo-nextest --locked
cargo install cargo-insta --locked
cargo install cargo-audit --locked
brew install just  # or: cargo install just --locked

# Pin toolchain for reproducibility
echo '[toolchain]
channel = "1.87"
components = ["rustfmt", "clippy"]
profile = "minimal"' > rust-toolchain.toml
```

---

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

---

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

---

## Version compatibility notes

| A | B | Note |
|---|---|---|
| `sha2 0.11` | `digest 0.11` | Must upgrade together; `cargo tree -d` to check for `sha2 0.10` duplicates pulled by transitive deps |
| `ed25519-dalek 2.x` | `rand_core 0.6` | dalek 2.x still uses rand_core 0.6 API; do not upgrade to `rand_core 0.9` in crates that share keys with dalek |
| `axum 0.8` | `hyper 1.x` / `http 1.x` | axum 0.7+ is on the hyper-1.x train; do not mix with `hyper 0.14` code from older tutorials |
| `reqwest 0.12+` | `hyper 1.x` | Same transition as axum; reqwest 0.11 is hyper 0.14, incompatible |
| `rustls 0.23` | `tokio-rustls 0.26` | Paired upgrade; check before bumping either |
| `serde_jcs 0.2` | `serde_json 1.x` | Fine today; monitor for divergence if either does a major bump |

---

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

**Beginner strategy:** in Phase 0–2, use owned types everywhere in public APIs. Accept the extra allocations. You can tighten lifetimes in a dedicated cleanup phase once the full workspace compiles and tests pass.

---

## Confidence assessment per choice

| Dimension | Confidence | Source |
|---|---|---|
| Crate selections (items 1, 3–11, 13–15) | HIGH | crates.io live API 2026-04-12 + ecosystem consensus |
| Version numbers | HIGH | Fetched live from crates.io API on research date |
| `serde_jcs` correctness for RFC 8785 edge cases | **MEDIUM** | Single-maintainer, self-labeled "unstable"; MUST be gated by conformance test vectors |
| `stateright` long-term maintenance | MEDIUM | ~9 months since last release as of research date; still the only option |
| `base64 0.22` staleness | HIGH | Last release April 2024 but feature-complete; no risk |
| CI action versions | MEDIUM | Not individually re-verified today; widely used, stable |

---

## Sources

- **crates.io JSON API** (live fetch 2026-04-12) — authoritative version/update-date data for every crate listed
- **lib.rs/crates/serde_jcs** — dependent count (34 direct), maintainer identity, "unstable" label, ryu-js dependency note
- **RFC 8785** (IETF, JSON Canonicalization Scheme) — test vectors for conformance gate: <https://datatracker.ietf.org/doc/html/rfc8785>
- **RFC 9562** (UUID) — UUIDv7 specification
- **FAMP v0.5 spec §7.1, §14.3, INV-10** — signature-over-canonical-JSON requirement
- **docs.rs** (implicit) — for every crate, the "latest" URL is canonical

---
*Stack research for: FAMP v0.5 Rust reference implementation*
*Researched: 2026-04-12*
