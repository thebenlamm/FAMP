# Wrap FAMP this week as the Reference Implementation of FAMP v0.5.1

> **STATUS: DEFERRED to v1.0 (decision recorded 2026-04-26 evening).**
>
> This plan was drafted earlier 2026-04-26 in plan-mode. It is **not the active path forward.** The vector pack ships at **Gate B** (2nd implementer commits to interop) per [`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`](../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md). Note that Gate B is independent of Gate A (gateway shipping) — the welded trigger that bundled them has been retired. Sofer remains a candidate, but Gate B fires for any 2nd implementer.
>
> Rationale: vectors prove interop between two implementations; there is no second implementation yet, so the pack is notation until one exists. v0.9's local-bus broker reuses Layer 0 primitives unchanged — vectors aren't blocked by v0.9 and v0.9 doesn't block them.
>
> Active path: `.planning/V0-9-PREP-SPRINT.md` (T1-T9) → `/gsd-new-milestone v0.9`. Full context in `.planning/RETROSPECTIVE.md` "v0.9 Prep Sprint and the Sofer Field Report" section.
>
> **Do not pick this up as written.** When the v1.0 trigger fires, this plan is the starting reference for the actual vector-pack work — but the surrounding context (CLAUDE.md L2+L3 constraint already revised in prep-sprint T6, federation gateway in flight) will reshape it.
>
> --- Original plan below ---

---

## ⚠️ First decision tomorrow: scope conflict with CLAUDE.md

While drafting this plan, the project `CLAUDE.md` was updated/restored to include this constraint under **Project → Constraints**:

> **Conformance target**: Level 2 + Level 3 in one milestone. **Level 1-only is explicitly not a release target.**

This plan as written ships **Conformance Level 1** (canonical JSON + crypto + envelope + happy-path FSM) and explicitly defers Level 2/3 vectors (card rotation §6.3, competing-instance §11.5a, clock-skew δ §13.1, delegation §11–12, provenance §14.2) to a follow-up release.

That contradicts the documented constraint. **Three coherent ways to resolve:**

1. **Honor the constraint as written** — expand this week's scope to Level 2 + 3. Per Day-2 explorer estimate, that's ~9–10 business days minimum (5–7 for L1 + 3–5 for the missing L2 vectors), realistically ~3 weeks once L3 (provenance, delegation ceiling, extensions registry) is added. **Not a one-week wrap.**
2. **Revise the constraint in CLAUDE.md** to allow staged conformance — ship L1 as `v0.8.1`, L2 as `v0.9.x`, L3 as `v1.0.x`. Aligns with the framing settled in the conversation ("reference implementation, finishable, move on clean"). **One-week wrap stays viable.**
3. **Drop the test-vector pack as the done-bar entirely** — pick a different falsifiable bar (e.g., interop with a hand-written Python verifier; or just the formal release tag + CHANGELOG). Cheaper, weaker craft claim. Probably wrong direction given how close L1 vectors already are.

**Recommendation: option 2.** The CLAUDE.md constraint reflects an earlier moment when the bet was "ship a single conformance milestone." The conversation that produced this plan changed the bet ("just me, build the reference impl, finish it"). The constraint should follow the bet, not lead it. But this is Ben's call — make it before Day 1 starts.

---

## Context

After a long conversation with the-architect and zed-velocity-engineer, Ben acknowledged plainly: *"It's just me at this point. But I do want to create the ultimate working protocol implementation."*

The framing settled on:
- **Drop "ultimate"** (unbounded, perfectionism trap) → **"the reference implementation of FAMP v0.5.1"** (bounded, finishable).
- mcp_agent_mail is **not a competitor** — service product, different bet. Steal leases as a protocol primitive *later*; not in scope this week.
- v0.9 (local-first UDS broker) and v1.0 (federation gateway) are **out of scope** for this week. They remain as forward roadmap, not blockers.
- Pick a **falsifiable done-bar** so the project has a defined "complete" state. Selected: **a publishable test-vector pack** any second implementer can run against their own impl to claim v0.5.1 conformance.

