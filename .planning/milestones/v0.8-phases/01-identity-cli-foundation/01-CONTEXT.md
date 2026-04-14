# Phase 1: Identity & CLI Foundation — Context

**Gathered:** 2026-04-14
**Status:** Ready for planning
**Milestone:** v0.8 Usable from Claude Code

<domain>
## Phase Boundary

Turn `crates/famp/src/bin/famp.rs` from its 8-line `println!` placeholder into a real subcommand-dispatching binary whose first and only shipped subcommand — `famp init` — materializes a persistent `FAMP_HOME` directory containing an Ed25519 keypair, a self-signed TLS cert/key pair, a minimal `config.toml`, and an empty `peers.toml`. Every later v0.8 subcommand will read its identity from this directory.

**In scope:** `famp init`, `FAMP_HOME` resolution, identity directory layout, the CLI dispatch skeleton that Phases 2–4 will plug into.

**Explicitly out of scope (belongs to later phases):** `famp listen` (Phase 2), `famp send`/`await`/`inbox`/`peer add` (Phase 3), `famp mcp` (Phase 4), `.well-known` Agent Card distribution (v0.9), richer error UX (colors/hints), XDG compliance, Windows path handling.

</domain>

<decisions>
## Implementation Decisions

### CLI Framework & Code Layout
- **D-01:** CLI parsing uses **`clap` derive**. No hand-rolled parser, no `argh`. Derive macros on a `Cli` struct + `Subcommand` enum in `crates/famp/src/cli/mod.rs`.
- **D-02:** **Subcommand implementations live in the lib crate** under a new `pub mod cli` in `crates/famp/src/lib.rs` (e.g. `famp::cli::init::run(home: &Path) -> Result<InitOutcome, CliError>`). The binary at `crates/famp/src/bin/famp.rs` stays ~20 lines: parse clap args, call `famp::cli::run(args)`, map the returned `Result` to a process exit code. This makes every subcommand reachable from integration tests without `assert_cmd` subprocess overhead.
- **D-03:** **`main` → `run()` → subcommand shape.** `main()` is a trivial wrapper that calls `famp::cli::run(Cli::parse())` and converts the returned `Result<(), CliError>` to an exit code via `eprintln!("{e}")` + `std::process::exit(1)` on error.
- **D-04:** A small **typed `CliError` enum in `crates/famp/src/cli/error.rs`** using `thiserror::Error`. No `anyhow` in the binary path — even though CLAUDE.md permits it in bins, the project bias is narrow typed errors and we want each init failure to be matchable in tests. Expected variants (non-exhaustive, planner refines): `HomeNotAbsolute { path }`, `HomeCreateFailed { path, source: io::Error }`, `AlreadyInitialized { existing_files: Vec<PathBuf> }`, `IdentityIncomplete { missing: PathBuf }`, `KeygenFailed(source)`, `CertgenFailed(source)`, `Io { path, source }`, `TomlSerialize(source)`.
- **D-05:** `CliError` carries **no private key material** in any variant. Error construction sites must never embed raw key bytes, filesystem contents of `key.ed25519`, or anything derived from them. The `#[source]` chain is the only error plumbing; no `format!` of key material into `Display`.

### Identity Directory Layout & FAMP_HOME Resolution
- **D-06:** **Flat directory layout, exactly as the v0.8 roadmap spec names the files.** No `keys/` or `tls/` subdirectories in Phase 1. The six entries directly under FAMP_HOME are:
  - `key.ed25519` — raw 32-byte Ed25519 private key, mode 0600
  - `pub.ed25519` — raw 32-byte Ed25519 public key, mode 0644
  - `tls.cert.pem` — self-signed cert, mode 0644
  - `tls.key.pem` — cert private key in PEM, mode 0600
  - `config.toml` — mode 0644
  - `peers.toml` — empty file on init, mode 0644
- **D-07:** **FAMP_HOME resolution rule (single precedence chain):**
  1. If `$FAMP_HOME` is set → use it verbatim.
  2. Otherwise, use `$HOME/.famp`.
  3. No XDG_CONFIG_HOME support in Phase 1.
  4. No tilde expansion — that's the shell's job. A literal `~` in `FAMP_HOME` is treated as a filesystem name, not expanded.
