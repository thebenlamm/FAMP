# Phase 12: v1.0.0 Release Gate - Pattern Map

**Mapped:** 2026-07-29
**Files analyzed:** 8 code/doc/manifest files (REL-01, REL-03, REL-05); REL-04 hygiene edits noted separately (no code analog)
**Analogs found:** 5 / 5 applicable

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|-----------------|----------------|
| `crates/famp/tests/gateway_setup_doc_accuracy.rs` (EXTEND, not create) | test (doc-accuracy) | request-response (subprocess `--help` + string assertions) | itself — existing assertion blocks in the same file | exact (self-pattern) |
| `docs/GATEWAY-SETUP.md` §6 (insert paragraph) | doc | n/a | itself — surrounding §5/§6 prose | exact (self-pattern) |
| `crates/famp/src/cli/send/mod.rs` (`SendArgs` doc comments) | CLI arg struct (clap) | request-response | itself — existing field doc comments (e.g. `terminal`, `domain` fields) | exact (self-pattern) |
| `README.md` (remote-path / "Not Shipped Yet" correction) | doc | n/a | itself — "Not Shipped Yet" section, `## What Works Today` bullets | exact (self-pattern) |
| `crates/famp/src/cli/mod.rs` (`BANNER_ABOUT` const + `version_strings_unified` test) | config/const + test | transform (string literal → compiled banner) | itself — no external analog needed, both sides already co-located | exact (self-pattern) |
| `Cargo.toml` (root + 15 member crates, `version = "1.0.0-rc.1"` pins) | config/manifest | batch (global find/replace) | n/a — mechanical string replace, no code pattern needed | n/a |
| `docs/GETTING-STARTED.md:53` (example banner output) | doc | n/a | same string as `cli/mod.rs` banner | exact |
| `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `11-VERIFICATION.md` (REL-04 hygiene) | doc (prose/process record) | n/a | **no code analog — pure prose edits, see "No Analog Found"** | n/a |

## Pattern Assignments

### `crates/famp/tests/gateway_setup_doc_accuracy.rs` (test, doc-accuracy) — EXTEND

**Analog:** itself (lines 104-227), the established "semantic checks" block added for prior findings (#1-#8).

**House style, extracted verbatim:**

1. **Read + whitespace-normalize the doc once** (already present, do not duplicate — reuse the existing `doc`/`normalized` bindings at lines 85-112):
```rust
let doc_path = gateway_setup_doc_path();
let doc = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| {
    panic!(
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: could not read {}: {e}",
        doc_path.display()
    )
});
let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");
```

2. **One `assert!` per semantic claim, positive presence check, always via `normalized.contains(...)` for anchor phrases that could line-wrap** (pattern at lines 187-198, 204-208):
```rust
assert!(
    normalized.contains("<exact anchor phrase for REL-01>"),
    "update the guide or the code: <one-line reason tied to the requirement, \
     e.g. 'guide must state what famp send's exit code confirms (REL-01)'>"
);
```

3. **Fail-message convention:** every assertion's message starts with `"update the guide or the code:"` (CI-accuracy findings) — the ONE exception is the flag-grep block at the top (`"docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide or the flag:"`), used only for CLI-surface (`--help`) checks, not prose-semantic checks. REL-01's new assertion is a prose-semantic check → use `"update the guide or the code:"`.

4. **Ordering-sensitive claims use `.find(...).expect(...)` + index comparison**, not just `.contains()` (lines 209-222) — reuse this pattern only if REL-01's assertion needs to prove the confirmation-semantics paragraph appears in the correct place relative to another anchor (e.g., after the `famp send --to agent:...` code block, before the `famp inspect tasks` instruction) — otherwise a plain `.contains()` is sufficient and preferred (simpler, matches most of the file's blocks).

5. **New assertion block goes at the end of the existing `#[test] fn gateway_setup_doc_accuracy()`** (after line 227, before the closing `}`), with a one-line comment header naming the requirement it pins, matching the section-comment convention at lines 104-111 and 200-203 (e.g. `// REL-01: send-confirmation semantics ...`).

**Sibling doc-accuracy test consistency check:** searched `crates/famp/tests/` for other doc-accuracy / flag-grep files; `cli_help_invariant.rs` is referenced in this file's own header comment (line 6) as the pattern this file mirrors for `--help`-based CLI-surface checks. No second `*_doc_accuracy.rs` file exists — `gateway_setup_doc_accuracy.rs` is the sole doc-pinning test and is authoritative for house style.

---

### `docs/GATEWAY-SETUP.md` §6 "Connect / verify" (doc) — INSERT