Two parallel exploration agents confirmed:
- ~95% of the v0.5.1 spec is already shipped across the workspace crates.
- Existing vectors cover RFC 8785 canonical JSON, RFC 8032 Ed25519, §7.1c worked envelope signature, base64url roundtrip, FSM determinism.
- The main gap is **5 packaged FSM-round-trip envelope vectors** + a JSON pack schema + a runner + Conformance section in README + CHANGELOG + tagged release.
- The in-flight phase **01-03 (session-bound MCP identity)** just landed code (six MCP tools, `IdentityBinding`, `FAMP_LOCAL_ROOT`); needs verification + summary doc.

Intended outcome: by end of week, `famp v0.8.1` is tagged on `main`, README declares Conformance Level 1 against v0.5.1, a `vectors/` pack and runner exist in-tree, CHANGELOG is written, and Ben can close the laptop without anything dangling.

## Approach

Five-day sprint. Each day has a single shape so it's hard to slip.

### Day 1 (Mon) — Land 01-03, lock the framing

**Verify in-flight phase 01-03 (session-bound MCP identity).** Code shipped over commits `1f8ef00` → `6636f5f`. Run the new + updated tests and confirm green:
- `crates/famp/tests/mcp_pre_registration_gating.rs`
- `crates/famp/tests/mcp_register_whoami.rs` (per the agent report; verify it exists and passes)
- `crates/famp/tests/mcp_stdio_tool_calls.rs` (updated harness)
- `crates/famp/tests/mcp_error_kind_exhaustive.rs`
- Full `just ci` clean.

Write `01-03-SUMMARY.md` under `.planning/phases/` so the phase closes per GSD convention. Mark phase 01 done in `.planning/STATE.md` (and `MILESTONES.md` if appropriate).

**Lock the framing in code:**
- `README.md:7` — replace the Status line: `v0.8.1 — Reference Implementation of FAMP v0.5.1 (Conformance Level 1: canonical JSON · crypto · envelope · core FSM)`. *(Adjust phrasing per the conflict-resolution decision above.)*
- `README.md` — collapse the "v0.9 / v1.0" forward-looking sections into a single short *Roadmap (post-v0.5.1)* block. Stop selling unshipped milestones.
- Add a placeholder *Conformance* section heading (filled Day 4).

**Decide the v1 vector-pack scope (locked in this plan, pending the constraint-conflict resolution above):** canonical-JSON corpus + crypto vectors + §7.1c worked envelope + 5 happy-path FSM round-trip envelopes (request → commit → deliver → ack → terminal). **Not** in v1: card rotation, competing-instance tiebreak, clock-skew δ rule, delegation. Those are v1.1 of the pack — explicitly tracked in `vectors/ROADMAP.md` so future-Ben isn't confused.

### Day 2 (Tue) — Vector pack scaffolding + collect existing vectors

Create `vectors/` at workspace root (sibling to `crates/`). Layout:
```
vectors/
  README.md            # what this is, how to run it, schema doc
  schema.json          # JSON schema for a vector entry
  ROADMAP.md           # v1 scope vs v1.1 deferrals
  v1/
    canonical-json/    # RFC 8785 corpus, one file per vector
    crypto/            # RFC 8032 + base64url + weak-key + worked-example
    envelope/          # §7.1c vector_zero
    fsm/               # placeholders (filled Day 3)
```

**Reuse, do not regenerate:** copy/symlink existing fixtures from:
- `crates/famp-canonical/tests/vectors/` (RFC 8785 corpus + cyberphone weird.json)
- `crates/famp-envelope/tests/vectors/vector_0/` (§7.1c canonical.hex, signature.hex, envelope.json)
- `crates/famp-crypto/tests/rfc8032_vectors.rs` test data → extract to data files
- `crates/famp-crypto/tests/base64_roundtrip.rs` test data
- `crates/famp-fsm/tests/deterministic.rs` transition matrix → table form

