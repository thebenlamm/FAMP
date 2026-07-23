---
phase: 08-signed-cross-host-envelope-trust-bootstrap
plan: 04
subsystem: auth
tags: [ed25519, cli, tofu, keyring, clap, trust-bootstrap]

requires:
  - phase: 08-signed-cross-host-envelope-trust-bootstrap (plan 02)
    provides: "FampSigningKey::generate() + key_id() fingerprint primitives"
provides:
  - "famp peer export --as <name>: prints a single Signal-paste-safe line (principal + b64url pubkey + key_id fingerprint) from the gateway's own persisted Ed25519 keypair"
  - "famp peer import [<file>|-]: parses the export blob and TOFU-pins the peer key into ~/.famp/gateway/peers.keyring"
  - "load_or_generate(path) -> FampSigningKey: generate-once-and-persist gateway signing keypair, idempotent across restarts"
  - "gateway_identity_path()/gateway_peers_keyring_path(): stable ~/.famp/gateway/ path helpers reusable by Phase 9's live signing/verify wiring"
affects: [phase-09-live-transport, gateway-ingress-verify]

tech-stack:
  added: []
  patterns:
    - "run()/run_at() production-vs-test entrypoint split (cli/info.rs convention) applied to famp peer export/import"
    - "CLI-layer 3-field blob parser distinct from famp-keyring::file_format's strict 2-field on-disk format"
    - "TOCTOU-safe 0600 secret-file write (cli/perms::write_secret) reused for the gateway signing key"

key-files:
  created:
    - crates/famp/src/cli/peer/mod.rs
    - crates/famp/src/cli/peer/identity.rs
    - crates/famp/src/cli/peer/export.rs
    - crates/famp/src/cli/peer/import.rs
    - crates/famp/tests/peer_roundtrip.rs
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/tests/cli_help_invariant.rs

key-decisions:
  - "famp-keyring promoted from [dev-dependencies] to [dependencies] — famp peer export/import use Keyring at runtime, not just in tests."
  - "Export blob is a NEW CLI-layer 3-field parser (parse_export_line), NOT famp-keyring::file_format::parse_line (strict 2-field — would reject the fingerprint token). The on-disk keyring file stays the clean 2-field format."
  - "Gateway signing key persists at ~/.famp/gateway/identity.ed25519 (NOT the stale IdentityLayout::key_ed25519 name) — a fresh, gateway-owned identity file, mode 0600 via the existing perms::write_secret TOCTOU-safe helper."
  - "Two fresh CliError variants (PeerBlobMalformed, PeerKeyConflict) rather than repurposing the orphaned TlsFingerprintMismatch/TofuBootstrapRefused fossils, which target an unrelated TLS-cert trust model."
  - "Scope constraint: one signing key per remote principal name (RESEARCH Pitfall 4) — recorded in module docs, not enforced in code this phase."

patterns-established:
  - "Pattern: CLI subcommand trees needing runtime crypto/keyring access promote the dep from dev- to real- dependencies and add an unconditional `use <crate> as _;` silencer at both the lib.rs and bin.rs level for the transitional/production-unused-but-declared state."

requirements-completed: [TRUST-01]

coverage:
  - id: D1
    description: "famp peer export --as <name> prints a single copy/paste-safe line (principal + b64url pubkey + key_id fingerprint) from the gateway's own persisted Ed25519 keypair; no key material crosses FAMP."
    requirement: "TRUST-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/peer/export.rs#tests::format_export_line_has_three_whitespace_fields_and_trailing_newline"
        status: pass
      - kind: integration
        ref: "crates/famp/tests/peer_roundtrip.rs#export_import_pins_the_exact_key"
        status: pass
    human_judgment: false
  - id: D2
    description: "famp peer import [<file>|-] parses the export blob and TOFU-pins the peer key into ~/.famp/gateway/peers.keyring, the same file verify_inbound reads."
    requirement: "TRUST-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/peer/import.rs#tests::parse_export_line_round_trips_principal_and_pubkey"
        status: pass
      - kind: integration
        ref: "crates/famp/tests/peer_roundtrip.rs#export_import_pins_the_exact_key"
        status: pass
    human_judgment: false
  - id: D3
    description: "A conflicting re-pin (different key, same principal) fails closed with PeerKeyConflict; a never-imported principal is absent from the keyring."
    requirement: "TRUST-01"
    verification:
      - kind: integration
        ref: "crates/famp/tests/peer_roundtrip.rs#conflicting_repin_fails_closed"
        status: pass
      - kind: integration
        ref: "crates/famp/tests/peer_roundtrip.rs#never_imported_principal_is_absent"
        status: pass
    human_judgment: false
  - id: D4
    description: "load_or_generate persists the gateway's own Ed25519 signing keypair once and reloads (not regenerates) it on subsequent calls."
    requirement: "TRUST-01"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/peer/identity.rs#tests::load_or_generate_is_idempotent"
        status: pass
      - kind: unit
        ref: "crates/famp/src/cli/peer/identity.rs#tests::load_or_generate_persists_to_disk"
        status: pass
    human_judgment: false

