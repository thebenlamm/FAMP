---
phase: 18-cross-person-trust-bootstrap-pairing
plan: 01
subsystem: security
tags: [pairing, bip39, ed25519, axum, keyring, tofu, cli]

requires:
  - phase: 15-keyring-multi-key-extension-revocation
    provides: "`pin_tofu`/`rotate_to`/`active_key` — the integration point pairing writes trust through, inheriting its fail-closed guarantees"
  - phase: 17-protocol-grade-ingress-reachability-implementation
    provides: "the gateway ingress tier and the shared 1 MiB RequestBodyLimitLayer the pairing route is merged before"
provides:
  - "Vendored BIP-39 English wordlist (2048 words) with a measured, test-asserted SHA-256 pin"
  - "`famp::pairing` — uniform code draw, constant-time digest compare, atomic invite store under an O_CREAT|O_EXCL lock"
  - "`famp pair invite|redeem|status` CLI with the code reachable only via stdin"
  - "`POST /famp/v1/pair/redeem` — a dedicated unauthenticated route on the inviter's own gateway"
  - "End-to-end proof that one texted five-word code pins two machines mutually"
affects: [18-02, 18-03, 19-auto-wake-gate, 20-human-acceptance-gate]

actuals:
  tokens: 71000
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Vendored data file + measured-SHA-256 pin re-asserted on every test run"
    - "Structural secret-exclusion: no argv-capable field, rather than a runtime argv check"
key-files:
  created:
    - crates/famp/src/pairing/bip39-english.txt
    - crates/famp/src/pairing/wordlist.rs
    - crates/famp/src/pairing/invite.rs
    - crates/famp/src/pairing/mod.rs
    - crates/famp/src/cli/pair/{mod,invite,redeem,status}.rs
    - crates/famp-gateway/src/pairing_ingress.rs
    - crates/famp-gateway/tests/pairing_e2e.rs
  modified:
    - crates/famp-gateway/src/{lib,ingress,main}.rs
    - crates/famp/src/{lib.rs,cli/mod.rs,bin/famp.rs}

key-decisions:
  - "Rendezvous transport = Option A (pre-resolved by Ben 2026-08-03, not re-asked): a dedicated unauthenticated POST /famp/v1/pair/redeem on the inviter's OWN gateway. famp-relay (Option B) was rejected — its enqueue 404s until a domain is manually pre-registered, and verify_inbound_any rejects the unpinned senders every pairing peer is by definition."
  - "BIP-39 wordlist vendored as a data file, NOT the `bip39` crate — the crate carries mnemonic checksum + PBKDF2 machinery a 5-independent-word code does not use, and whose checksum validation would wrongly reject valid pairing codes."
  - "Upstream license: RESOLVED by Ben 2026-08-03 — BIP-39's MIT license permits vendoring this data file. Recorded attributed and dated in wordlist.rs's header, with the verbatim findings retained beneath it."
  - "save_atomic is new code, deliberately NOT a copy of Keyring::save_to_file — the latter is a plain File::create + write_all with no rename and no fsync, which is exactly the torn-write exposure T-18-08 exists to close."

patterns-established:
  - "Constant-time compare: fixed-length XOR-accumulate over all 32 bytes, no early return — compare time independent of matching prefix length"
  - "Persist-before-reply: state transition is written through save_atomic under StoreLock BEFORE the HTTP response is constructed, so a process kill cannot resurrect a consumed invite"
  - "Unauthenticated-surface minimization: the route 404s whenever no Pending invite exists, so it is live only during a real pairing window"

requirements-completed: [PAIR-01, PAIR-06]

coverage:
  - id: D1
    description: "One texted five-word code pins two machines mutually — both keyrings hold the other side's key"
    requirement: PAIR-01
    verification:
      - kind: e2e
        ref: "cargo test -p famp-gateway --test pairing_e2e#happy_path_pins_both_sides_mutually"
        status: pass
  - id: D2
    description: "The pairing code cannot be passed as a command-line argument"
    requirement: PAIR-06
    verification:
      - kind: e2e
        ref: "cargo test -p famp-gateway --test pairing_e2e#redeem_argv_refuses_positional_code"
        status: pass
  - id: D3
    description: "A wrong code leaves the invite store byte-identical and Pending"
    requirement: PAIR-01
    verification:
      - kind: e2e
        ref: "cargo test -p famp-gateway --test pairing_e2e#wrong_code_leaves_store_byte_identical_and_pending"
        status: pass
  - id: D4
    description: "Uniform 11-bits/word draw with no modulo bias; wordlist integrity pinned by measured digest"
    requirement: PAIR-01
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib pairing#uniform_draw_covers_every_index_with_no_out_of_range, #wordlist_matches_pinned_sha256"
        status: pass
  - id: D5
    description: "The unauthenticated pairing surface is dark when no invite is outstanding, and refuses a redeemer claiming the inviter's own domain"
    requirement: PAIR-01
    verification:
      - kind: e2e
        ref: "cargo test -p famp-gateway --test pairing_e2e#no_pending_invite_when_store_is_absent, #own_domain_refused_when_redeemer_claims_inviter_authority"
        status: pass