- **D-08:** **Absolute paths only.** If the resolved FAMP_HOME is relative (e.g. `FAMP_HOME=./foo`), `init` exits non-zero with `CliError::HomeNotAbsolute`. No `canonicalize` fallback — we prefer a loud error over silent promotion. `$HOME/.famp` is always absolute by construction.
- **D-09:** **`init` creates the directory if missing, with mode 0700.** This applies to both the explicit `FAMP_HOME=/tmp/foo` case and the default `$HOME/.famp` case. If the parent of the resolved home does not exist, init fails (we do not `mkdir -p` arbitrary ancestor chains).
- **D-10:** **`init` refuses any non-empty FAMP_HOME without `--force`.** Partial state is still refused; init does not backfill individual missing files. The failure returns `CliError::AlreadyInitialized { existing_files }` listing every entry it found, so the user sees exactly what's in the way. `--force` wipes and rewrites **atomically**: write all six files into a tempdir inside the same filesystem (`tempfile::TempDir::new_in(parent)`), then rename-swap the tempdir over the target, then delete the old directory. This guarantees the user never sees a half-written FAMP_HOME after a crashed `--force`.
- **D-11:** **Non-init subcommands (Phase 2+) return `CliError::IdentityIncomplete { missing }`** — a distinct variant from `AlreadyInitialized` — when they try to load FAMP_HOME and find a missing file. This gives two clean failure modes: one for "don't init over existing state" and one for "identity is broken, re-init".

### Config & Peers File Contents
- **D-12:** **`config.toml` contains strictly one field in Phase 1:** `listen_addr = "127.0.0.1:8443"`. No `log_level`, no `principal_alias`, no speculative hooks. Every future field is added by the phase that uses it.
- **D-13:** The config struct uses `#[serde(deny_unknown_fields)]` so a stray key added by hand fails the load with a typed error. (Matches v0.6 convention across every other serde-backed type in the workspace.)
- **D-14:** `peers.toml` on init is **a zero-byte file**, not a file containing `[]` or `peers = []`. The empty file deserializes cleanly into the empty-peers representation. The struct is also `#[serde(deny_unknown_fields)]`.

### First-Run UX & Output Discipline
- **D-15:** **`famp init` on success writes two lines total:**
  - **stdout (exactly one line):** the newly generated public key, base64url-unpadded, as raw bytes (same format `famp-keyring` already uses for Principal), followed by `\n`. Nothing else. This makes `famp init | famp peer add alice --pubkey -` trivially pipeable in later phases.
  - **stderr (one human-readable line):** `initialized FAMP home at <absolute path>\n`. No banner, no ASCII art, no "welcome" copy, no hint about next steps in Phase 1.
