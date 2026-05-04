# Phase 4: Federation CLI unwire + federation-CI preservation — Pattern Map

**Mapped:** 2026-05-03
**Files analyzed:** 6 CREATE + 7 significant MODIFY + 27 MOVE
**Analogs found:** 6 / 6 CREATE files have strong analogs; all 7 MODIFY files have in-tree precedent; MOVE operations are mechanical (`git mv`).

---

## File Classification

### CREATE (new files needing analog patterns)

| New File | Role | Data Flow | Closest Analog | Match Quality |
|----------|------|-----------|----------------|---------------|
| `crates/famp/tests/e2e_two_daemons.rs` (refactor) | integration-test | request-response (HTTPS happy path) | `crates/famp/tests/http_happy_path.rs` | **EXACT** (CONTEXT.md "Reusable Assets" + Audit 6 explicitly name it the template) |
| `crates/famp/tests/e2e_two_daemons_adversarial.rs` (or sibling `#[test]` row) | integration-test (sentinel) | request-response (rejection at middleware) | `crates/famp/tests/adversarial/http.rs` | **EXACT** (Audit 6 + D-09 reuse the "handler-closure-not-entered" sentinel) |
| `crates/famp/tests/_deferred_v1/README.md` | docs (freeze explainer) | n/a | `docs/history/README.md` | role-match (history-archive narrative + stale-link advisory) |
| `docs/MIGRATION-v0.8-to-v0.9.md` | docs (migration table) | n/a | RESEARCH Audit 9 skeleton (no in-tree migration doc precedent) | partial (skeleton drafted; tone matches `docs/history/README.md`) |
| `docs/history/v0.9-prep-sprint/famp-local/README.md` | docs (one-line freeze marker) | n/a | `docs/history/README.md` opening paragraph | role-match (terse archive marker; D-14 dictates one-line shape) |
| `crates/famp/tests/cli_help_invariant.rs` (optional) | smoke-test (CLI help invariant) | request-response (cargo-bin subprocess) | `crates/famp/tests/cli_dm_roundtrip.rs` (the `Bus::famp_cmd` helper) + `assert_cmd::cargo::CommandCargoExt` pattern | role-match (only existing `cargo_bin("famp")` precedent) |

### MODIFY (significant edits with in-tree precedent)