**Analog:** itself — surrounding §5/§6 structure (already read in full above, lines 230-300).

**Structural conventions to match:**
- Heading level: `## 6. Connect / verify` (H2, numbered) — no new heading needed for the inserted paragraph; it's prose within the existing §6, inserted after the code block ending line 256 (`famp send --to agent:hostB.example/bob --new-task --body "hello from A"` fenced block) and before `Then confirm the task's FSM reached a terminal state on **both** sides:` (current line 258).
- Callout/emphasis style: bold lead-in phrases used elsewhere in this doc for load-bearing warnings, e.g. `**Before you send, probe the live peer first.**` (line 271) and `**Known limitation (leaf-name ambiguity, deferred to v1.1).**` (line 285) — the new paragraph should follow this same bold-lead-in convention, e.g. `**What the exit code confirms.**` or similar, so it visually matches the doc's existing warning/callout density.
- Code-fence convention: bash fenced blocks (```` ```bash ````) for every command example — no new fence needed unless the paragraph re-quotes the `famp send` command.
- Cross-reference style: doc links to other doc sections via `[text](path#anchor)` (see footer, lines 297-300) — if the new paragraph needs to point elsewhere, follow that shape.

---

### `crates/famp/src/cli/send/mod.rs` (`SendArgs` field doc comment) — INSERT

**Analog:** itself — the `domain` field's doc comment (lines 102-109) and `terminal` field's doc comment (lines 81-87), both of which mix a one-line clap-help summary with a longer explanatory paragraph.