- **D-16:** **`famp init` on failure writes the `thiserror::Display` of the `CliError` to stderr** and exits non-zero. No colors, no unicode decorations, no "Did you mean..." hints in Phase 1. Richer error UX is explicitly deferred — the planner can note it as a future polish phase.
- **D-17:** **Private-key leakage defense is structural, not decorative.** Three mechanisms stack:
  1. **Verify (don't add) that `famp-crypto::FampSigningKey` has no `Display` impl and no `Debug` impl that prints byte material.** Phase 1 adds a compile-time check or doc-test that asserts this — if someone later derives `Debug` on `FampSigningKey`, Phase 1's test fails.
  2. **No `CliError` variant embeds key bytes** (D-05). Enforced by reading the error enum once during plan review.
  3. **One integration test (`tests/init_no_leak.rs`)** runs `famp::cli::init::run` against a tempdir FAMP_HOME, then reads `key.ed25519` back from disk and asserts that neither captured stdout nor captured stderr contains any 8+ byte substring of the private key. The test runs in the lib crate (because of D-02) so no subprocess machinery is needed.
- **D-18:** **No `zeroize-on-drop` work in Phase 1.** Zeroize protects against core dumps and memory scraping, which is an orthogonal threat model to "don't print secrets". If we want it later we add it deliberately; adding it now dilutes the signal of Phase 1's actual leakage guarantee.

### Claude's Discretion
- **CD-01:** Exact layout of the `cli` module tree (`cli/mod.rs`, `cli/init.rs`, `cli/error.rs`, etc.) — the planner picks the file split that keeps each file <200 lines.
- **CD-02:** Whether `InitOutcome` is a struct, an enum, or `()`. The only hard requirement is that the bin has enough information to print the one pubkey line on stdout.
- **CD-03:** Whether `--force` is a top-level `famp init --force` flag or a subcommand-scoped arg. (The former is the obvious clap-derive shape.)
- **CD-04:** The exact `toml` crate choice (`toml` vs `toml_edit` vs `basic-toml`). The planner picks based on serde integration quality and dep weight; `toml = "0.8"` is the most likely default. Whichever is picked must honor `deny_unknown_fields` on load.
- **CD-05:** Whether the integration test uses `tempfile::TempDir` + `std::env::set_var` or a test helper that threads FAMP_HOME through the Rust API without touching process env. The Rust-API route is cleaner but the planner can decide based on how `FAMP_HOME` is plumbed.

### Folded Todos
_None — no pending todos matched Phase 1 scope._

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Milestone & requirements
- `.planning/ROADMAP.md` §v0.8 Phase 1 — phase goal, depends-on list, 5 success criteria, requirement IDs (CLI-01, CLI-07, IDENT-01..06). This is the scope anchor.
- `.planning/REQUIREMENTS.md` — full text of CLI-01, CLI-07, IDENT-01, IDENT-02, IDENT-03, IDENT-04, IDENT-05, IDENT-06. Read every acceptance criterion before writing tests.
- `.planning/STATE.md` — current milestone position, carried-forward decisions.
- `.planning/PROJECT.md` — project-level principles and the "byte-exact substrate first" stance.

### Spec
- `specs/FAMP-v0.5.1-spec.md` — authority. Phase 1 only needs §7.1 (key format, raw 32-byte pubkey) but the planner should re-read §2 (Principal) so `init`'s output format stays consistent with what `famp-keyring` already consumes. _(If the path differs from `specs/`, the planner updates this ref during RESEARCH.md.)_

### v0.7 substrate this phase depends on
- `crates/famp-crypto/src/lib.rs` — `FampSigningKey`, `FampVerifyingKey`, keygen entry points. The D-17 leakage test asserts against the `Debug`/`Display` impls here.
- `crates/famp-keyring/src/lib.rs` — existing raw-32-byte + base64url-unpadded Principal encoding that `famp init`'s stdout line must match byte-for-byte.
- `crates/famp-transport-http/src/lib.rs` — where the generated `tls.cert.pem` + `tls.key.pem` will be consumed in Phase 2. The cert produced by `init` must be loadable by this crate's rustls setup without modification.
- `crates/famp/src/bin/famp.rs` — the 8-line placeholder being replaced.
- `crates/famp/src/lib.rs` — current public re-exports; the planner adds `pub mod cli` here.

### Workspace conventions
- `CLAUDE.md` §Technology Stack — locked-in crate versions (clap not yet listed — planner adds it), `rustls`-only, no OpenSSL.
- `CLAUDE.md` §11 — `thiserror` in libs, `anyhow` permitted in bins (but Phase 1 chooses typed `CliError` anyway — see D-04).
- `.planning/SEED-001.md` — `serde_jcs` conformance gate, not directly relevant to Phase 1 but confirms "serde + deny_unknown_fields everywhere" is non-negotiable.

### Crate docs to read during research (use context7 where available)
- `clap` 4.x derive API (`#[derive(Parser)]`, `#[derive(Subcommand)]`, `#[command(...)]`).
- `rcgen` 0.14 self-signed cert generation API — specifically which key algorithms it supports and whether Ed25519 TLS certs are reliably loadable by `rustls-platform-verifier`. _(This is the research task the cert-parameters gray area was deferred on; see §deferred.)_
- `toml` serde integration, `deny_unknown_fields` behavior.
- `tempfile::TempDir::new_in` for the atomic `--force` rewrite.
- Unix file-mode APIs (`std::os::unix::fs::PermissionsExt`) for 0600/0700 — note that Phase 1 is Unix-only in practice; the planner documents the Windows story explicitly as "not supported in v0.8".

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`famp-crypto::FampSigningKey` / keygen path** — Phase 1 calls the existing keygen, writes the raw 32 bytes. No new crypto code.
- **`famp-keyring` Principal encoding** — Phase 1's stdout line reuses the exact base64url-unpadded format `famp-keyring` already load/saves, so Phase 3's `famp peer add` will be symmetric.
- **`famp-transport-http` TLS loading path** — already knows how to consume a PEM cert+key pair via rustls. Phase 1's `rcgen` output must satisfy whatever this loader expects. (Research task: confirm key algorithm constraints.)
- **`crates/famp/src/runtime/`** — existing module layout inside the lib crate. `pub mod cli` is added as a sibling of `pub mod runtime`.

### Established Patterns
- **Narrow typed errors via `thiserror`** — every existing member crate does this. `CliError` follows the same shape as `ProtocolError`, `RuntimeError`, `EnvelopeDecodeError`.
- **`#[serde(deny_unknown_fields)]` on every on-wire/on-disk struct** — config.toml and peers.toml inherit this convention.
- **Byte-exact round-trip tests** — the v0.7 keyring has one; Phase 1 adds a similar one for `config.toml` (write → read → assert equal).
- **Examples live under `crates/famp/examples/`** — Phase 1 probably does NOT add a new example; it adds a test and grows the bin. If the planner decides a `cli_init_walkthrough` example is worthwhile it can justify in PLAN.md.

### Integration Points
- `crates/famp/src/lib.rs` line ~52 (`pub mod runtime;`) is where `pub mod cli;` lands.
- `crates/famp/src/bin/famp.rs` (whole file) is rewritten.
- `crates/famp/Cargo.toml` grows new deps: `clap` (with `derive` feature), `rcgen`, `toml`, `dirs` or equivalent for `$HOME` resolution, `tempfile` (maybe already present via dev-deps — the planner checks).
- Workspace `Cargo.toml` `[workspace.dependencies]` grows those crates if any other member needs them later (planner's call).

</code_context>

<specifics>
## Specific Ideas from the User

- **Decisive preference for narrow typed errors over `anyhow` even in the binary** — the user explicitly rejected the "anyhow is fine in bins" escape hatch for this phase. Keep every failure mode matchable in tests.
- **"Minimal but not silent" first-run output** — the user's exact words. Pubkey on stdout, one `initialized ...` line on stderr, nothing else. No banners, no hints, no hand-holding.
- **Leakage defense framed as "structural separation, not acknowledgment"** — matches the CLAUDE.md anti-pattern rule. The user specifically named the three mechanisms (no Display on key type, no key bytes in errors, scan test). Don't substitute a weaker check.
- **Flat directory layout justified by "inventing a hierarchy before we have enough files"** — this is a deliberate anti-premature-abstraction stance. The planner should not add `keys/` or `tls/` subdirs even if it "feels cleaner".
- **Phase 1 is the foundation every later subcommand builds on, not a one-off** — D-02's "in the lib crate" choice is driven by this. Every subcommand added in Phases 2–4 will be another module under `famp::cli::`.

</specifics>

<deferred>
## Deferred Ideas

### TLS cert parameters (Gray Area 3 — deferred by user)
The user explicitly deferred locking the cert story in discussion, with a provisional stance of "conservative compatibility over purity". The planner should treat this as a **research task** in RESEARCH.md and return a short comparison, then pick during planning:

- **Key algorithm:** Ed25519 (simplest, matches the protocol) vs ECDSA P-256 (broadest rustls compatibility) vs RSA-2048 (deprecated for new work). **Research question:** does `rcgen 0.14` + `rustls 0.23` + `rustls-platform-verifier 0.5` load an Ed25519 self-signed cert without warnings or special features? If yes, pick Ed25519 for elegance. If no, fall back to ECDSA P-256.
- **SANs:** `localhost`, `127.0.0.1`, `::1`. (Provisional — confirm nothing in v0.8 Phase 2 needs a wider SAN list.)
- **CN:** boring placeholder (e.g. `"famp-local"`) — it's a self-signed personal cert, the CN carries no meaning.
- **Validity window:** long but finite. Default proposal: 10 years (3650 days). Avoids both "forever" (bad hygiene) and "1 year" (annoying re-init for a personal tool). The planner picks a concrete number in PLAN.md.
- **Serial number:** random 128-bit, per `rcgen` defaults.

These parameters are NOT locked in CONTEXT.md — they go in RESEARCH.md and then into PLAN.md as a single decision with rationale.

### Richer error UX (colors, hints, "Did you mean...")
Out of scope for Phase 1 per D-16. May become its own polish phase later in v0.8 or v0.9.

### XDG_CONFIG_HOME compliance
Deferred per D-07. If a user asks for it after v0.8 ships, we add it as a new precedence layer without breaking FAMP_HOME override semantics.

### Windows path/mode handling
Phase 1 is Unix-only in practice (0600/0700 modes, `$HOME`, PEM files in known locations). The planner documents this explicitly in PLAN.md's limitations section rather than pretending portability.

### `zeroize-on-drop` for in-memory key material
Deferred per D-18. Orthogonal threat model; add deliberately later if needed.

### Reviewed Todos (not folded)
_None — no pending todos surfaced as candidates for Phase 1._

</deferred>

---

*Phase: 01-identity-cli-foundation*
*Milestone: v0.8 Usable from Claude Code*
*Context gathered: 2026-04-14*