---

## Accomplishments

- **Vendored the BIP-39 English wordlist** (2048 words, LF endings) as repo data with zero new
  runtime dependencies. Its SHA-256 was measured with `shasum -a 256` — never copied from a spec
  document — and pinned as `WORDLIST_SHA256`, re-asserted against the `include_str!`-ed bytes on
  every test run.
- **Built `famp::pairing`.** `draw_code` goes through `rand::Rng::gen_range(0..2048)`, which
  rejection-samples internally, so the ~55-bit entropy claim is true rather than approximately
  true. `digests_equal` is a fixed-length XOR-accumulate with no early return. `save_atomic`
  writes a mode-0600 temp file, `sync_all`s it, renames over the target, then `sync_all`s the
  parent directory; every read-modify-write is serialized by an `O_CREAT|O_EXCL` `StoreLock`.
- **Built the `famp pair` CLI** matching the existing `famp peer` idiom exactly (`run`/`run_at`
  split, typed `CliError`, exit-code discipline).
- **Shipped the pairing route** as a dedicated Router merged before the shared 1 MiB body cap.

## Deviations

1. **Three files outside the plan's `files_modified`** were touched, all reviewed and kept:
   - `crates/famp/src/bin/famp.rs` — `reqwest` and `famp-transport-http` became real (non-dev)
     dependencies of the lib target, so their `#[cfg(test)]`-gated unused-crate-dependency shims
     had to go unconditional.
   - `crates/famp-gateway/Cargo.toml` — added `rand` + `clap` dev-deps that `pairing_e2e.rs` needs.
   - `crates/famp-gateway/tests/inbound_destination_validation.rs` — one added `None,` argument, a
     mechanical call-site update from the new harness parameter.

2. **`#[allow(clippy::future_not_send)]` on `pair::redeem::{run, run_at}` and `pair::mod`.**
   Diagnosed rather than blanket-allowed: the non-`Send` value is `StdinLock` (a
   `std::sync::MutexGuard`), reached because PAIR-06 forces the code through stdin. It is **not**
   the pairing `StoreLock`. `block_on_async` calls `tokio::Runtime::block_on`, which polls the
   future exclusively on the calling thread and never spawns it onto the work-stealing pool, so
   the guard never needs to cross a thread boundary while held. This follows four existing repo
   precedents (`cli/hook/emit.rs` ×2, `cli/hook/codex_stop.rs` ×2, referenced by
   `cli/inbox/list.rs`) that hold `&mut dyn Write` across awaits for the same reason.
   No lint configuration was weakened anywhere.

## Execution note (process, not product)

Two dispatched executors stalled at the 600s no-progress watchdog without committing, because
`just lint` is a multi-minute silent clippy run and a fix-one-lint-then-re-lint loop trips the
watchdog every cycle. The second agent had in fact fixed all 18 nursery lints before dying; its
work was recovered from the working tree rather than rerun. **Lesson for later plans in this
phase: batch all lint fixes, then run `just lint` once.**

## Verification

| Check | Command | Result |
|-------|---------|--------|
| Pairing unit tests | `cargo test -p famp --lib pairing` | 12 passed, 0 failed |
| Pairing e2e | `cargo test -p famp-gateway --test pairing_e2e` | 5 passed, 0 failed |
| Lint (nursery + pedantic) | `just lint` | exit 0 |
| Format | `cargo fmt --all --check` | clean |
| Layer 0 freeze | `git status --porcelain \| grep famp-(envelope\|canonical\|crypto\|core\|fsm)` | no matches — freeze held |

`cargo nextest` hangs on this repo, so `just ci`/`just test` were not used; per-crate
`cargo test` was substituted per the plan's own constraint.

## Self-Check: PASSED

## Open for later plans

- Attempt counter, TTL enforcement, `famp pair revoke`, single-use-across-restart → **18-02**
- Error taxonomy, consent-before-code artifact ordering, observe-before-pin done-signals → **18-03**
- PAIR-05's comprehension half is not mechanically assertable; closes at Phase 20's UAT-02.
- A new pin is durable but not active until `famp daemon restart` — the same gap `peer rotate`
  and `peer revoke` already ship under.