Each vector file: a JSON envelope conforming to `schema.json` with `id`, `spec_section`, `category`, `inputs`, `expected`, `verifies`, `provenance` (where the bytes came from — RFC, cyberphone repo, etc.).

Critical: **no vector bytes are computed by FAMP itself.** Provenance always points to an external authority (RFC test, cyberphone, Python jcs lib). This is what makes the pack credible to a second implementer.

### Day 3 (Wed) — Author the FSM envelope vectors + runner

**Author 5 FSM round-trip envelope vectors** under `vectors/v1/fsm/`. Use the deterministic Ed25519 keypair from RFC 8032 Test 1 (already used in §7.1c) so signatures are reproducible:
1. `01-request-to-REQUESTED.json` — unsigned envelope + canonical bytes + signature + expected post-state
2. `02-commit-to-COMMITTED.json`
3. `03-deliver-to-COMPLETED.json`
4. `04-ack-terminal.json`
5. `05-control-cancel-to-CANCELLED.json` — covers terminal absorption from non-completed branch

For each: pre-sign with `famp-crypto`, freeze the bytes, document the FSM transition assertion. **The signing happens once, by hand-with-script; the vectors then live as bytes-on-disk.**

**Build the runner** — new bin crate `crates/famp-conform` (small, ~200 lines):
- `famp-conform run --pack vectors/v1` → loads schema, walks each vector, dispatches to the appropriate verification function (canonicalize-and-compare, verify-signature, simulate-fsm-transition).
- Exit code 0 = pass, 1 = fail. Per-vector PASS/FAIL line.
- Reuses `famp-canonical`, `famp-crypto`, `famp-envelope`, `famp-fsm` directly — zero duplication.
- Add `famp-conform run --pack vectors/v1` to `just ci` so regressions break the build.

### Day 4 (Thu) — Docs: CHANGELOG + Conformance + script docs

**Write `CHANGELOG.md`** (root). Follow Keep-a-Changelog format. Backfill from git tags (`v0.5.1 spec fork` → `v0.6 foundation crates` → `v0.7 personal runtime` → `v0.8 usable` → `v0.8.1 conformance`). Each entry: 3–6 bullets of what shipped, mapped to spec sections.

**Write the Conformance section in `README.md`** (replace placeholder from Day 1):
- Define **Conformance Level 1**: passes `vectors/v1` end-to-end. Lists which spec sections are exercised (§4a, §7.1, §8a, §9–12 happy path).
- Explicitly state what Level 1 does **not** cover (card rotation §6.3, competing-instance §11.5a, clock-skew δ §13.1, delegation §11–12 advanced). These are L2/L3 territory and tracked in `vectors/ROADMAP.md`.
- Show the one-line `famp-conform run --pack vectors/v1` command.
- Invite second implementers: *"If your impl runs the same pack and gets all green, you are FAMP v0.5.1 Level 1 conformant."*

**Write operator docs** (short, `docs/`):
- `docs/operations/redeploy-listeners.md` — what `scripts/redeploy-listeners.sh` does, when to use, guard semantics. (~1 page from the script header comments.)
- `docs/operations/famp-local.md` — already partially in README; cross-link, don't duplicate.

### Day 5 (Fri) — Release

**Pre-release checklist:**
- `just ci` green.
- `famp-conform run --pack vectors/v1` green.
- Working tree clean.
- README + CHANGELOG land in one final commit.

**Tag and ship:**
- `git tag -a v0.8.1 -m "FAMP v0.8.1 — Reference Implementation of FAMP v0.5.1 (Conformance Level 1)"`
- `git push origin v0.8.1`
- Create GitHub Release from the tag, body = the v0.8.1 CHANGELOG section verbatim.

**Close-out artifacts:**
- Update `.planning/STATE.md` to "v0.5.1 reference implementation complete; v0.9 is optional follow-on."
- Open a GitHub issue titled "v0.5.1 Conformance Level 2 (post-release work)" listing the deferred vectors from `vectors/ROADMAP.md` so they're visible without being on a deadline.
- One short blog-style note in `docs/history/` (or similar): *Why this is done.* Anchors the framing so the perfectionism trap doesn't reopen the project two weeks later.