| File | Edit Class | Closest Precedent | Match Quality |
|------|-----------|-------------------|---------------|
| `crates/famp/src/cli/mod.rs` | enum-variant + dispatch-arm + `pub mod` removals | own structure (additive prior commits like `feat(02): add register/join/leave`) | role-match — no pure-removal precedent at this scale |
| `crates/famp/src/cli/info.rs` | inline `PeerCard` struct + private `load_identity` to drop cross-module deps (Risk #1) | `crates/famp/src/cli/whoami.rs` (small self-contained subcommand with its own `Outcome` struct) | role-match |
| `crates/famp/src/cli/send/mod.rs` | drop two `pub mod` lines (`client`, `fsm_glue`) | own line 57–58 (additive declarations from Phase 2 plan 02-04) | exact (negate-add) |
| `crates/famp/src/cli/error.rs` | drop `Tls(#[from] famp_transport_http::TlsError)` variant | own enum (line 70–71); the variant is the only `famp_transport_http`-typed payload | exact |
| `crates/famp/src/lib.rs` + `crates/famp/src/bin/famp.rs` | drop `use _ as _;` silencer lines + `pub mod runtime;` | own file structure | exact |
| Workspace `Cargo.toml` | comment relabel above `crates/famp-keyring` and `crates/famp-transport-http` member entries | existing comment style around `crates/famp-canonical` (no per-member comments — comment is a clean addition) | partial (no per-member comment precedent; ~5-word comment is novel but uniform) |
| `README.md`, `CLAUDE.md`, `.planning/ROADMAP.md`, `ARCHITECTURE.md`, `.planning/MILESTONES.md` | staged-framing edits + delete federation-CLI tutorial | Phase 3 D-13 already landed staged framing in ARCHITECTURE.md (per CONTEXT.md "ARCHITECTURE.md already has the staged framing"); Phase 3 README/CLAUDE/MILESTONES updates form the closest precedent | role-match |
| `.planning/REQUIREMENTS.md` (FED/MIGRATE/TEST-06/CARRY-01 checkbox flips) | `[ ] → [x]` + inline SHA reference | every prior phase closing commit (`docs(03-06): record stop hook UAT pass` etc.) | exact |

### MOVE (`git mv` only — no new patterns; just verify destinations)

| Source | Destination | Mechanics |
|--------|-------------|-----------|
| ~27 federation-tagged tests (Audit 2 + 2.5) | `crates/famp/tests/_deferred_v1/<file>` | `git mv` per file; **drop `#[ignore = "Phase 04 ..."]` line on each moved file** (no longer in active set). |
| `scripts/famp-local` (single 1316-LOC bash file, NOT a directory) | `docs/history/v0.9-prep-sprint/famp-local/famp-local` | `mkdir -p` parent first; `git mv` the script as a file; add the README in the same commit. |

---

## Pattern Assignments

### `crates/famp/tests/e2e_two_daemons.rs` (integration-test, request-response)

**Analog:** `crates/famp/tests/http_happy_path.rs` (162 lines — the entire file IS the template).

**Header / lint suppressions** (lines 1–14):

```rust
//! Same-process HTTP happy path — runs alice and bob as tokio tasks against
//! 127.0.0.1:<ephemeral> with real rustls TLS using committed fixture certs.
//! ...
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::similar_names,
    clippy::significant_drop_tightening,
    clippy::doc_markdown,
    unused_crate_dependencies
)]
```

For Phase 4: same suppression block; module doc-comment changes to:

```rust
//! Phase 4 plumb-line-2 insurance: target famp-transport-http library API
//! directly to keep the federation HTTPS path exercised in `just ci`.
//! See ARCHITECTURE.md and `crates/famp/tests/_deferred_v1/README.md`.
```

**`#[path]` cycle-driver pattern** (lines 16–19):

```rust
// Pull the cycle_driver via #[path] so this test binary and the example
// consume the SAME driver implementation.
#[path = "common/cycle_driver.rs"]
mod cycle_driver;
```

Reuse verbatim — D-12 conversation shape (`request → commit → deliver → ack`) is exactly what `cycle_driver` drives; Audit 6 confirms `tests/common/cycle_driver.rs` exists and is the proven driver.

**Imports pattern** (lines 21–31):

```rust
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};
use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;
use famp_transport_http::{build_router, tls, tls_server, HttpTransport};
use url::Url;
```

Copy verbatim. These are the exact deps the refactor needs.

**Fixture-cert helper** (lines 33–38):

```rust
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}
```

D-11 locks fixture reuse — copy this helper verbatim.

**TLS-listener-ready settle helper** (lines 40–43):

```rust
async fn wait_for_tls_listener_ready() {
    tokio::task::yield_now().await;
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;
}
```

Copy verbatim.

**Async runtime + happy-path test signature** (lines 45–46):

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_happy_path_same_process() {
```

For Phase 4 rename to `e2e_two_daemons_happy_path` (or similar) — same `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` attributes; same `worker_threads = 2` matches D-10 (same runtime, two `tokio::spawn` listener tasks).

**Two-keyring + two-transport setup** (lines 47–119):

```rust
// Keys + keyrings
let alice_sk = FampSigningKey::from_bytes([1u8; 32]);
let bob_sk = FampSigningKey::from_bytes([2u8; 32]);
// ...

// Fixture certs from disk
let alice_cert = tls::load_pem_cert(&dir.join("alice.crt")).unwrap();
let alice_key = tls::load_pem_key(&dir.join("alice.key")).unwrap();
// ...

// Bind listeners first (read local_addr before spawn)
let bob_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
bob_listener.set_nonblocking(true).unwrap();
let bob_addr = bob_listener.local_addr().unwrap();
// ...

// HttpTransports (each trusts the peer's fixture cert)
let alice_transport = HttpTransport::new_client_only(Some(&alice_trust)).unwrap();
alice_transport.register(alice.clone()).await;
alice_transport.add_peer(bob.clone(), Url::parse(&format!("https://localhost:{}/", bob_addr.port())).unwrap()).await;

// Spawn the two axum-rustls servers
let bob_router = build_router(bob_keyring.clone(), bob_transport.inboxes());
let alice_router = build_router(alice_keyring.clone(), alice_transport.inboxes());

let bob_handle = tls_server::serve_std_listener(bob_listener, bob_router, Arc::new(bob_server_cfg));
let alice_handle = tls_server::serve_std_listener(alice_listener, alice_router, Arc::new(alice_server_cfg));
bob_transport.attach_server(bob_handle).await;
alice_transport.attach_server(alice_handle).await;
```

Copy verbatim. This entire block is the library-API surface FED-03 mandates.

**Drive cycle + trace assertion** (lines 121–161):

```rust
wait_for_tls_listener_ready().await;

let trace_alice: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));
let trace_bob: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));