duration: 20min
completed: 2026-07-23
status: complete
---

# Phase 8 Plan 4: Two-Machine Trust Bootstrap CLI (`famp peer export`/`import`) Summary

**`famp peer export --as <name>` / `famp peer import [<file>|-]` establish mutual Ed25519 TOFU trust via a hand-copied, Signal-paste-safe 3-field blob, backed by a new generate-once-and-persist gateway keypair — closing the previously-nonexistent keygen gap and wiring `famp-keyring`'s existing pin/conflict machinery end-to-end.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-23T17:10:00-04:00 (approx, prior HEAD)
- **Completed:** 2026-07-23T17:30:00-04:00
- **Tasks:** 3 completed
- **Files modified:** 12 (5 created, 7 modified)

## Accomplishments
- `crates/famp/src/cli/peer/identity.rs`: `load_or_generate(path) -> FampSigningKey` — the gateway's own signing keypair, generated once at `~/.famp/gateway/identity.ed25519` (mode 0600) and reloaded (never regenerated) on every subsequent call. Closes RESEARCH Pitfall 3 — no keygen/persistence path existed anywhere in the codebase before this plan.
- `famp peer export --as <name>` prints the single, Signal-paste-safe line `<principal> <pubkey-b64url> <key_id>\n`.
- `famp peer import [<file>|-]` parses that blob (3-field-tolerant, fingerprint optional/advisory) and TOFU-pins the peer key into `~/.famp/gateway/peers.keyring` via `Keyring::pin_tofu` + `save_to_file` — the same file Plan 03's `verify_inbound` reads.
- `crates/famp/tests/peer_roundtrip.rs`: fully in-process TRUST-01 proof — export → import → `Keyring::get` returns the exact pinned key; a conflicting re-pin under the same principal fails closed (`PeerKeyConflict`) without corrupting the existing pin; a never-imported principal is absent (the TRUST-02 precondition).
- `famp-keyring` promoted from dev- to a real dependency of `famp`; `Commands::Peer` wired into the CLI dispatch tree and the MCP `mcp_error_kind()` exhaustive match.

## Task Commits

Each task was committed atomically:

1. **Task 1: Gateway keypair persistence + peer subcommand tree + Commands wiring** - `74c96f7` (feat)
2. **Task 2: famp peer export + famp peer import subcommands** - `4aecc12` (feat)
3. **Task 3: Single-machine export→import→pin round-trip integration test (TRUST-01)** - `21bbf82` (test)

**Plan metadata:** committed via this SUMMARY + STATE/ROADMAP update (below)

## Files Created/Modified
- `crates/famp/src/cli/peer/mod.rs` - `PeerArgs`/`PeerSubcommand` tree (mirrors `cli/daemon/mod.rs`), scope-note module docs
- `crates/famp/src/cli/peer/identity.rs` - `load_or_generate`, `gateway_identity_path`, `gateway_peers_keyring_path`
- `crates/famp/src/cli/peer/export.rs` - `PeerExportArgs`, `run`/`run_at`, `format_export_line`
- `crates/famp/src/cli/peer/import.rs` - `PeerImportArgs`, `run`/`run_at`, `parse_export_line`
- `crates/famp/tests/peer_roundtrip.rs` - TRUST-01 round-trip + fail-closed conflict + unpinned-absence tests
- `crates/famp/Cargo.toml` - `famp-keyring` moved to `[dependencies]`
- `crates/famp/src/cli/mod.rs` - `pub mod peer;`, `Commands::Peer` enum variant + dispatch arm
- `crates/famp/src/cli/error.rs` - `PeerBlobMalformed`, `PeerKeyConflict` variants
- `crates/famp/src/cli/mcp/error_kind.rs` - exhaustive-match arms for the two new variants
- `crates/famp/src/lib.rs`, `crates/famp/src/bin/famp.rs` - `unused_crate_dependencies` silencers updated for famp-keyring's real-dependency status
- `crates/famp/tests/cli_help_invariant.rs` - updated FED-01 invariant (see Deviations)

