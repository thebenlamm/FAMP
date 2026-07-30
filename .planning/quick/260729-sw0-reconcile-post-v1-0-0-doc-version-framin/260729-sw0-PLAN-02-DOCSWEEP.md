---
phase: quick-260729-sw0
plan: 02
type: execute
wave: 2
depends_on: [01]
files_modified:
  - .planning/PROJECT.md
  - README.md
  - docs/GETTING-STARTED.md
  - docs/ONBOARDING.md
  - docs/DEVELOPMENT.md
  - docs/CONFIGURATION.md
  - docs/GATEWAY-SETUP.md
autonomous: true
---

# Plan 02 — Full live-doc sweep (wave 2)

Wave 1 (plan 01) fixed README/ARCHITECTURE/CLAUDE.md version framing. This wave
fixes (a) the **managed-block sources** so wave 1's CLAUDE.md fix cannot be
reverted by `/gsd-docs-update`, and (b) every remaining verified factual defect
in the live user-facing doc set.

Every finding below was verified by the orchestrator against actual source or the
actual binary. The verification command is recorded per item.

## Scope boundary

IN: the files listed in `files_modified`.
OUT (deliberately — dated historical records; editing them falsifies history):
`REFACTORING-REVIEW-2026-*.md`, `ADVERSARIAL-REVIEW-refactors.md`,
`beta-feedback-*`, `.codebase-review/*`, `.planning/forensics/*`,
`.planning/quick/*/SUMMARY.md`, `RETROSPECTIVE.md`, `docs/adr/*`,
`docs/MIGRATION-v0.8-to-v0.9.md`, `docs/MIGRATION-v0.9-to-v0.10.md`,
`docs/superpowers/**`, `FAMP-v0.5.1-spec.md`.
DEFERRED to wave 3 (pending the root/crate-doc audit): `CONTRIBUTING.md`,
`SECURITY.md`, `crates/famp-crypto/README.md`, slash-command assets, `SKILL.md`,
and all `crates/*/Cargo.toml` `description` fields — the last of these is gated
by `Justfile:206 check-spec-version-coherence`, which greps descriptions, so it
must not be touched until that gate's exact contract is known.

## FROZEN — must remain byte-identical

`docs/GATEWAY-SETUP.md` is gated by `crates/famp/tests/gateway_setup_doc_accuracy.rs`
(21 assertions). `README.md` is gated by that test plus
`crates/famp/tests/readme_line_count_gate.rs`. Confirmed frozen and NOT touched by
any edit below:

| Literal | Gate |
|---|---|
| `Federation gateway (v1.0, shipped)` | gateway_setup_doc_accuracy.rs:331 (README) |
| `## Quick Start` + first ```bash fence (<=12 lines; contains `cargo install famp`, `famp install-claude-code`, `/famp-register`) | readme_line_count_gate.rs |
| `brew install famp` / `/famp-msg` must NEVER appear in README | readme_line_count_gate.rs (negative) |
| `famp peer export --as`, `famp peer import`, `CA:FALSE`, `serverAuth`, `FAMP_OWN_DOMAIN`, `own-domain`, `famp send --to agent:`, `socketfilterfw`, `sender AGENT principal`, ``backs the remote principal `bob``, ``backs the remote principal `alice``, `keyring loads once`, `after the keyring has loaded prints`, `A zero exit code confirms only that the local broker accepted the envelope into the gateway-backed outbound mailbox on this host.`, `It does not confirm that the gateway has drained, signed, and relayed the envelope`, `That is the fire-and-forget boundary.` | gateway_setup_doc_accuracy.rs (GATEWAY-SETUP.md) |
| regex `agent:[^\s]*/gateway` must NOT match GATEWAY-SETUP.md | gateway_setup_doc_accuracy.rs:176-194 |
| ordering: `loads its keyring` before `famp-gateway: ready`; the "confirms only" paragraph between `famp send --to agent:hostB.example/bob` and `famp inspect tasks --id` | gateway_setup_doc_accuracy.rs |

The single GATEWAY-SETUP.md edit (E12) touches line 295 only, which the audit
confirmed is not among the frozen assertions.

---

## Task 1 — managed-block sources (BLOCKER: wave 1's CLAUDE.md fix is reversible without this)

### E1 · `.planning/PROJECT.md:167` — conformance target
Verified: `grep -n "vector pack" .planning/PROJECT.md CLAUDE.md`. Feeds `CLAUDE.md:15`
via `<!-- GSD:project-start source:PROJECT.md -->`. Source still asserts the vector
pack shipped in v1.0; it did not (Gate B open).

OLD:
```
- **Conformance target**: Staged conformance is supported — each milestone tags conformance level achieved; vector pack ships in v1.0 alongside federation gateway. (Revised 2026-04-27 in v0.9 prep sprint T6; see `.planning/V0-9-PREP-SPRINT.md` for context. Original constraint was "Level 2 + Level 3 in one milestone" — superseded by the local-first reframe and the absence of a named second implementer at v0.5.1 wrap.)
```
NEW:
```
- **Conformance target**: Staged conformance is supported — each milestone tags conformance level achieved; the vector pack did NOT ship in v1.0 — it is gated on a second implementer committing to interop (Gate B, still open). (Revised 2026-04-27 in v0.9 prep sprint T6, re-confirmed 2026-07-29 at v1.0 close; see `.planning/V0-9-PREP-SPRINT.md` for context. Original constraint was "Level 2 + Level 3 in one milestone" — superseded by the local-first reframe and the absence of a named second implementer.)
```

### E2 · `.planning/PROJECT.md:168` — spec fidelity
Verified: repo spec authority is v0.5.2 (`FAMP_SPEC_VERSION`), and `CLAUDE.md:16`
already renders v0.5.2. Source still says v0.5.1 → regen would revert.

OLD:
```
- **Spec fidelity**: v0.5.1 fork is the authority for this implementation. All diffs from v0.5 documented with reviewer rationale.
```
NEW:
```
- **Spec fidelity**: v0.5.2 is the authority for this implementation (the v0.5.1 fork amended with the `audit_log` `MessageClass`, which does not fire the task FSM, shipped alongside v0.9 Phase 1). All diffs from v0.5 documented with reviewer rationale.
```

### E3 · `.planning/PROJECT.md` — 8-tool vs 12-tool self-contradiction
Verified: `crates/famp/src/cli/mcp/server.rs:489` test
`tool_descriptors_has_exactly_twelve_named_tools` asserts 12. PROJECT.md states
"Stable 12-tool MCP surface" in one paragraph and "8-tool MCP surface" ~8 lines
later. Not inside a managed block; fix the stale one.

OLD (the `8-tool` sentence only):
```
8-tool MCP surface stable across v0.8 → v0.9 → v1.0.
```
NEW:
```
MCP surface grew from 8 tools (v0.9) to 12 (current); the contract is stable across v0.8 → v0.9 → v1.0, the count is not.
```

---

## Task 2 — README remaining factual defects

### E4 · `README.md:305-308` — MCP tool count (BLOCKER)
Verified: 12 tools, enumerated at `crates/famp/src/cli/mcp/server.rs:510-523`;
anti-drift test at :489. README is the last hardcoded stale count in the repo.

OLD:
```
FAMP ships an MCP stdio server (`famp mcp`) that exposes eight tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_await`, `famp_inbox`,
`famp_peers`, `famp_join`, `famp_leave`. The model: **one MCP server config
per client; the window picks an identity at runtime via `famp_register`.**
```
NEW:
```
FAMP ships an MCP stdio server (`famp mcp`) that exposes twelve tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_await`, `famp_inbox`,
`famp_peers`, `famp_join`, `famp_leave`, `famp_channel_log`, `famp_set_listen`,
`famp_verify`, `famp_inspect_waiters`. The authoritative list is enumerated at
runtime via the MCP `tools/list` method. The model: **one MCP server config
per client; the window picks an identity at runtime via `famp_register`.**
```

### E5 · `README.md:707-718` — Repo Layout lists 9 of 16 crates (BLOCKER)
Verified: `ls crates/` = 16. Missing `famp-bus`, `famp-gateway`, `famp-inbox`,
`famp-inspect-client`, `famp-inspect-proto`, `famp-inspect-server`, `famp-taskdir`.
Roles derived from each crate's `Cargo.toml` description + `src/lib.rs` header.

Replace the whole bullet list under `## Repo Layout` with all 16 crates, in the
order shown, keeping the existing bullet style:
```
- `crates/famp`: runtime glue, CLI, MCP stdio server, examples, and
  cross-crate integration tests (umbrella crate + binary)
- `crates/famp-bus`: Layer 1 local bus — pure-actor UDS broker protocol
  primitives, tokio-free
- `crates/famp-canonical`: RFC 8785 canonical JSON wrapper and conformance gate
- `crates/famp-core`: `Principal`, `Instance`, UUIDv7 IDs, `ArtifactId`,
  `ProtocolErrorKind`, invariants
- `crates/famp-crypto`: Ed25519 sign/verify, base64url codecs, worked vectors
- `crates/famp-envelope`: signed envelope types and five shipped message bodies
- `crates/famp-fsm`: minimal 5-state task FSM
- `crates/famp-gateway`: Layer 2 federation gateway (shipped in v1.0) — proxies
  remote principals onto the local bus over the signed cross-host wire
- `crates/famp-inbox`: durable JSONL inbox (append with fsync, tail-tolerant read)
- `crates/famp-inspect-client`: read-only Inspector RPC client (UDS, async)
- `crates/famp-inspect-proto`: Inspector RPC request/response types (no I/O)
- `crates/famp-inspect-server`: Inspector RPC server handlers (tokio-free,
  read-only, mounted by the broker)
- `crates/famp-keyring`: TOFU keyring file format and peer parsing
- `crates/famp-taskdir`: per-task TOML storage primitive (atomic replace + fsync)
- `crates/famp-transport`: transport trait + `MemoryTransport`
- `crates/famp-transport-http`: minimal HTTPS transport binding
```