let bob_fut = cycle_driver::drive_bob(&bob_transport, &bob_keyring, &bob, &alice, &bob_sk, &trace_bob);
let alice_fut = cycle_driver::drive_alice(&alice_transport, &alice_keyring, &alice, &bob, &alice_sk, &trace_alice);

let (bob_res, alice_res) = tokio::join!(bob_fut, alice_fut);
bob_res.expect("bob driver");
alice_res.expect("alice driver");

// Trace sanity — alice's trace must see Commit, Deliver, Ack lines.
let alice_trace = trace_alice.lock().unwrap();
assert!(alice_trace.iter().any(|l| l.contains("Commit")), ...);
assert!(alice_trace.iter().any(|l| l.contains("Deliver")), ...);
assert!(alice_trace.iter().any(|l| l.contains("Ack")), ...);
```

Copy verbatim. `request → commit → deliver → ack` matches D-12.

**Net delta from analog:** module doc-comment (3 lines) + test-fn rename. Body is byte-for-byte from `http_happy_path.rs`. **This is the cheapest possible refactor — the analog is already the template.**

---

### `crates/famp/tests/e2e_two_daemons_adversarial.rs` (integration-test, sentinel)

**Analog:** `crates/famp/tests/adversarial/http.rs` (162 lines).

**Imports + sentinel struct** (lines 24–52):

```rust
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use famp::runtime::RuntimeError;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::EnvelopeDecodeError;
use famp_keyring::Keyring;
use famp_transport::TransportMessage;
use famp_transport_http::{build_router, InboxRegistry};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, Mutex};