**Convention (clap doc-comment = `--help` text, verbatim):**
```rust
/// <One terse sentence — this becomes the --help text line, keep it short.>
/// <Optional continuation lines add detail visible only in --help's long
/// form / rustdoc, not truncated by clap.>
#[arg(long, ...)]
pub field_name: Type,
```
Existing precedent for exactly this kind of "behavioral caveat" comment: the `domain` field's comment states precedence order and when the flag is/isn't consulted (lines 102-107) — same shape needed for a fire-and-forget caveat on `to` (or a new doc line above the struct, per RESEARCH.md's note that struct-level vs field-level is a plan-time choice). Keep it to ONE sentence per RESEARCH.md's explicit guidance ("clap help text should stay terse").

---

### `README.md` (remote-path correction / "Not Shipped Yet" fix) — EDIT

**Analog:** itself — the `## Not Shipped Yet` section (lines 75-86) and `## What Works Today` bullets (lines 34-73).

**Structural conventions:**
- `## Not Shipped Yet` is a flat bullet list grouped under a bold sub-label (`**v1.0 — Federation Profile** (after v0.11):`, line 77) — the stale bullet `- `famp-gateway` bridging the local bus to remote FAMP-over-HTTPS`` (line 78) is the exact line RESEARCH.md flags as inaccurate (federation IS shipped as of Phase 7-11). Per RESEARCH.md's recommendation (option b, `[ASSUMED]`), the cheapest fix is: remove/correct that bullet and add a one-line pointer to `docs/GATEWAY-SETUP.md` rather than a new duplicated subsection.
- `## What Works Today` uses the same bold-lead-in + bullet convention for shipped-feature call-outs (e.g. `**Local-first bus (v0.9, shipped):**`, line 61; `**Broker daemon & cross-tool bootstrap (v0.11, shipped):**`, line 66) — if a new "federation gateway, v1.0, shipped" bullet is added here instead, follow this exact `**Feature (vX.Y, shipped):** one-paragraph description. See [doc](path).` shape.
- Cross-reference convention: `[design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md)` (line 65) — same relative-markdown-link shape for pointing at `docs/GATEWAY-SETUP.md`.

---

### `crates/famp/src/cli/mod.rs` (`BANNER_ABOUT` const + `version_strings_unified` test) — EDIT TOGETHER, SAME COMMIT

**Analog:** itself — both sides already co-located in one file; extract verbatim so the planner can specify both literal edits in a single plan action.

**Current const** (line 40):
```rust
const BANNER_ABOUT: &str = "FAMP 1.0.0-rc.1 (spec v0.5.2)";
```
→ must become `"FAMP 1.0.0 (spec v0.5.2)"`.

**Current test** (lines 228-253, `mod tests` using `super::BANNER_ABOUT`):
```rust
#[test]
fn version_strings_unified() {
    // clap reads CARGO_PKG_VERSION at compile time — pin to 1.0.0-rc.1.
    assert_eq!(
        env!("CARGO_PKG_VERSION"),
        "1.0.0-rc.1",
        "workspace version must be 1.0.0-rc.1"
    );
    assert!(
        BANNER_ABOUT.contains("1.0.0-rc.1"),
        "banner must contain 1.0.0-rc.1; got: {BANNER_ABOUT}"
    );
    assert!(
        BANNER_ABOUT.contains("spec v0.5.2"),
        "banner must contain spec v0.5.2; got: {BANNER_ABOUT}"
    );
    assert!(
        !BANNER_ABOUT.contains("v0.5.1"),
        "banner must NOT contain stale v0.5.1; got: {BANNER_ABOUT}"
    );
}
```
All three `1.0.0-rc.1` literals in this test (the `assert_eq!` expected value at line 238, and the `.contains("1.0.0-rc.1")` at line 242) must change to `1.0.0` in the SAME commit as the `Cargo.toml` bump — this is Pitfall 2 from RESEARCH.md. The `!BANNER_ABOUT.contains("v0.5.1")` negative-assertion pattern (line 250) is a good precedent to mirror if the planner wants an analogous `!BANNER_ABOUT.contains("1.0.0-rc.1")` post-bump guard, though RESEARCH.md doesn't require this — the existing test's literals simply need updating, no new assertion required.

---

## Shared Patterns

### Doc-accuracy pinning convention
**Source:** `crates/famp/tests/gateway_setup_doc_accuracy.rs:104-227`
**Apply to:** REL-01's new assertion block.
Every prose-semantic assertion: read+normalize once, `assert!(normalized.contains("<anchor phrase>"), "update the guide or the code: <reason>")`. Never invent a new fail-message prefix.

### Bold-lead-in callout convention
**Source:** `docs/GATEWAY-SETUP.md:271,285`
**Apply to:** the REL-01 §6 insertion and any README addition.
`**<Short imperative or noun phrase>.** <explanatory sentence(s)>` — matches the doc's existing density of warnings/callouts.

### Version-string co-location (three-way literal sync)
**Source:** `crates/famp/src/cli/mod.rs:40,238,242`
**Apply to:** REL-05's version-bump commit.
Any occurrence of `1.0.0-rc.1` outside `Cargo.toml`/`Cargo.lock` is duplicated across `BANNER_ABOUT`, `version_strings_unified`'s two literal assertions, `README.md:12`, and `docs/GETTING-STARTED.md:53` — all must move atomically in one commit or CI's `test` job self-inflicts a red run (Pitfall 2).

### Commit-bundling for CI `paths-ignore` (process pattern, not code)
**Source:** RESEARCH.md "Sequencing Constraint"
**Apply to:** every REL-item's commit.
Bundle a doc/test edit that touches a non-ignored path (`.rs`) in the SAME commit as any accompanying `.md` edit, so the commit triggers a real CI run. This mirrors "every Phase-11 `DOC-05`/`TEST-03` commit" per RESEARCH.md §"Sequencing" — i.e., this is itself a house pattern, not new guidance.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `.planning/REQUIREMENTS.md` (checkbox/traceability fixes, REL-04a) | doc (process record) | n/a | Pure prose/checkbox edit — no code pattern applies. Follow existing table row/checkbox syntax already present in the file (`- [x]` / `- [ ]`, `\| ID \| Phase \| Status \|` table rows). |
| `.planning/ROADMAP.md` (Phase 11 entry fix, REL-04c) | doc (process record) | n/a | Pure prose edit — add `11-08` to the Wave list, fix plan count, check the box. No code analog needed. |
| `.planning/phases/11-.../11-VERIFICATION.md` (ADDR-04 addendum, REL-04b) | doc (process record) | n/a | One-line addendum pointing at `REQUIREMENTS.md:49-56` — pure prose, no code pattern. |
| `Cargo.toml` × 16 files (version pin replace, REL-05) | config/manifest | batch | Mechanical global find/replace (`1.0.0-rc.1` → `1.0.0`); no code pattern to copy, just apply consistently per RESEARCH.md's exhaustive occurrence table. |
| `docs/GETTING-STARTED.md:53` (example output string) | doc | n/a | Single literal string swap, no structural pattern needed. |

## Metadata

**Analog search scope:** `crates/famp/tests/`, `crates/famp/src/cli/`, `docs/`, `README.md`, `.planning/` (targeted reads only, per RESEARCH.md's exact file:line citations — no broad Glob/Grep sweep needed since RESEARCH.md already pinpointed every location).
**Files scanned:** 8 (all directly named in RESEARCH.md's REL-01/REL-03/REL-05 sections)
**Pattern extraction date:** 2026-07-29