---

## Task 3 — docs/ guides

### E6 · `docs/GETTING-STARTED.md:121` — documented command is BROKEN (BLOCKER)
Verified: `cargo run -q -p famp -- inbox --as bob` → `error: unexpected argument
'--as' found`. `inbox` is a subcommand group; `--as` belongs to `inbox list`
(confirmed via `famp inbox list --help`).

OLD: `famp inbox --as bob`
NEW: `famp inbox list --as bob`

### E7 · `docs/ONBOARDING.md:5` — federation framed as future (STALE)
Verified: `grep -n 'lands in v1.0' docs/ONBOARDING.md`.

OLD: `a shared local broker. Federation across machines lands in v1.0.`
NEW: `a shared local broker. Federation across machines shipped in v1.0 via`
     `` `famp-gateway`; see [GATEWAY-SETUP.md](GATEWAY-SETUP.md). ``

### E8 · `docs/DEVELOPMENT.md:82` — wrong crate count (STALE)
Verified: `grep -c '"crates/' Cargo.toml` → 16.

OLD: ``The workspace root `Cargo.toml` lists 15 crates under `crates/`. Three``
NEW: ``The workspace root `Cargo.toml` lists 16 crates under `crates/`. Three``

### E9 · `docs/DEVELOPMENT.md` — crate tables omit `famp-bus` and `famp-gateway` (BLOCKER)
Verified: both crates exist and build; `grep -c 'famp-gateway\|famp-bus'
docs/DEVELOPMENT.md` → 2 (only incidental mentions, absent from the tables).
A v1.0-era contributor reading "the crates that matter" does not find the
headline v1.0 crate.

Add to the "Protocol primitives" table (after the `famp-transport` row):
```
| `famp-bus` | Layer 1 local bus — pure-actor UDS broker core, tokio-free |
```
Add to the "Federation internals" table (after the `famp-transport-http` row):
```
| `famp-gateway` | Proxies remote principals onto the local bus over the signed cross-host wire (shipped v1.0) |
```
If adding `famp-bus` to "Protocol primitives" makes the surrounding "Three
groups" wording wrong, keep the count consistent with whatever grouping results.

### E10 · `docs/DEVELOPMENT.md:281` — PR policy reads as a pending gate (STALE)
OLD:
```
External PRs are welcome from v1.0 onward; until then, file issues rather
than opening PRs (see CONTRIBUTING.md).
```
NEW:
```
External PRs are welcome now that v1.0 has shipped (see CONTRIBUTING.md).
```
NOTE: cross-check `CONTRIBUTING.md` for the same "until v1.0" gate. If
CONTRIBUTING.md still says PRs are closed, do NOT silently diverge the two —
report the conflict instead of guessing which policy the maintainer wants.

### E11 · `docs/CONFIGURATION.md` — v0.8 fossils documented as live + v1.0 surface missing
All verified against source:
- `famp init` does NOT exist (`cargo run -q -p famp -- init --help` → `error:
  unrecognized subcommand 'init'`). Remove it from the `FAMP_HOME` row.