struct HttpRig {
    base_url: String,
    /// The inbox receiver for `bob`. If the handler closure was entered, a
    /// message would be queued here — adversarial rows assert it stays empty.
    inbox_rx: mpsc::Receiver<TransportMessage>,
    sentinel: Arc<AtomicBool>,
    server: tokio::task::JoinHandle<()>,
}
```

**WARNING for Phase 4:** the analog imports `famp::runtime::RuntimeError`. **`runtime/` module is being deleted in Phase 4 commit 6** (Audit 5: `rm -r crates/famp/src/runtime/`). The Phase 4 sentinel test MUST NOT import `famp::runtime`; instead, project HTTP status+slug to a local enum or to `EnvelopeDecodeError` directly. Pattern adjustment:

```rust
// Phase 4 adjustment — runtime::RuntimeError no longer exists post-deletion.
// Project status+slug to a local enum:
#[derive(Debug, PartialEq)]
enum AdversarialOutcome {
    BadEnvelope,
    SignatureInvalid,
    CanonicalDivergence,
}
fn project(status: u16, slug: Option<&str>) -> AdversarialOutcome { ... }
```

The Phase 4 sentinel only needs the **unsigned** case per D-09 ("cheapest sentinel — signature-verification middleware rejects unsigned envelope"); D-13 keeps the full adversarial matrix in `tests/adversarial/`.

**Build-rig pattern** (lines 60–84):

```rust
async fn build_rig() -> HttpRig {
    let inboxes: Arc<InboxRegistry> = Arc::new(Mutex::new(HashMap::new()));
    let (tx, inbox_rx) = mpsc::channel::<TransportMessage>(8);
    inboxes.lock().await.insert(bob(), tx);

    let keyring = build_bob_keyring();
    let router = build_router(keyring, inboxes.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    HttpRig { base_url: format!("http://{addr}"), inbox_rx, sentinel: Arc::new(AtomicBool::new(false)), server }
}
```

Copy verbatim — note this rig uses **plain HTTP** not TLS. Per analog header: "TLS adds nothing to adversarial-byte rejection — the middleware runs identically regardless." Cheaper for the sentinel; matches D-09.

**Sentinel proof — handler-closure-not-entered** (lines 121–143):

```rust
// D-D5 sentinel proof: the inbox handler's sole side-effect is pushing a
// TransportMessage onto this channel. If we can observe a message, the
// handler ran. try_recv must return Empty on every adversarial row.
match rig.inbox_rx.try_recv() {
    Err(mpsc::error::TryRecvError::Empty) => {
        // Expected: middleware short-circuited, handler never ran.
    }
    Ok(msg) => {
        rig.sentinel.store(true, Ordering::SeqCst);
        panic!("TRANS-09 SC#2: handler closure entered on adversarial case ...");
    }
    Err(mpsc::error::TryRecvError::Disconnected) => {
        panic!("inbox channel disconnected unexpectedly ...");
    }
}
assert!(!rig.sentinel.load(Ordering::SeqCst), ...);
```

This is the canonical "handler-closure-not-entered" proof. **Copy verbatim** as the Phase 4 sentinel — only the case-shape (single `Unsigned`) and outcome enum need adapting.

**Test attribute** (line 148):

```rust
#[tokio::test]
async fn http_unsigned() { run_http_case(Case::Unsigned).await; }
```

For Phase 4: rename to `e2e_two_daemons_rejects_unsigned` or similar; keep `#[tokio::test]` attribute.

---

### `crates/famp/tests/_deferred_v1/README.md` (docs, freeze explainer)

**Analog:** `docs/history/README.md` (49 lines — narrative archive marker).

**Heading + intent paragraph pattern** (lines 1–10):

```markdown
# Project History

This directory is the **curated record of how FAMP came to be**, extracted
from a private working archive of planning artifacts. ...
```

For Phase 4 (mapped to D-02 contents):

```markdown
# Federation tests — frozen for v1.0 reactivation

These tests are **dormant in v0.9** because the federation CLI surface they
exercised (`famp init / setup / listen / peer`) was hard-deleted in Phase 4
(commit `feat!(04): remove federation CLI surface ...`). The tests survive
in this directory as **intent documents**: they encode adversarial cases,
conversation shapes, and non-obvious patterns that took adversarial review
to discover.

## Reactivation criteria

[D-02 (b) — Sofer-from-different-machine triggers v1.0 federation gateway,
then port-and-rename against new lib API.]

## What stays exercised in `just ci`

The library-API surface they targeted is preserved by
`crates/famp/tests/e2e_two_daemons.rs` (FED-03/04). The federation crates
(`famp-transport-http`, `famp-keyring`) stay compiling on every commit.

## See also

- [`docs/history/v0.9-prep-sprint/famp-local/`](../../../../docs/history/v0.9-prep-sprint/famp-local/) — archived prep-sprint scaffolding
- [`docs/MIGRATION-v0.8-to-v0.9.md`](../../../../docs/MIGRATION-v0.8-to-v0.9.md) — migration guide
- `v0.8.1-federation-preserved` git tag — escape hatch for federation users
```

**Tone reference (matches `docs/history/README.md` "Stale-link advisory" voice — terse, sectioned, no apology).**

---

### `docs/MIGRATION-v0.8-to-v0.9.md` (docs, migration guide)

**Analog:** RESEARCH.md Audit 9 skeleton (no in-tree migration doc precedent — closest tone analog is `docs/history/README.md`).

**Skeleton authority:** RESEARCH.md Audit 9 lines 530–615 contain a 65-LOC complete draft skeleton. Use Audit 9 as the file's content authority. Constraints from D-18: ≤200 lines, table-first, terse. Required sections (D-18 enumerated):

1. CLI mapping table (top of doc)
2. `.mcp.json` cleanup
3. `~/.famp/` directory cleanup (optional)
4. `v0.8.1-federation-preserved` tag pointer
5. `crates/famp/tests/_deferred_v1/README.md` pointer
6. Workspace internals note (`famp-transport-http` + `famp-keyring` stay)

**Tone marker — top-of-doc TL;DR pattern** (Audit 9 lines 538–545):

```markdown
## TL;DR

- Run `famp install-claude-code` — auto-rewrites your `.mcp.json` ...
- Switch `famp setup` / `famp init` → `famp register <name>`.
- `famp listen` is gone — the broker auto-spawns.
- `famp peer add` / `famp peer import` are gone — same-host discovery is automatic.
- `famp send` keeps the same flag surface; only the transport changed.
```

Copy verbatim.

---

### `docs/history/v0.9-prep-sprint/famp-local/README.md` (one-line freeze marker)

**Analog:** D-14 prescribes a single-line marker; no in-tree precedent for a one-line README.

**Pattern (per Audit 8 line 506–507):**

```markdown
# famp-local — frozen v0.9 prep-sprint scaffolding

This script is **frozen**, not maintained. It embodies the v0.9 prep-sprint
UX validation (T1–T9). Bug fixes ship via the live `famp` binary or the
`famp-local hook` subcommand (Phase 2 / HOOK-04a).

See [`docs/MIGRATION-v0.8-to-v0.9.md`](../../../MIGRATION-v0.8-to-v0.9.md).
```

3–5 lines maximum. Per D-14 "single-line marker per D-14" + "Bug fixes only via the live `famp` binary or `famp-local hook` subcommand."

---

### `crates/famp/tests/cli_help_invariant.rs` (optional — CLI help smoke)

**Analog:** `crates/famp/tests/cli_dm_roundtrip.rs` (the only `assert_cmd::cargo::CommandCargoExt` precedent).

**Header + cargo-bin pattern** (lines 1–18):

```rust
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 02 plan 02-12 — `famp` CLI integration round-trip tests.
//! Exercises ... All five tests shell `famp` as a real subprocess via
//! `assert_cmd::Command::cargo_bin`...

use std::process::{Command, Stdio};
use assert_cmd::cargo::CommandCargoExt;
```

For Phase 4 invariant test:

```rust
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 04 FED-01 invariant: the 6 deleted federation verbs (init, setup,
//! listen, peer add, peer import, old TLS-form send) MUST NOT appear in
//! `famp --help` output. Migration doc carries the load (D-05 hard delete).

use std::process::Command;
use assert_cmd::cargo::CommandCargoExt;

#[test]
fn famp_help_omits_deleted_federation_verbs() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    for verb in ["init", "setup", "listen", "peer"] {
        assert!(
            !stdout.lines().any(|l| l.trim_start().starts_with(verb)),
            "famp --help must not advertise deleted verb `{verb}`; got:\n{stdout}"
        );
    }
}
```

**Cargo-bin invocation pattern** (lines 47–55 of `cli_dm_roundtrip.rs`):

```rust
Command::cargo_bin("famp")
    .unwrap()
    .env("FAMP_BUS_SOCKET", self.sock())
    .env("HOME", self.tmp.path())
    .args(args)
    .output()
    .unwrap()
```

Phase 4 invariant test does NOT need bus-socket isolation (it's a `--help` smoke), so drop the `.env(...)` lines.

---

### `crates/famp/src/cli/mod.rs` (controller dispatch — significant edits)

**Analog (negate-add pattern):** the file's own structure. RESEARCH Audit 5 lines 332–355 has the exact per-line cut list. Per-line edits (verified line numbers):

- Line 14 — `pub mod init;` → DELETE
- Line 18 — `pub mod listen;` → DELETE
- Line 22 — `pub mod peer;` → DELETE
- Line 26 — `pub mod setup;` → DELETE
- Line 33 — `pub use init::InitOutcome;` → DELETE
- Line 34 — `pub use listen::ListenArgs;` → DELETE
- Lines 45–46 (`Init(InitArgs)` variant + doc) → DELETE
- Lines 47–48 (`Setup(setup::SetupArgs)` variant + doc) → DELETE
- Lines 71–73 (`Listen(ListenArgs)` variant + doc) → DELETE
- Lines 74–75 (`Peer(peer::PeerArgs)` variant + doc) → DELETE
- Lines 118–123 (`InitArgs` struct) → DELETE
- Line 147 (`Commands::Init(args) => init::run(args).map(|_| ()),`) → DELETE
- Line 148 (`Commands::Setup(args) => setup::run(&args).map(|_| ()),`) → DELETE
- Line 154 (`Commands::Peer(args) => peer::run(args),`) → DELETE
- Line 158 (`Commands::Listen(args) => block_on_async(listen::run(args)),`) → DELETE

**Pattern surrounding the keepers (preserve exactly):**

```rust
// Sync arms (no tokio runtime needed).
Commands::InstallClaudeCode(args) => install::claude_code::run(args),
Commands::UninstallClaudeCode(args) => uninstall::claude_code::run(args),
Commands::InstallCodex(args) => install::codex::run(args),
Commands::UninstallCodex(args) => uninstall::codex::run(args),
Commands::Info(args) => info::run(&args).map(|_| ()),
// Async arms: each boots a multi-thread tokio runtime via
// `block_on_async` and dispatches into the subcommand's
// `async fn run`. Only async-required arms pay the runtime cost.
Commands::Send(args) => block_on_async(send::run(args)),
Commands::Await(args) => block_on_async(await_cmd::run(args)),
// ...
```

The "Sync arms" / "Async arms" comment block is structural — preserve it. After deletion the sync block has 5 entries (Install ×4 + Info), the async block has 10 entries.

**Subtle precedent: ordering of variants matches the order of variants in `Commands` enum.** Maintain this discipline post-deletion.

---

### `crates/famp/src/cli/info.rs` (controller — Risk #1 surgical refactor)

**Analog:** `crates/famp/src/cli/whoami.rs` (101 lines — closest "small self-contained subcommand with own outcome struct" pattern).

**Current `info.rs` cross-module deps to remove (lines 11–14):**

```rust
use crate::cli::config::Config;
use crate::cli::error::CliError;
use crate::cli::setup::PeerCard;       // → INLINE the struct
use crate::cli::{home, init};          // → drop `init`; replace with private fn
```

**Source for `PeerCard` to inline** — `crates/famp/src/cli/setup.rs` lines 32–43:

```rust
/// Peer card: shareable identity for peer registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCard {
    /// Suggested alias (the agent's name).
    pub alias: String,
    /// HTTPS endpoint for the agent's inbox.
    pub endpoint: String,
    /// base64url-unpadded ed25519 public key.
    pub pubkey: String,
    /// FAMP principal (e.g., `agent:localhost/alice`).
    pub principal: String,
}
```

Inline this struct verbatim into `info.rs`. **Note:** `PeerCard` derives `Serialize`/`Deserialize` from `serde::{Deserialize, Serialize}` — `info.rs` will need to add `use serde::{Deserialize, Serialize};` when the inline lands.

**Source for `load_identity` to inline** — `crates/famp/src/cli/init/mod.rs` lines 211–232:

```rust
/// Phase 1 slice of IDENT-05: verify all six identity files exist.
pub fn load_identity(home: &Path) -> Result<IdentityLayout, CliError> {
    if !home.is_absolute() {
        return Err(CliError::HomeNotAbsolute { path: home.to_path_buf() });
    }
    let layout = IdentityLayout::at(home.to_path_buf());
    for (_label, path) in layout.entries() {
        if !path.exists() {
            return Err(CliError::IdentityIncomplete { missing: path.to_path_buf() });
        }
    }
    Ok(layout)
}
```

Inline as a **private** `fn load_identity` inside `info.rs` (drop `pub`). Add `use crate::cli::paths::IdentityLayout;` since `load_identity` returns it.

**Net delta from analog (whoami.rs):** info.rs gains a private struct + private fn (~15 LOC); becomes self-contained; the public `run` / `run_at` / `InfoArgs` surface is unchanged so `info_happy_path.rs` test stays in the active suite (per Risk #2).

---

### `crates/famp/src/cli/send/mod.rs` (controller — surgical 2-line removal)

**Analog (negate-add):** the file's own lines 57–58.

**Per Audit 3 (lines 226–234):**

```rust
// REMOVE line 57:
pub mod client;
// REMOVE line 58:
pub mod fsm_glue;
```

Plus the doc-comment block at lines 40–46 (the "v0.8 federation (HTTPS) path" explainer paragraph). Replace with:

```rust
// v0.8 federation HTTPS path was deleted in Phase 4; see
// `docs/MIGRATION-v0.8-to-v0.9.md`.
```

**No other edits inside `send/mod.rs`.** Bus-routed `famp send` (Phase 2 plan 02-04) has zero deps on the deleted code paths (Audit 3 confirmed).

---

### `crates/famp/src/cli/error.rs` (controller — drop `Tls` variant)

**Analog (negate-add):** the file's own lines 70–71.

**Per Audit 3 + Risk #8:**

```rust
// REMOVE lines 70–71:
#[error("TLS config error")]
Tls(#[from] famp_transport_http::TlsError),
```

**Risk #8 ripple:** any `match CliError` with `CliError::Tls { .. } =>` arms must drop the arm. Per Phase 2 D-06/D-11 exhaustive-match discipline: **wildcard-free**. Search before commit:

```bash
grep -rn "CliError::Tls" crates/famp/src/
```

Likely consumer (per Risk #8): `mcp_error_kind` exhaustive match. Drop arm atomically with variant deletion.

**Pattern marker — `thiserror::Error` derive** (line 11):

```rust
#[derive(Debug, thiserror::Error)]
pub enum CliError { ... }
```

Preserve discipline: every variant has `#[error(...)]` attribute; payloads use `#[source]` or `#[from]`; no raw key material (D-04/D-05 enforced by `init_no_leak.rs`).

---

### Workspace `Cargo.toml` (config — comment relabel)

**Analog (style):** existing `[workspace] members = [...]` block lines 3–16 (no per-member comments currently — the relabel is novel but uniform).

**Per Audit 11 commit 3 + RESEARCH Code Examples §3:**

```toml
# Before (lines 10, 12 of workspace Cargo.toml):
"crates/famp-keyring",
"crates/famp-transport-http",

# After (suggested ~5-word comment per CONTEXT.md Discretion):
# v1.0 federation internals
"crates/famp-keyring",
# v1.0 federation internals
"crates/famp-transport-http",
```

**Comment style:** terse, present-tense, ≤5 words. CLAUDE.md's tech-stack research §13 ("Workspace `Cargo.toml` comment style — terse, present-tense") is the precedent. Two identical `# v1.0 federation internals` comments — uniform.

---

### Documentation framing edits (README.md / CLAUDE.md / ROADMAP.md / MILESTONES.md / ARCHITECTURE.md)

**Analog:** Phase 3 D-13 ARCHITECTURE.md staged-framing edit (already landed — confirms tone). RESEARCH Audit 10 lines 622–688 has per-file landing sites + verbatim quote-blocks of the proposed replacement text.

**Pattern: surgical-changes rule (CLAUDE.md global):**

> every changed line must trace directly to the user's request; don't "improve" adjacent code, comments, or formatting as drive-by refactoring.

Per CONTEXT.md `<specifics>`:

> No drive-by polish. Per CLAUDE.md "surgical changes" rule: every README/CLAUDE.md/ROADMAP edit traces directly to D-16/D-17 staged framing or to the FED/MIGRATE requirement table. No reformatting of unrelated sections.

**Verbatim landing-site replacement texts** (use Audit 10 as authority):

- README.md first-paragraph block (Audit 10 lines 636–640)
- CLAUDE.md "## Project" block (Audit 10 lines 654–659)
- ROADMAP.md v0.9 milestone callout (Audit 10 lines 666–668)
- MILESTONES.md v0.9 section header (Audit 10 lines 676–683)
- ARCHITECTURE.md line 4 + line 38 header flips (Audit 10 line 686)

---

### `.planning/REQUIREMENTS.md` checkbox flips

**Analog:** every closing commit (e.g., `docs(03-06): record stop hook UAT pass`).

**Pattern (per Audit 4 + commit-1 atomic-claim from Audit 11):**

```markdown
- [ ] CARRY-01 — pin listen-subprocess test-group at max-threads=4
+ [x] CARRY-01 — pin listen-subprocess test-group at max-threads=4 (closed in `ebd0854`)
```

Inline SHA reference for CARRY-01 per D-22. Other phase-4 requirements (FED-01..06, MIGRATE-01..04, TEST-06) flip in the closing commit of the wave that lands them; no SHA reference needed (the work is in this phase).

---

## Shared Patterns

### Pattern: Atomic-commit-with-tag-escape-hatch (Phase 1 AUDIT-05 precedent)

**Source:** `crates/famp-envelope/src/version.rs` (`FAMP_SPEC_VERSION = "0.5.2"`) + Phase 1 STATE.md anchor (commit `9ca6e13`).

**Apply to:** all Phase 4 commits. Per Audit 11, the 9-commit sequence respects:

> A reader of `git log v0.8.1-federation-preserved..main` on a fresh checkout MUST see ONLY the deletion + relabeling work, never the refactor.

**Tag-cut operation (per Audit 7 + RESEARCH Code Examples §4):**

```bash
git tag v0.8.1-federation-preserved $(git rev-parse HEAD)
# Optional (D-21 not strictly required):
git push origin v0.8.1-federation-preserved
```

Lightweight tag (NO `-a`/`-m`). Cut on commit-3 SHA per Audit 11.

---

### Pattern: `git mv` over `cp + rm` for archival moves

**Source:** CONTEXT.md "Reusable Assets" + CLAUDE.md commit-discipline.

**Apply to:**
- D-04 test-file freeze (~27 file moves to `_deferred_v1/`)
- D-14 `scripts/famp-local` archive (single-file move to `docs/history/v0.9-prep-sprint/famp-local/famp-local`)

**Verification post-move:**

```bash
git log --follow <new-path>   # confirms history preserved
```

---

### Pattern: Atomic-commit titles (verified against `git log`)

**Source:** `git log --oneline | grep -E "^[a-f0-9]+ (feat|fix|chore|docs|test|refactor)\(\d+"` — confirms the codebase uses Conventional Commits with phase numbering (`docs(03-06)`, `chore(03-01)`, `test(03-03)`, `feat!(...)`).

**Apply to:** all 9 Phase 4 commits. Audit 11 proposed titles match precedent:

| Audit 11 title | Style verified against |
|----------------|------------------------|
| `chore(04): pin reqs/roadmap CARRY-01 to closing SHA ebd0854` | `chore(03-01): add workspace publish recipes` |
| `refactor(04): e2e_two_daemons targets transport-http library API directly` | `test(03-03): cover hook runner dispatch failures` (refactor implied) |
| `chore(04): relabel famp-transport-http and famp-keyring as v1.0 federation internals` | `chore(03-01): replace stub crate descriptions` |
| `test(04): freeze federation tests under _deferred_v1/` | `test(03-05): cover codex install roundtrip` |
| `feat!(04): remove federation CLI surface (init, setup, listen, peer, TLS-form send)` | `feat!(...)` precedent NOT yet in repo (this is the first breaking change at this scale); the `!` flag follows Conventional Commits SemVer-major signal |
| `chore(04): archive scripts/famp-local under docs/history/v0.9-prep-sprint/` | `chore(03-01): add workspace publish recipes` |
| `docs(04): MIGRATION-v0.8-to-v0.9.md` | `docs(03): record 6 plans for v0.9 Phase 3` |
| `docs(04): staged-framing edits across README, CLAUDE, ROADMAP, MILESTONES, ARCHITECTURE` | `docs(03-02): land Claude Code integration amendments` |

**Recommendation for planner:** the `feat!(04): ...` title is novel for this codebase but matches Conventional Commits and matches the SemVer-major signal Phase 4 actually carries (CLI surface deletion). Lock at plan-time.

---

### Pattern: Library-API integration test as federation-CI insurance

**Source:** Audit 6 — `tests/http_happy_path.rs` (template) + `tests/adversarial/http.rs` (sentinel).

**Apply to:** the refactored `e2e_two_daemons.rs` (happy path) + `e2e_two_daemons_adversarial.rs` (sentinel). One happy + one adversarial = plumb-line-2 insurance against `famp-transport-http` mummification. **Resist further expansion** per D-09.

---

### Pattern: `thiserror::Error` derive + `#[source]` discipline

**Source:** `crates/famp/src/cli/error.rs` (every variant carries at most a `PathBuf` label and a `#[source]`-wrapped inner error; no variant embeds raw seed bytes / `FampSigningKey` / rcgen secret).

**Apply to:** Phase 4 surgical edit on `error.rs` (drop `Tls` variant). Maintain discipline: no exception-swallowing wildcard arms (Phase 2 D-06/D-11 exhaustive-match locked).

---

### Pattern: `#![allow(unused_crate_dependencies)]` per integration-test file

**Source:** every test file in `crates/famp/tests/*.rs` opens with this attribute (workspace lint `unused_crate_dependencies = "warn"` triggers per-test if a workspace dep isn't pulled).

**Apply to:** new `e2e_two_daemons.rs` + `e2e_two_daemons_adversarial.rs` + (optional) `cli_help_invariant.rs`. Always include the `#![allow(...)]` block.

---

### Pattern: lint-suppression block for tests using `unwrap`/`expect`

**Source:** workspace lint `unwrap_used = "deny"` + `expect_used = "deny"` (lines 67–68 of root `Cargo.toml`). Test files must opt out:

```rust
#![allow(clippy::unwrap_used, clippy::expect_used)]
```

**Apply to:** every new test file. `http_happy_path.rs` adds `clippy::similar_names`, `clippy::significant_drop_tightening`, `clippy::doc_markdown` to the same block; copy that exact list for `e2e_two_daemons.rs`.

---

## No Analog Found

| File | Reason |
|------|--------|
| (none) | All 6 CREATE files have a strong or role-match analog in the existing tree. |

---

## Metadata

**Analog search scope:**
- `crates/famp/tests/*.rs` (integration tests — found `http_happy_path.rs`, `adversarial/http.rs`, `cli_dm_roundtrip.rs`)
- `crates/famp/src/cli/*` (subcommand modules — found `whoami.rs` analog for `info.rs` refactor; verified `mod.rs` per-line cut list)
- `crates/famp/src/cli/error.rs` (`thiserror::Error` discipline)
- `Cargo.toml` workspace root (member-comment style)
- `docs/history/README.md` (archive-narrative tone for `_deferred_v1/README.md`)
- `git log --oneline` (Conventional Commits precedent — confirms `(NN-NN):` phase-numbering style)
- `.config/nextest.toml` (CARRY-01 pin verification)

**Files scanned:** 14 source files + 6 test files + workspace root + docs/history/README.md

**Pattern extraction date:** 2026-05-03

**Notable points for planner:**
1. `e2e_two_daemons.rs` refactor is **the cheapest possible**: copy `http_happy_path.rs` body verbatim, change module doc-comment + test-fn name. Library-API surface is already proven by the analog.
2. `e2e_two_daemons_adversarial.rs` must NOT import `famp::runtime::RuntimeError` (the module dies in commit 6); replace with a local enum or `EnvelopeDecodeError` direct.
3. `info.rs` Risk #1 surgical refactor is small (~15 LOC inline of `PeerCard` + private `load_identity`); after the refactor the file is self-contained and `info_happy_path.rs` stays in the active suite.
4. Workspace `Cargo.toml` per-member comments are novel but uniform — `# v1.0 federation internals` is the locked phrasing.
5. The `feat!(04): ...` commit title introduces the first SemVer-major breaking-change marker in this repo's history; Conventional Commits precedent supports it.
