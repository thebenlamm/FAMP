# Phase 15: Keyring Multi-Key Extension + Revocation - Research

**Researched:** 2026-07-31
**Domain:** Rust key-management / trust-store data model, keyed-file backward compatibility, signature-based revocation
**Confidence:** HIGH (all findings grounded in direct source reads of `crates/famp-keyring/`, `crates/famp-gateway/`, `crates/famp-crypto/`, `crates/famp-envelope/`, and this machine's live `~/.famp/gateway/peers.keyring`; no new external libraries are involved, so no web/package research was required)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** The keyring stores multiple keys per principal with explicit active/retired state.
- **D-02:** Existing single-key keyring files MUST load unchanged. Prove this with a fixture test using a real pre-v1.1 file committed as a fixture — NOT by code review, and NOT by round-tripping the new writer's own output.
- **D-03:** A peer's key can be rotated: pin a new key for a known peer without dropping the previous key until it is explicitly retired.
- **D-04:** "Key CHANGED for a known peer" must be a structurally distinct path from "new peer, first pin" — different exit code AND different operator confirmation. Not a warning line in a stream the operator has learned to scroll past.
- **D-05:** This is the phase's most safety-critical single requirement — silent re-pin on key change is exactly v1.0's TOFU failure mode.
- **D-06:** REVK-01 (expiry-based validity window) is the primary revocation mechanism, not REVK-02. A key past its validity window is rejected at verify time regardless of whether any revocation record was ever received.
- **D-07:** REVK-02 (signed revocation statement) is verifiable and fail-closed, but is explicitly defense-in-depth on top of REVK-01, not the primary path.
- **D-08:** REVK-03: an envelope signed before a revocation takes effect must be rejected after it takes effect — no pre-revocation replay window. Prove with a test: sign pre-revocation, revoke, then attempt verify — must reject.
- **D-09:** The on-disk record shape must be designed once, with revocation/expiry fields present from the start, even if some revocation logic lands in a later plan within this same phase. Do NOT ship the multi-key format now and bolt revocation fields on later.
- **D-10:** Phase 16 (PAKE pairing) and Phase 19 (famp-directory) both consume whatever pin/rotate/retire surface this phase exposes. Design deliberately; state it explicitly in the phase summary.
- **D-11:** Layer 0 stays FROZEN and byte-identical. `famp-keyring` is explicitly NOT Layer 0. Changes land in `famp-keyring` and `famp-gateway`'s `verify.rs`.
- **D-12:** Do NOT reopen `BUS_PROTO_VERSION` — if it looks like it needs another bump, STOP and tell famp-lead-730 before implementing.
- **D-13:** `just lint` (nursery lints) must be clean. `just check-no-tokio-in-bus` must stay green.
- **D-14:** Do NOT use `cargo nextest` / `just ci` / `just test` — nextest hangs on this machine. Use `cargo test --workspace --no-fail-fast`, backgrounded (but run in foreground under a `timeout` per learned-rules.md — see Pitfalls). Report exact result-block counts (pass/fail/ignored).
- **D-15:** Known-accepted flake classes if they resurface: `test_bind_exclusive_unlinks_stale_socket` (deferred v0.11 spawn-lock race) and the 5 codex install/uninstall relink races (verify in isolation via `cargo test -p famp --lib codex`).
- **D-16:** Report back with: the pin/rotate/retire API surface, the on-disk record shape, how KEYR-03's two paths differ observably (exit codes, confirmation prompts), and anything that seems wrong with the plan. Push back BEFORE implementing if something doesn't hold up.

### Claude's Discretion

- Exact on-disk record shape / serialization format for the multi-key + revocation fields (must satisfy D-02's backward-compat fixture test and D-09's "design once" constraint).
- Exact CLI/API ergonomics for pin/rotate/retire, as long as KEYR-03's two paths are observably distinct (D-04) and the surface is documented for Phase 16/19 consumption (D-10).

### Deferred Ideas (OUT OF SCOPE)

- Phase 16 (Cross-Person Trust Bootstrap / PAKE Pairing) — consumes this phase's pin/rotate/retire surface, out of scope here.
- Phase 19 (Signed Peer Directory) — same, out of scope here.
- QUAR-07 (Phase 14's independent diff-only review) — still running externally; do not wait on it.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| KEYR-01 | Multi-key-per-principal storage, explicit active/retired state; existing single-key files load unchanged (fixture-proven) | See "Current On-Disk Format" and "Backward-Compat Fixture Precedent Already Exists" below — the exact fixtures needed are already committed at `crates/famp-keyring/tests/fixtures/` |
| KEYR-02 | Rotate a peer's key: pin new key without dropping the old one until explicitly retired | See "Current `pin_tofu` Behavior" and "Proposed API Surface" — `rotate_to` does not exist yet, only named in ROADMAP.md prose |
| KEYR-03 | "Key changed" is a structurally distinct path (exit code + confirmation) from "first pin" | See "Exit Code Precedent" and "Architecture Pattern: Confirmed Rotation Flow" |
| REVK-01 | Validity window; a key past it is rejected at verify time regardless of any revocation record | See "Pitfall: Never Trust the Envelope's Own Timestamp for Revocation Decisions" — this is the load-bearing design finding of this research |
| REVK-02 | Signed revocation statement, verifiable, fail-closed, defense-in-depth only | See "Reuse `sign_value`/`verify_value` for the Revocation Statement" and "Open Question: Who May Sign a Revocation?" |
| REVK-03 | Envelope signed before revocation takes effect is rejected after it takes effect | Same pitfall as REVK-01 — solved by checking key state at verify time, not by comparing timestamps |
</phase_requirements>

## Summary

`famp-keyring` today is a genuinely minimal crate (278 lines across 4 files): a `HashMap<Principal, TrustedVerifyingKey>` backed by a hand-rolled, line-oriented text file format (`principal  pubkey_b64url\n`, two-space separator, full-line `#` comments only). There is exactly one mutation path, `pin_tofu`, which is a TOFU-only, fail-closed-on-conflict primitive — first sight pins, a second *different* key for the same principal is a hard `KeyConflict` error, and there is **no** `rotate_to`, `retire`, or `revoke` function anywhere in the codebase. `rotate_to` is named only in `ROADMAP.md` prose describing Phase 16/19's dependency on this phase — it does not exist in code. This means Phase 15 is building genuinely new API surface, not extending an existing one.

The good news is that the backward-compatibility fixture Test D-02 demands **already exists**: `crates/famp-keyring/tests/fixtures/two_peers.keyring` and its canonical sibling are real, already-committed, pre-v1.1-format files (written by Phase 3-era code, unrelated to this phase's writer) — exactly the "not the new writer's own output" fixture the CONTEXT.md insists on. The plan does not need to fabricate a new legacy fixture; it needs a new test that loads these two files with the new multi-key `Keyring::load_from_file` and asserts each principal resolves to exactly one active key with no expiry/revocation state, i.e. the old 2-field grammar is a strict subset of the new grammar.

The single most important architectural finding is about REVK-01/REVK-03: `famp-envelope`'s `expiry` field is self-asserted by the sender and only format-validated (`federation_format_ok` checks `expiry > ts`, nothing else — deliberately, since active anti-replay is a v1.1 concern per its own doc comment). If the keyring's revocation/expiry check is implemented by comparing the envelope's own `ts`/`expiry` against a `revoked_at` value, a compromised or malicious sender can simply keep asserting a pre-revocation `ts` forever — that is *exactly* the "pre-revocation replay window" REVK-03 forbids. The correct design checks the **pinned key's own state** (active/retired/revoked + validity window) against the **verifier's own wall-clock `now`, at verify time** — never against anything the sender supplied. This must be designed in from the start (it changes the record shape and the `verify.rs` call signature), which is exactly why D-09 insists on designing the on-disk shape once.

The gateway currently loads its `Keyring` **once at process startup** (`Arc<Keyring>`, no reload) — this is the same caching behavior that has already burned this project once (see `learned-rules.md`'s `daemon-keyring-cache-ordering` rule from the v0.8 era). Phase 15 does not need to add hot-reload; it is consistent with the existing `famp peer import` UX (which also requires a gateway restart to take effect) for `rotate`/`retire`/`revoke` to likewise require a restart. This should be stated explicitly rather than assumed, since it is not settled in CONTEXT.md.

**Primary recommendation:** Extend the on-disk grammar additively (old 2-field lines remain valid, one key entry per line, multiple lines per principal now permitted with a `state` discriminator), give `famp-keyring` a single `Keyring::active_key(principal, now: &str) -> Result<&TrustedVerifyingKey, KeyLookupOutcome>`-shaped query that takes "now" as an explicit parameter (never calls a clock internally — matches the `famp-bus` `Instant`-as-parameter convention already used in this codebase), and let `famp-gateway/src/verify.rs` map `KeyLookupOutcome` variants onto new `RejectReason` variants (`ExpiredKey`, `RevokedKey`) alongside the existing `UnpinnedKey`/`InvalidSignature`. Build the REVK-02 signed revocation statement on the existing `famp_crypto::sign_value`/`verify_value` (RFC 8785 JCS + `FAMP-sig-v1\0` domain prefix) — no new crypto primitive, no new dependency.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Multi-key storage, on-disk grammar, active/retired/revoked state machine | Data/Storage (`famp-keyring`) | — | `famp-keyring` already owns the sole on-disk trust-store format; this is a pure data-model + file-grammar extension, no network or process boundary involved |
| Rotate / retire / revoke mutation API (`pin_tofu`-successor surface) | Data/Storage (`famp-keyring`) | CLI (`famp peer rotate/retire/revoke`) | The library owns fail-closed state transitions; the CLI subcommand is a thin wrapper that also owns the KEYR-03 confirmation/exit-code UX, mirroring the existing `peer import` split (`import.rs` calls `keyring.pin_tofu` then maps errors to `CliError` variants) |
| Expiry check at verify time (REVK-01) | API/Backend (`famp-gateway::verify`) | Data/Storage (`famp-keyring`) | The *decision* ("is this key currently usable") is a pure function of keyring state + wall-clock time, best owned by `famp-keyring` as a testable query; `verify.rs` is the *only* caller and owns turning the outcome into a wire-facing `RejectReason` — this mirrors the existing `verify_inbound` two-gate shape (peek → keyring lookup → decode) |
| Signed revocation statement construction + verification (REVK-02) | Data/Storage (`famp-keyring`) or CLI | API/Backend (`famp-gateway::verify`) consumes it read-only | Revocation statements are canonicalized+signed JSON values (reuses `famp-crypto`), not a wire envelope — they belong beside the keyring they mutate, not inside `famp-envelope` (frozen) or as a new gateway-only concept |
| Gateway startup keyring load (no hot-reload) | API/Backend (`famp-gateway::main`) | — | Already the existing pattern (`Keyring::load_from_file` once in `main.rs`, held as `Arc<Keyring>`); Phase 15 does not need to change this boundary |

## Standard Stack

No new external crates are required for this phase. Every primitive needed already exists in the workspace:

### Core (already-dependency, reused)
| Library | Version | Purpose | Why Standard (for this phase) |
|---------|---------|---------|--------------------------------|
| `famp-crypto` | workspace 1.0.0 | `TrustedVerifyingKey`/`FampSigningKey`, `sign_value`/`verify_value`, `key_id` | Already `famp-keyring`'s crypto dependency; `sign_value`/`verify_value` (RFC 8785 JCS canonicalize + `FAMP-sig-v1\0` domain-separated Ed25519) is the exact primitive REVK-02's signed revocation statement needs — no new signing path |
| `famp_canonical::canonicalize` | workspace 1.0.0 (transitive via `famp-crypto::sign_value`) | RFC 8785 JCS canonicalization of the revocation statement JSON before signing | Reused transitively; no direct new dependency needed in `famp-keyring`'s `Cargo.toml` unless the revocation statement type construction needs it directly |
| `thiserror` | 2.0.18 (workspace) | `KeyringError` variants | Already `famp-keyring`'s only non-`famp-*` dependency |
| `famp-core::Principal` | workspace 1.0.0 | Hash key, `FromStr`/`Display` | Unchanged, already used throughout |

### Explicitly NOT needed
| Considered | Why rejected |
|------------|--------------|
| `time` or `chrono` crate for RFC 3339 parsing/comparison | `famp-envelope`'s Phase 08 decision (`[Phase 08]: expiry-vs-ts ordering uses lexical string comparison of byte-preserving RFC 3339 strings, no new dependency`, STATE.md) already established the pattern this phase should reuse: canonical-UTC-form-gated lexical string comparison. Parsing through a date library was explicitly rejected once already in this exact problem space; there is no new reason to revisit it. |
| A new `famp-keyring` → `famp-envelope` dependency edge, to reuse `Timestamp`/`shallow_validate`/`is_canonical_utc_form` | `shallow_validate` and `is_canonical_utc_form` are `pub(crate)` in `famp-envelope::timestamp` — not exported, so this dependency would not even compile against them. Re-implement the same ~15-line canonical-form gate locally in `famp-keyring` instead of adding a cross-crate dependency for code you can't call. `Timestamp` itself is `pub`, but the value it buys without the validation helpers is minimal. |
| serde/JSON for the on-disk keyring file itself | The existing format is a deliberately hand-rolled line grammar (see `file_format.rs`'s doc comment); D-02 requires the OLD grammar keep parsing, so the natural extension is additive fields on the same line grammar, not a rewrite to JSON. (The REVK-02 revocation *statement* is a JSON value signed via `sign_value` — a distinct, separate artifact from the keyring file itself.) |

**Installation:** none — no `Cargo.toml` dependency changes anticipated beyond possibly promoting `famp_canonical` to a direct (not transitive) `famp-keyring` dependency if the revocation-statement struct is built there rather than in the CLI layer.

## Package Legitimacy Audit

**Not applicable.** This phase adds zero new external packages. All primitives (signing, canonicalization, hashing) are reused from already-vendored, already-audited in-workspace crates (`famp-crypto`, `famp-canonical`). No `npm view`/`pip index`/`cargo search` verification is needed because nothing new is being installed.

## Architecture Patterns

### System Architecture Diagram

```
                     ┌─────────────────────────────┐
                     │  famp peer {import,rotate,   │   CLI layer (new/extended)
                     │  retire,revoke}  (crates/famp)│
                     └──────────────┬────────────────┘
                                    │ mutates on-disk file
                                    ▼
                     ┌─────────────────────────────┐
                     │  ~/.famp/gateway/peers.keyring│   On-disk trust store
                     │  (line grammar, additive v1.1)│   (famp-keyring owns format)
                     └──────────────┬────────────────┘
                                    │ Keyring::load_from_file
                                    │ (ONCE, at process startup — no hot reload)
                                    ▼
                     ┌─────────────────────────────┐
    inbound bytes ──▶│ famp-gateway::verify.rs      │
   (peeked `from`)   │  1. peek_sender (unverified)  │
                     │  2. Keyring::active_key(      │
                     │     principal, now) ───┐      │
                     │  3. SignedEnvelope::decode     │
                     │     (verify_strict)     │      │
                     └──────────────┬──────────┼──────┘
                                    │          │
                     ┌──────────────┴───┐   ┌──┴─────────────────────┐
                     │ Ok(SignedEnvelope)│   │ Err(RejectReason)       │
                     │  → onward to bus  │   │  UnpinnedKey            │
                     └───────────────────┘   │  ExpiredKey   (REVK-01) │
                                              │  RevokedKey   (REVK-02) │
                                              │  InvalidSignature       │
                                              └─────────────────────────┘

  Signed revocation statement (REVK-02), a side channel, NOT the envelope path:
  ┌───────────────────────┐   sign_value(new_active_key, stmt)   ┌───────────────────────┐
  │ Operator: `famp peer   │ ───────────────────────────────────▶│ Distributed same way   │
  │ revoke <principal>     │   (famp-crypto, RFC 8785 JCS +       │ as the original pin    │
  │ <old_key_id>`          │    FAMP-sig-v1\0 domain prefix)      │ (out-of-band, D-02)    │
  └───────────────────────┘                                      └───────────┬───────────┘
                                                                              │ verify_value
                                                                              ▼
                                                              famp peer import-revocation
                                                              (writes revoked state into
                                                               the SAME peers.keyring file)
```

### Recommended Project Structure

No new crates. Extend existing files/modules:

```
crates/famp-keyring/
├── src/
│   ├── lib.rs           # Keyring struct becomes HashMap<Principal, Vec<KeyEntry>>;
│   │                     # add rotate_to / retire / revoke / active_key methods
│   ├── file_format.rs    # extend parse_line/serialize_entry for additive
│   │                     # state + valid_until (+ revoked_at) fields
│   ├── entry.rs          # NEW: KeyEntry { key, state: KeyState, valid_until, pinned_at }
│   │                     # KeyState { Active, Retired, Revoked }
│   ├── revocation.rs     # NEW: RevocationStatement struct + sign/verify helpers
│   │                     # built on famp_crypto::sign_value/verify_value
│   ├── error.rs           # extend KeyringError with new variants (see below)
│   └── peer_flag.rs       # unchanged; already dead code in production (only
│                           # exercised by its own tests) — candidate to reuse
│                           # for `famp peer rotate --peer <...>=<pubkey>` CLI parsing
├── tests/
│   ├── fixtures/
│   │   ├── two_peers.keyring              # EXISTING — the D-02 backward-compat fixture
│   │   ├── two_peers.canonical.keyring    # EXISTING — canonical round-trip fixture
│   │   └── multi_key.canonical.keyring    # NEW — v1.1 canonical fixture (rotated peer)
│   ├── roundtrip.rs        # extend with a `keyr01_legacy_fixture_loads_as_single_active_key` test
│   ├── rotation.rs         # NEW — KEYR-02/03 rotate/retire/confirm tests
│   └── revocation.rs       # NEW — REVK-01/02/03 tests

crates/famp-gateway/
├── src/
│   ├── verify.rs         # extend RejectReason match arms; call
│   │                     # Keyring::active_key(from, now) instead of Keyring::get
│   └── error.rs           # add RejectReason::ExpiredKey / RevokedKey variants
└── tests/
    └── revocation.rs      # NEW — end-to-end verify_inbound rejection tests

crates/famp/src/cli/peer/
├── mod.rs                # add Rotate/Retire/Revoke subcommand variants
├── rotate.rs              # NEW — confirmation-gated key-change flow (KEYR-03)
├── retire.rs              # NEW — explicit retire-after-rotation
└── revoke.rs              # NEW — sign + write a revocation statement
```

### Pattern 1: Additive line grammar, not a format rewrite

**What:** Keep the existing `principal<sp><sp>pubkey_b64url` two-field line as a valid, fully-supported entry (implicitly `state=active`, no expiry enforced). New v1.1 writes add optional trailing whitespace-separated fields: `state`, `valid_until` (canonical UTC RFC 3339 or a literal `-` sentinel for "no expiry enforced" on legacy-shaped entries), and `pinned_at`.

**When to use:** Any time an on-disk format needs to gain fields while an existing fixture-proven reader must keep working without a migration script.

**Example (conceptual grammar, not yet implemented):**
```text
# legacy v0.7 line — still valid, treated as state=active, no expiry
agent:local/alice  iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w

# v1.1 line — explicit state + validity window
agent:local/bob  gTl3Dqh9F19Wo1Rmw0x-zMuNipG07jeiXfYPW4_Js5Q  active  2027-01-01T00:00:00Z  2026-07-31T00:00:00Z
agent:local/bob  <old-pubkey-b64url>  retired  2026-08-01T00:00:00Z  2026-04-01T00:00:00Z
```
Note `agent:local/bob` now legitimately has TWO lines — this REQUIRES dropping today's blanket `DuplicatePrincipal` reject (`crates/famp-keyring/src/lib.rs:68-73`) in favor of a duplicate check that is keyed on `(principal, state=Active)` — at most one **active** key per principal, any number of retired/revoked ones. This is the one genuinely load-bearing grammar change; get it explicit in the plan rather than discovering it mid-implementation.

### Pattern 2: `now` as an explicit parameter, never an internal clock call

**What:** Any function that decides "is this key still valid" takes `now: &str` (or a small `Now` newtype wrapping a canonical-UTC-form-validated string) as an argument, rather than calling `SystemTime::now()`/`Utc::now()` internally.

**When to use:** Every expiry/revocation decision point (`Keyring::active_key`, any REVK-03 test).

**Why this is the established convention here, not just good practice:** `famp-bus`'s broker step function takes `now: Instant` explicitly (`crates/famp-bus/src/broker/handle.rs:17,30`, `fn tick<E: BrokerEnv>(broker: &mut Broker<E>, now: Instant)`) specifically so tests can drive deterministic time without real sleeps — the exact same shape REVK-01/03's tests need ("sign an envelope pre-revocation, revoke, then attempt verify — must reject" is trivial to write deterministically if `now` is an argument, and painful/flaky if it calls a real clock). This also matches the project's own captured rule: *"Prefer data-as-input over synthetic-message-routing — take time/disconnect as input parameters rather than wire-layer forging synthetic messages back in"* (MEMORY.md).

**Example:**
```rust
// crates/famp-keyring/src/lib.rs (proposed)
pub enum KeyLookupOutcome<'a> {
    Active(&'a TrustedVerifyingKey),
    Unpinned,
    Expired { retired_at: Option<&'a str> },
    Revoked { revoked_at: &'a str },
}

impl Keyring {
    pub fn active_key(&self, principal: &Principal, now: &str) -> KeyLookupOutcome<'_> {
        // look up principal's Active-state entry; compare valid_until against `now`
        // using the SAME canonical-UTC-form lexical comparison famp-envelope
        // established (never parse through a date library — see Standard Stack).
    }
}
```

### Pattern 3: Confirmed rotation flow (KEYR-03's structurally distinct path)

**What:** `famp peer rotate` (new subcommand, NOT a silent behavior of `peer import`) detects that the principal already has an active key different from the incoming one, and requires an explicit confirmation signal before proceeding — never a bare warning line.

**When to use:** Any CLI mutation where "this would silently replace a previously-trusted credential" is the failure mode being defended against (the exact TOFU-collapse failure this whole phase exists to fix, per D-05).

**Concrete exit-code precedent already in this codebase:** `famp`'s CLI uses per-subcommand-family exit codes beyond the default 0/1 today — `cli/inspect/tasks.rs:33` uses `CliError::Exit(2)` ("broker not running"), `cli/await_cmd/mod.rs:143,229` use `CliError::Exit(3)` (await abort). These don't collide across subcommand families. Recommend: `famp peer rotate` without `--confirm` on a real key-change returns `CliError::Exit(2)` (distinct from `1` = generic malformed input, `0` = success) with a message that says `KEY CHANGED — <principal> was pinned to <old_key_id>, incoming key is <new_key_id>. Re-run with --confirm-rotation to accept.` — never a `warning:`-prefixed stderr line. First-sight pin (`peer import`, unchanged) stays exit 0 with no prompt at all, exactly as today.

### Anti-Patterns to Avoid

- **Comparing envelope `ts`/`expiry` against `revoked_at`:** see the dedicated Pitfall below — this is the single most important thing to get right in this phase and the one place a superficially-reasonable implementation is actually the vulnerability REVK-03 exists to close.
- **Silently accepting a second line for a principal without a state discriminator:** would make an old-format file with a genuine data-entry duplicate (today a `DuplicatePrincipal` hard error, correctly catching operator mistakes) silently "work" as a two-key entry — loses a real safety check. Any relaxation of `DuplicatePrincipal` must be scoped exactly to "two entries, different `state`," never "any duplicate."
- **Reusing `TlsFingerprintMismatch`/`TofuBootstrapRefused`-style orphaned error variants:** `cli/error.rs`'s own doc comment flags this as a past mistake ("RESEARCH Pitfall 5") — always add a fresh, narrowly-scoped error variant for a new failure mode rather than repurposing an unrelated one.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Signed revocation statement | A new ad-hoc signing scheme, or a new envelope-like wire type | `famp_crypto::sign_value` / `verify_value` (already canonicalizes via RFC 8785 JCS and domain-separates via `FAMP-sig-v1\0`) | This is precisely what these functions exist for — signing an arbitrary `serde::Serialize` value with the same substrate as envelopes, without needing envelope semantics (no `from`/`to`/class/FSM). Building a second signing path would duplicate INV-10's domain-separation guarantee and risk getting it wrong a second time. |
| RFC 3339 timestamp comparison for expiry | A `time`/`chrono` dependency, or naive `>=` on arbitrary strings | The canonical-UTC-form-gated lexical string comparison pattern already used by `famp-envelope::envelope.rs:537-556` (`federation_format_ok`) | Already solved once in this exact codebase, with a documented pitfall (`is_canonical_utc_form`'s doc comment explains exactly why raw lexical comparison is unsafe unless both operands are known-canonical first). Re-derive the ~15-line gate locally in `famp-keyring` rather than reaching for a date library — this was an explicit "no new dependency" decision on the same problem (Phase 08). |
| Multi-value trust-store key lookup | A new database, sqlite, or JSON-with-schema-versioning scheme | Extend the existing line-oriented grammar additively | `famp-keyring` is intentionally minimal (per its own module doc: "Narrow by absence... Pinning is sticky"); the crate is 278 lines total and every consumer (`famp-gateway`, `famp` CLI, examples, tests) expects `Keyring::load_from_file`/`save_to_file` semantics. A format rewrite is a much bigger, riskier change than an additive grammar extension, and D-02 explicitly forbids anything that isn't backward-compatible by construction. |

**Key insight:** Every primitive this phase needs (signing, canonicalization, timestamp handling) already exists and has already been battle-tested against a nearly identical problem (envelope `expiry`/`ts` ordering) in this exact codebase. The work here is data-model design and API surface, not cryptography or serialization research.

## Runtime State Inventory

> This phase changes an on-disk file format (single-key → multi-key + revocation fields), so a lightweight inventory is included even though it is not a rename/refactor phase.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **Real, live file confirmed on this dev machine**: `~/.famp/gateway/peers.keyring` contains one real 2-field entry (`agent:100.112.29.111/bob  E_Zdufp1CEcRCOGXO_apsqRQlsPlONC_NJm_zaBjSQ4`) — this is the Gate A dogfood pin, not a fixture. It must keep loading under the new reader. | No data migration script needed if the new grammar is additive (old 2-field lines are valid input); the file will organically upgrade to the new grammar the next time any mutation (`rotate`/`retire`/`revoke`) calls `save_to_file`, since `save_to_file` already rewrites the whole file in canonical form on every save. |
| Live service config | `famp-gateway` loads this keyring **once at process startup** (`crates/famp-gateway/src/main.rs:374-382`, `Arc<Keyring>`, never reloaded) — confirmed by direct source read and by the pre-existing `daemon-keyring-cache-ordering` learned rule (v0.8-era: "the daemon loads its peer keyring once at startup and does not re-read... afterward"). | Any CLI-driven rotate/retire/revoke requires an operator-run gateway restart to take effect, same as `famp peer import` today. Document this explicitly in the plan rather than silently assuming hot-reload; do not add file-watching in this phase unless CONTEXT.md is amended — it's out of the stated Layer 1/2 scope. |
| OS-registered state | None — `famp-keyring` is pure file I/O with no OS-level registration (no launchd/systemd unit, no scheduled task) touches this data. | None. |
| Secrets/env vars | None — the keyring stores only **public** verifying keys (`TrustedVerifyingKey`), never private signing keys. `gateway_identity_path`'s signing key is a separate file (`identity.ed25519`, mode 0600) untouched by this phase. | None. |
| Build artifacts | None — no compiled/generated artifact embeds keyring content. | None. |

## Common Pitfalls

### Pitfall 1: Never trust the envelope's own timestamp for revocation decisions

**What goes wrong:** Implementing REVK-01/REVK-03 by comparing `envelope.expiry`/`envelope.ts` (self-asserted by the sender, only format-validated by `federation_format_ok` — see `crates/famp-envelope/src/envelope.rs:519-537`'s own doc comment: *"D-04 well-formedness check ONLY... active anti-replay + expiry rejection are v1.1 concerns"*) against a `revoked_at` timestamp. A sender who controls the signing key (compromised or malicious) can simply assert any `ts` they like, including one that predates the revocation — the check would then "grandfather" every message, silently defeating REVK-03's entire purpose.

**Why it happens:** The envelope already carries `expiry`/`ts` fields (v1.0 forward-compat reservations), so it looks natural to reuse them for revocation math. But those fields describe when the SENDER claims the message is valid, not what the VERIFIER currently trusts.

**How to avoid:** The expiry/revocation decision must be a property of the **keyring's own state** (the pinned key's `valid_until`/`state` fields) evaluated against the **verifier's own wall-clock `now`** at the moment of verification — never against anything decoded from the envelope. This makes REVK-03's "no pre-revocation replay window" trivially true: once a key transitions to `Revoked`, `Keyring::active_key` returns `Revoked` for every subsequent verify call regardless of any envelope's claimed `ts`.

**Warning signs during plan review:** any task description that says "compare the envelope's expiry against the revocation timestamp" or "check if the envelope was signed before the revocation" is describing the wrong mechanism — flag it immediately, per D-16's "push back before implementing" mandate.

### Pitfall 2: Broadening `DuplicatePrincipal` too far

**What goes wrong:** The multi-key model requires more than one line per principal, which means the current blanket "second line for a known principal is always an error" check (`crates/famp-keyring/src/lib.rs:68-73`) must be relaxed. Relaxing it to "duplicates are always fine now" silently loses the original safety property: an operator's genuine copy-paste mistake (two identical lines, or two DIFFERENT keys for the same principal accidentally both marked active) would now load without complaint.

**Why it happens:** The easy fix ("just delete the duplicate check") is a one-line diff; the correct fix ("at most one Active entry per principal, arbitrary many Retired/Revoked") requires threading state through the loader's duplicate-detection logic.

**How to avoid:** Design the loader's invariant explicitly as "load fails if two entries for the same principal are BOTH in `Active` state" (a real corruption signal) — keep this as a named, tested case, not something incidentally dropped while making the grammar multi-key.

### Pitfall 3: Assuming the gateway hot-reloads keyring changes

**What goes wrong:** A plan or implementation that assumes `famp peer rotate`/`revoke` takes effect on a running gateway immediately will silently fail to protect anything — the gateway's `Arc<Keyring>` was loaded once at startup and never re-reads the file. This is the exact `daemon-keyring-cache-ordering` failure mode already documented in `learned-rules.md` from a prior incident in this codebase (v0.8 era, `peers.toml`).

**Why it happens:** Nothing in `verify.rs`'s public API signals that the keyring is static; a plan reasoning purely from "the file changed" without reading `main.rs` would miss it.

**How to avoid:** State explicitly in the phase's `## Reporting contract` output (D-16) that a gateway restart is required after any rotate/retire/revoke, exactly like `famp peer import` today. If the plan wants to close this gap, that is new scope beyond what CONTEXT.md asked for — flag it as an open question rather than quietly building it.

### Pitfall 4: Local test tooling — `cargo nextest` hangs on this machine

**What goes wrong:** Running `just ci`/`just test`/bare `cargo nextest run` stalls indefinitely in the test-binary `--list` phase (documented project-wide issue, `project_nextest_list_hang.md`; D-14 in CONTEXT.md repeats this).

**How to avoid:** Use `cargo test --workspace --no-fail-fast`, and per the separate `gsd-executor agents strand work by backgrounding a command and yielding` learned rule, run it in the FOREGROUND under an explicit `timeout`, not backgrounded-and-forgotten — report exact pass/fail/ignored counts from real output, never an assumed count.

## Code Examples

### Reuse `sign_value`/`verify_value` for the revocation statement (REVK-02)

```rust
// crates/famp-crypto/src/sign.rs (existing, unmodified — just showing the API this phase reuses)
pub fn sign_value<T: serde::Serialize + ?Sized>(
    signing_key: &FampSigningKey,
    value: &T,
) -> Result<FampSignature, CryptoError> {
    let canonical = famp_canonical::canonicalize(value)?;
    Ok(sign_canonical_bytes(signing_key, &canonical))
}
```
A `RevocationStatement { principal: Principal, revoked_key_id: String, revoked_at: String /* canonical UTC */, reason: Option<String> }` (proposed, not yet implemented) signs and verifies through this exact pair — no new crypto code, no new envelope type, and it stays entirely inside `famp-keyring`/CLI territory rather than touching the frozen `famp-envelope`.

### Existing two-gate verify shape to extend (`crates/famp-gateway/src/verify.rs:37-46`)

```rust
pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&from) else {
        return Err(RejectReason::UnpinnedKey { principal: from });
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}
```
Proposed extension point: replace `keyring.get(&from)` with `keyring.active_key(&from, now)`, matching on `KeyLookupOutcome` to produce `UnpinnedKey`/`ExpiredKey`/`RevokedKey` before falling through to the same `SignedEnvelope::decode` call. The existing "zero local-bus writes, zero state mutation on any path" contract (module doc, D-08 from Phase 8) must be preserved — the new lookup is a pure `&self` read, matching `Keyring::get`'s existing signature shape.

## State of the Art

| Old Approach | Current/Proposed Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `HashMap<Principal, TrustedVerifyingKey>`, exactly one key, TOFU-sticky, fatal on any conflict | `HashMap<Principal, Vec<KeyEntry>>` with explicit `Active`/`Retired`/`Revoked` state and validity window, at most one `Active` entry per principal | This phase (v1.1, Phase 15) | Unblocks Phase 16 (pairing) and Phase 19 (directory), both of which write through the same integration point per the roadmap's explicit "avoid a second migration" rationale |
| No revocation path at all — a compromised key can only be dealt with by manually editing/deleting the keyring file out-of-band | Validity-window expiry (primary) + signed revocation statement (defense-in-depth) | This phase | First real remediation path for a compromised/rotated peer key |

**Deprecated/outdated:** none — this is greenfield API surface within an existing crate, not a replacement of a previously-shipped mechanism.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The gateway does NOT need hot-reload of the keyring in this phase — a restart after rotate/retire/revoke is acceptable, matching existing `peer import` UX | Runtime State Inventory, Pitfall 3 | If Ben/famp-lead-730 actually expects live revocation to take effect without a restart (e.g. for a currently-running long-lived gateway process), the phase would need new state — a `RwLock<Keyring>` + a reload trigger — which is materially more scope than CONTEXT.md's stated Layer 1/2 file list implies. Low-cost to confirm before planning: ask directly. |
| A2 | A revocation statement (REVK-02) must be signed by a DIFFERENT already-trusted key for the same principal (e.g. the new key from a completed rotation), not by the key being revoked itself, and not by any third-party authority | "Reuse `sign_value`/`verify_value`" / Don't Hand-Roll | If the intended design is self-revocation (the compromised key signs its own revocation) or some other signer identity, the verification logic in `verify.rs`/`famp-keyring` needs a different authorization check. Not stated anywhere in CONTEXT.md, ROADMAP.md, or REQUIREMENTS.md — genuinely open, not just under-specified. |
| A3 | The on-disk grammar extension should stay in the existing line-oriented text format (additive fields) rather than migrating to a structured format (JSON/TOML) | Standard Stack, Don't Hand-Roll | If the plan or reviewer prefers a structured on-disk format for the richer state machine, D-02's backward-compat fixture test still holds (JSON parsing of an old 2-field-per-line file would just fail outright, which is NOT what D-02 wants) — so this assumption is actually load-bearing for D-02 compliance, not just a style preference. Flagged as an assumption because CONTEXT.md leaves the "exact serialization format" to Claude's Discretion, but the reasoning for staying in the line format specifically (rather than e.g. one-JSON-object-per-principal) is mine, not stated. |

## Open Questions

1. **Who is authorized to sign a REVK-02 revocation statement for a principal?**
   - What we know: the statement must be "verifiable and fail-closed," distributed "over the same channel as the original pin" (REQUIREMENTS.md REVK-02).
   - What's unclear: whether the signer must be a currently-active key for that same principal (my recommended default, A2 above), the principal's own about-to-be-revoked key (self-revocation), or something else entirely.
   - Recommendation: default to "a different currently-Active key for the same principal" (composes naturally with KEYR-02's rotate-then-retire flow: rotate in a new key first, then the new key signs the old key's revocation) and flag this explicitly for confirmation in the plan's first review pass, per D-16.

2. **Does the gateway need to pick up rotate/retire/revoke without a restart?**
   - What we know: today's `Arc<Keyring>` is loaded once at startup; `famp peer import` already requires a restart.
   - What's unclear: whether Ben/famp-lead-730 consider "revoke a key, restart the gateway" an acceptable remediation-path SLA, given REVK-01/02/03 are framed as security-critical.
   - Recommendation: keep the existing no-hot-reload pattern (A1) unless told otherwise — adding live reload is new scope, not implied by the stated `famp-keyring` + `verify.rs` constraint.

3. **Exact exit code value for KEYR-03's "key changed, confirmation required" path.**
   - What we know: exit codes 2 and 3 are already used by unrelated subcommand families (`inspect tasks`, `await`) with no global collision requirement — each subcommand family owns its own code space.
   - What's unclear: whether the plan should pick 2 (mirroring `inspect tasks`'s "actionable, needs a second step" precedent) or something else, and whether the confirmation should be an interactive stdin prompt vs. a `--confirm-rotation`-style flag (needed for non-interactive/scripted use, e.g. Phase 16's pairing flow eventually driving this same surface programmatically).
   - Recommendation: a `--confirm-rotation` flag (not an interactive prompt) for scriptability — Phase 16 (pairing) will almost certainly need to drive this surface non-interactively — with exit code 2 reserved for "would change a pinned key, rerun with --confirm-rotation."

## Environment Availability

Not applicable — this phase has no external tool/service dependencies (no databases, no network calls, no new CLIs). All work is in-process Rust file I/O and existing in-workspace crypto.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (built-in), NOT `cargo nextest` (hangs on this machine — D-14/learned-rules.md) |
| Config file | none — no `nextest.toml`/`.config/nextest.toml` gating relevant here |
| Quick run command | `cargo test -p famp-keyring --lib` / `cargo test -p famp-keyring --test roundtrip --test rotation --test revocation` |
| Full suite command | `cargo test --workspace --no-fail-fast` (foreground, under `timeout`, per D-14 and the backgrounding learned-rule) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KEYR-01 | Existing single-key fixture loads unchanged under new multi-key reader | unit/integration | `cargo test -p famp-keyring --test roundtrip -- keyr01` | Fixture files (`two_peers.keyring`, `two_peers.canonical.keyring`) exist; new test function does not — Wave 0 gap |
| KEYR-02 | Rotate pins new key without dropping old | unit/integration | `cargo test -p famp-keyring --test rotation` | ❌ Wave 0 — `rotation.rs` does not exist |
| KEYR-03 | Key-change path structurally distinct (exit code + confirmation) from first-pin | CLI integration | `cargo test -p famp --test peer_rotate_cli` (or similar, under `crates/famp/tests/`) | ❌ Wave 0 |
| REVK-01 | Expired key rejected at verify time regardless of revocation record | unit (deterministic `now` injection) | `cargo test -p famp-keyring --test revocation` and `cargo test -p famp-gateway --test revocation` | ❌ Wave 0 |
| REVK-02 | Signed revocation statement verifies, fail-closed | unit | `cargo test -p famp-keyring --test revocation -- revk02` | ❌ Wave 0 |
| REVK-03 | Sign pre-revocation → revoke → verify → must reject (falsification-style test, matching D-08's exact prescription) | integration | `cargo test -p famp-gateway --test revocation -- revk03_no_replay_window` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p famp-keyring --lib` (fast, no I/O beyond tempfiles)
- **Per wave merge:** `cargo test --workspace --no-fail-fast` (foreground, timeout-bounded)
- **Phase gate:** Full suite green, `just lint` clean (D-13), `just check-no-tokio-in-bus` green (D-13, though `famp-keyring`/`famp-gateway` don't touch `famp-bus` in this phase — should be a no-op check)

### Wave 0 Gaps
- [ ] `crates/famp-keyring/tests/rotation.rs` — covers KEYR-02
- [ ] `crates/famp-keyring/tests/revocation.rs` — covers REVK-01/02
- [ ] `crates/famp-gateway/tests/revocation.rs` — covers REVK-01/03 at the `verify_inbound` boundary
- [ ] `crates/famp/tests/` peer-rotate CLI integration test — covers KEYR-03's exit-code/confirmation contract
- [ ] New fixture: `crates/famp-keyring/tests/fixtures/multi_key.canonical.keyring` — a v1.1-format canonical fixture (rotated peer, one active + one retired entry) for round-trip testing of the NEW format, parallel to the existing `two_peers.canonical.keyring`
- [ ] Extend `crates/famp-keyring/tests/roundtrip.rs` with the KEYR-01 backward-compat assertion against the EXISTING `two_peers.keyring`/`two_peers.canonical.keyring` fixtures (no new fixture needed for this specific assertion)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | No user/password authentication in this phase — this is machine-to-machine public-key pinning, not credential auth |
| V3 Session Management | No | Not applicable — no session concept in the keyring/verify path |
| V4 Access Control | Partial | The trust decision itself (is this key currently authorized to speak for this principal) IS an access-control decision — `verify_inbound`'s fail-closed hard-reject on `UnpinnedKey` (no auto-pin, TRUST-02) is the existing control this phase extends with `ExpiredKey`/`RevokedKey` |
| V5 Input Validation | Yes | The on-disk keyring file grammar and the revocation statement are both untrusted-until-validated input surfaces — extend `file_format::parse_line`'s existing per-field validation (already rejects malformed principals/pubkeys with line numbers) rather than loosening it |
| V6 Cryptography | Yes | Ed25519 via `famp-crypto`, `verify_strict`-only (never plain `verify`), RFC 8785 JCS canonicalization before signing, `FAMP-sig-v1\0` domain separation — ALL already-established, reused verbatim for the revocation statement. Never hand-roll a signature scheme or comparison (see Don't Hand-Roll). |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Replay of a pre-revocation-signed envelope after the signer's key is revoked | Spoofing / Elevation of Privilege | Check key state (`Active`/`Retired`/`Revoked` + `valid_until`) against verifier's own `now` at verify time — never against the envelope's self-asserted `ts`/`expiry` (Pitfall 1, this doc) |
| Forged revocation statement (attacker revokes a legitimate peer's key to deny service, or "un-revokes" their own compromised key) | Tampering / Denial of Service | REVK-02 statements are themselves Ed25519-signed and `verify_strict`-verified against an already-pinned key for that principal (see Open Question 1 — the exact authorized-signer rule needs to be pinned down at plan time, but SOME already-trusted key must be the signer, never an unauthenticated claim) |
| Silent trust-anchor replacement (the TOFU-collapse failure this whole phase exists to fix) | Spoofing | KEYR-03's structurally distinct exit code + mandatory confirmation on any key change for a known principal (D-04/D-05) |
| Malformed/duplicate-entry keyring file corrupting the active-key set | Tampering | Extend, don't remove, `file_format::parse_line`'s per-field + per-line validation; keep a real "two Active entries for one principal" rejection (Pitfall 2) |

## Sources

### Primary (HIGH confidence — direct source reads this session)
- `crates/famp-keyring/src/lib.rs`, `file_format.rs`, `error.rs`, `peer_flag.rs` — full read, all 278 lines
- `crates/famp-keyring/tests/roundtrip.rs`, `tests/fixtures/two_peers.keyring`, `tests/fixtures/two_peers.canonical.keyring` — fixture-test convention confirmed
- `crates/famp-gateway/src/verify.rs` (full, 381 lines), `src/error.rs` (`RejectReason` enum), `src/main.rs` (keyring load-once-at-startup, lines ~350-400)
- `crates/famp/src/cli/peer/{mod.rs,import.rs,export.rs}`, `crates/famp/src/cli/error.rs` (exit-code precedent, `PeerKeyConflict` variant)
- `crates/famp-crypto/src/{lib.rs,keys.rs,sign.rs,verify.rs}` — `sign_value`/`verify_value`/`key_id`/`TrustedVerifyingKey` API surface
- `crates/famp-envelope/src/{envelope.rs,timestamp.rs}` — `federation_format_ok`, `shallow_validate`/`is_canonical_utc_form` (confirmed `pub(crate)`, not exported)
- `crates/famp-bus/src/broker/handle.rs` — `now: Instant` explicit-parameter convention
- Live grep across the workspace confirming `rotate_to` does not exist in any `.rs` file (only in ROADMAP.md prose) and `parse_peer_flag` is currently dead code in production
- `~/.famp/gateway/peers.keyring` — this machine's live, real single-key pre-v1.1 keyring file, read directly, confirming the backward-compat requirement is not hypothetical

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` (KEYR-01..03, REVK-01..03 full text, and the orchestrator scoping table's expiry-primary rationale)
- `.planning/ROADMAP.md` Phase 15 section
- `.planning/STATE.md` decision log entries for `[Phase 08]` (expiry-vs-ts lexical comparison, no new dependency) and the Phase 15 roadmap-creation rationale

### Tertiary (LOW confidence)
- None — no web search was performed or needed for this phase; every claim is either a direct code read or a project-internal planning document.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, everything verified against actual `Cargo.toml`/source
- Architecture: HIGH for the current-state description (direct reads); MEDIUM for the proposed `KeyEntry`/`active_key` shape (design proposal, not yet implemented — flagged as Claude's Discretion territory per CONTEXT.md, with open questions called out explicitly)
- Pitfalls: HIGH — Pitfall 1 (timestamp trust) is derived directly from `famp-envelope`'s own doc comments plus the requirement text; Pitfall 3 is a previously-documented incident in this exact codebase (`learned-rules.md`)

**Research date:** 2026-07-31
**Valid until:** No expiry pressure — this is an internal-codebase research doc for a phase about to be planned immediately; stays valid until Phase 15 either plans or the underlying source files change.