## Decisions Made
- **famp-keyring: dev-dep -> real dep.** `famp peer export`/`import` need `Keyring` at runtime; the crate-level `unused_crate_dependencies` silencer in `lib.rs`/`bin.rs` was correspondingly de-scoped from `#[cfg(test)]` to unconditional.
- **CLI-layer 3-field blob parser, not an extension of `famp-keyring::file_format`.** Keeps the on-disk keyring file format's parse surface unchanged (2-field, strict); the fingerprint is CLI-only, advisory metadata.
- **Gateway identity file at `~/.famp/gateway/identity.ed25519`**, deliberately distinct from the dead-in-practice `IdentityLayout::key_ed25519` — a fresh, gateway-owned concept per RESEARCH Pitfall 3.
- **Fingerprint mismatch on import is a warning, not a hard failure** — supports partial/hand-typed pastes while still surfacing paste-corruption to the operator (T-08-10 mitigation).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Stale test invariant] Updated `cli_help_invariant.rs`'s FED-01 assertion**
- **Found during:** Task 3, running the full test suite for regression checking
- **Issue:** A pre-existing test (`famp_help_omits_deleted_federation_verbs`, from the v0.9 Phase 4 CLI purge) asserted `famp --help` must NEVER advertise a `peer` verb. Phase 8's locked CONTEXT.md decision D-05 deliberately reintroduces `famp peer` — with a different shape (`export`/`import` against `famp-keyring`, not the deleted TOML `peers.toml` `add`/`import`). This is an intentional, planned change, not a regression, but the old test would fail loudly on it.
- **Fix:** Updated the test to keep asserting `init`/`setup`/`listen` stay gone, and added positive assertions that `famp peer --help` advertises `export`/`import` while the OLD deleted `add` subcommand stays absent.
- **Files modified:** `crates/famp/tests/cli_help_invariant.rs`
- **Verification:** `cargo test -p famp --test cli_help_invariant` green after the fix; `just lint` clean.
- **Committed in:** `21bbf82` (Task 3 commit)

**2. [Rule 3 - Blocking] Task 1/2 split required stub `export.rs`/`import.rs` files**
- **Found during:** Task 1
- **Issue:** The plan's Task 1 declares `PeerSubcommand::Export(export::PeerExportArgs)`/`Import(import::PeerImportArgs)` in `peer/mod.rs`, which requires `export.rs`/`import.rs` to exist for `cargo build -p famp` (Task 1's own verify step) to pass — but those files are Task 2's stated file list.
- **Fix:** Created minimal compiling stub `export.rs`/`import.rs` (Args structs + a `NotImplemented` stub `run()`) in Task 1's commit so the subcommand tree compiles and wires end-to-end; Task 2 then replaced the stubs with full implementations.
- **Files modified:** `crates/famp/src/cli/peer/export.rs`, `crates/famp/src/cli/peer/import.rs` (created in Task 1, completed in Task 2)
- **Verification:** Both tasks' own verify steps (`cargo build -p famp`, `just lint`) passed independently.
- **Committed in:** `74c96f7` (Task 1 stub), `4aecc12` (Task 2 full implementation)

---

**Total deviations:** 2 auto-fixed (1 stale-test-invariant, 1 blocking/build-ordering)
**Impact on plan:** Both necessary consequences of the plan's own design (D-05's deliberate `peer` verb reintroduction, and the Task 1/2 file-ownership split). No scope creep — no architectural changes, no new files beyond what the plan specified.

## Issues Encountered
None beyond the deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `load_or_generate` and the `~/.famp/gateway/` path helpers are ready for Phase 9's live HTTP transport to call when it actually signs outbound envelopes.
- `~/.famp/gateway/peers.keyring` is the single source of pinned trust that Plan 03's `verify_inbound` already reads — Phase 8's TRUST-01/TRUST-02 loop is now fully closed at the CLI + gateway-verify layers (live wire delivery is Phase 9's GW-01/02/03).
- Scope note carried forward: one signing key per remote principal name (RESEARCH Pitfall 4, Open Question 1) — generalizing to multiple simultaneously-trusted principal names per remote machine is explicitly deferred to v1.1 if ever needed.
- No blockers identified for downstream plans in this phase or Phase 9.

---
*Phase: 08-signed-cross-host-envelope-trust-bootstrap*
*Completed: 2026-07-23*

## Self-Check: PASSED

All 3 task commit hashes (74c96f7, 4aecc12, 21bbf82) found in `git log`.
All 12 created/modified files confirmed present on disk.