## Critical files to modify

- `README.md` — Status line, collapse roadmap section, add Conformance section.
- `CHANGELOG.md` — new file, retroactive history.
- `vectors/` — new directory tree (README, schema.json, ROADMAP.md, v1/ subdirs).
- `crates/famp-conform/` — new small bin crate (Cargo.toml, src/main.rs).
- `Cargo.toml` (workspace root) — add `crates/famp-conform` to members.
- `Justfile` — add `just conform` recipe; wire into `just ci`.
- `docs/operations/redeploy-listeners.md` — new.
- `docs/operations/famp-local.md` — new (or stub linking to README).
- `.planning/phases/01-session-bound-mcp-identity/01-03-SUMMARY.md` — close phase 01-03.
- `.planning/STATE.md` — milestone close-out.

## Existing assets to reuse (do not rebuild)

- `crates/famp-canonical/tests/vectors/` + `tests/conformance.rs` — RFC 8785 corpus (Appendices B/C/E + cyberphone weird.json).
- `crates/famp-envelope/tests/vectors/vector_0/` — §7.1c worked example (canonical.hex, signature.hex, envelope.json).
- `crates/famp-crypto/tests/rfc8032_vectors.rs` — Ed25519 standard vectors.
- `crates/famp-crypto/tests/worked_example.rs` — full sign→verify roundtrip.
- `crates/famp-fsm/tests/deterministic.rs` + `proptest_matrix.rs` — FSM truth table.
- `crates/famp/src/cli/mcp/{session,tools/register,tools/whoami}.rs` — already-landed 01-03 work; just verify, don't touch.
- `scripts/famp-local`, `scripts/redeploy-listeners.sh`, `scripts/spec-lint.sh` — already production-grade; just document.

## Verification (end-of-week acceptance criteria)

1. `just ci` exits 0.
2. `cargo run -p famp-conform -- run --pack vectors/v1` exits 0 with one PASS line per vector.
3. `git tag` shows `v0.8.1`; `gh release view v0.8.1` returns the release.
4. `README.md` Status line reads "Reference Implementation of FAMP v0.5.1 (Conformance Level 1)".
5. `CHANGELOG.md` exists with a `v0.8.1` section.
6. `vectors/README.md` describes how a second implementer runs the pack.
7. `.planning/STATE.md` says "v0.5.1 reference implementation complete."
8. `.planning/phases/01-session-bound-mcp-identity/01-03-SUMMARY.md` exists.
9. **Second-implementer dry run:** pick a Python script, canonicalize one of the `vectors/v1/canonical-json/` inputs with `python -c "import jcs; ..."`, confirm bytes match. (10-minute sanity check; proves the pack is portable.)

## Out of scope (explicit, pending the constraint-conflict resolution at top)

- v0.9 local-first bus, UDS broker, channels.
- v1.0 federation gateway, Agent Cards, delegation, provenance graph.
- File-reservation leases borrowed from mcp_agent_mail.
- Conformance Level 2/3 vectors (card rotation, competing-instance, clock-skew δ).
- Web UI, FTS5 search index, macros for smaller models.
- Forking `serde_jcs` to `famp-canonical` standalone.

These remain valid future work, tracked in `vectors/ROADMAP.md` and the post-release GitHub issue. None of them blocks declaring v0.5.1 reference status — *unless* CLAUDE.md's "L2+L3 in one milestone" constraint is held as written, in which case most of them re-enter scope.

## Tomorrow's first three moves

1. Resolve the L1-vs-L2+L3 constraint conflict (top of this doc).
2. `/gsd-plan-phase` to convert the resolved scope into a real phase under `.planning/phases/`.
3. Day 1 work: verify 01-03 tests, write 01-03-SUMMARY.md, lock the README Status line.