- `famp listen` does NOT exist (same check → `unrecognized subcommand 'listen'`).
  `listen_addr`'s only live consumer is `crates/famp/src/cli/info.rs:86`
  (`format!("https://{}", config.listen_addr)`, builds the peer-card endpoint).
- `peers.toml` is NOT read by send: `grep -rn 'peers.toml\|PeersConfig'
  crates/famp/src/cli/send/` → zero hits.
- `FAMP_OWN_DOMAIN` IS real: `crates/famp/src/cli/own_domain.rs:39`, precedence
  `--domain` flag > `FAMP_OWN_DOMAIN` env > `$FAMP_HOME/own-domain` file >
  `Err(OwnDomainNotSet)` (own_domain.rs:16-17).

E11a — `FAMP_HOME` row: drop `init`, add the v1.0 gateway paths.
OLD: ``Override the identity home directory used by v0.8 federation commands (`init`, `info`, `config.toml`, `peers.toml`, keypair files).``
NEW: ``Override the identity home directory used by v0.8 federation artifacts (`info`, `config.toml`, `peers.toml`, keypair files) and by v1.0 gateway federation (the `own-domain` file and the gateway identity/keyring under `$FAMP_HOME`).``

E11b — `listen_addr` row.
OLD: ``TCP address for the v0.8 HTTPS federation listener (`famp listen`).``
NEW: ``v0.8-era HTTPS listen address. `famp listen` was removed in v0.9; today this field is consumed only by `famp info`, to build the peer-card `endpoint`.``

E11c — `alias` row.
OLD: ``Local nickname used in `famp send --to <alias>`. Must be unique.``
NEW: ``Local nickname for this peer entry. v0.8 artifact — not consulted by the current `famp send --to`, which resolves bare names via the broker and `agent:<domain>/<name>` via the gateway. Must be unique.``

E11d — intro completeness claim + add a `FAMP_OWN_DOMAIN` env row.
The intro claims to cover "every runtime configuration knob" but
`grep -n 'FAMP_OWN_DOMAIN' docs/CONFIGURATION.md` → zero hits, while
GETTING-STARTED.md points here as the authoritative flag/env reference.
Amend the intro to name the v1.0 gateway surface and point at GATEWAY-SETUP.md,
AND add a real row to the env-var table:
```
| `FAMP_OWN_DOMAIN` | No | (none) | This host's own-domain authority for v1.0 gateway federation. Precedence: `--domain` flag > `FAMP_OWN_DOMAIN` > `$FAMP_HOME/own-domain` file; unset in all three is an error. See docs/GATEWAY-SETUP.md. |
```

### E12 · `docs/GATEWAY-SETUP.md:295` — names an undefined milestone (STALE)
Next milestone is not yet defined; do not present v1.1 as the committed fix
target. Audit confirmed this line is NOT one of the 21 frozen assertions.

OLD: `**Known limitation (leaf-name ambiguity, deferred to v1.1).** A remote send`
NEW: `**Known limitation (leaf-name ambiguity, not yet resolved).** A remote send`

---

## Verification (run ALL; record literal output)

`cargo nextest` HANGS in this repo and `just ci`/`just test` are unusable locally —
use plain `cargo test`. `cargo test <filter>` EXITS 0 ON ZERO MATCHES, so confirm
the passed-count, never accept an empty run as a pass.

1. `cargo test -p famp --test readme_line_count_gate --test gateway_setup_doc_accuracy`
   → MUST report `3 passed` and `1 passed`. Any zero-count run is a FAILURE.
2. `cargo test -p famp tool_descriptors_has_exactly_twelve_named_tools` → 1 passed.
3. `just check-spec-version-coherence` → must pass (proves no crate description drifted).
4. `cargo run -q -p famp -- inbox list --as bob 2>&1 | head -3` → must NOT contain
   `unexpected argument`, proving E6's replacement command is real.
5. `grep -c '^- \`crates/' README.md` → 16.
6. `grep -n 'eight tools\|15 crates\|lands in v1.0\|deferred to v1.1\|from v1.0 onward' README.md docs/*.md`
   → MUST be empty.
7. `grep -n 'vector pack ships in v1.0\|v0.5.1 fork is the authority' .planning/PROJECT.md`
   → MUST be empty.
8. `git diff --name-only` → exactly the 7 files in `files_modified`.
