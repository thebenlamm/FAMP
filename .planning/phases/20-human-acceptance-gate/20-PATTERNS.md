# Phase 20: Human Acceptance Gate - Pattern Map

**Mapped:** 2026-08-05
**Files analyzed:** 8 new files
**Analogs found:** 8 / 8

## File Classification

| New File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `docs/FOLLOWER-SETUP.md` | documentation / runbook | request-response + event-driven | `docs/GATEWAY-SETUP.md` | role-match |
| `scripts/phase20-clean-box-preflight.sh` | utility / validation gate | environment inspection + file-I/O | `scripts/check-doc-release-urls.sh` | role-match |
| `crates/famp/tests/phase20_clean_box_preflight.rs` | test | process execution + file-I/O | `crates/famp/tests/installer_checksum_gate.rs` | exact |
| `crates/famp/tests/follower_setup_doc_accuracy.rs` | test | file-I/O + request-response CLI probes | `crates/famp/tests/gateway_setup_doc_accuracy.rs` | exact |
| `20-REHEARSAL-TEMPLATE.md` | evidence schema | human checkpoint / batch capture | `18-VERIFICATION.md` | partial |
| `20-REHEARSAL.md` | evidence record | human checkpoint / batch capture | `18-VERIFICATION.md` | role-match |
| `20-ACCEPTANCE-TEMPLATE.md` | evidence schema | human checkpoint / batch capture | `18-VERIFICATION.md` | partial |
| `20-ACCEPTANCE.md` | evidence record | human checkpoint / batch capture | `18-VERIFICATION.md` | role-match |

## Pattern Assignments

### `docs/FOLLOWER-SETUP.md` (documentation/runbook, request-response + event-driven)

**Analog:** `docs/GATEWAY-SETUP.md`

**Linear role-and-command pattern** (lines 20-28):

```markdown
## 1. Prerequisites

On **each** host:

- `famp` and `famp-gateway` installed, and the persistent broker running:
  ```bash
  curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
  curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-gateway-installer.sh | sh
  famp daemon install
  ```
```

Copy the numbered, copy-pasteable runbook shape, but replace the legacy peer export/import section with the shipped `famp pair` inviter/redeemer flow. Name “Ben/inviter” and “follower/redeemer” at every asymmetric step.

**Observable readiness pattern** (lines 224-230):

```markdown
Each gateway loads its keyring **first**, then connects to its local
broker and picks up its principals, and only after the keyring has loaded
prints `famp-gateway: ready, backing N principal(s): ...`.
```

Keep done-signals adjacent to the command that produces them. Pairing must finish, inviter status/pin must be confirmed, gateways must restart, and only a post-restart ready signal permits sending.

**Delivery-proof boundary** (lines 271-284):

```markdown
That is the fire-and-forget boundary. The `famp inspect tasks` check below is
what actually confirms end-to-end delivery.

famp inspect tasks --id <task_id> --json

Look for the task to advance past `REQUESTED` to a terminal state
(`COMPLETED`, `FAILED`, or `CANCELLED`)
```

Strengthen this analog for Phase 20: use two distinct task IDs and require the receiving machine to capture its own terminal JSON. Do not copy the analog's “both sides inspect the same task” wording.

---

### `scripts/phase20-clean-box-preflight.sh` (utility/validation gate, environment inspection + file-I/O)

**Analog:** `scripts/check-doc-release-urls.sh`

**Shell and fail-closed conventions** (lines 23-26, 103-111):

```bash
set -euo pipefail

DOCS=(README.md)
while IFS= read -r f; do DOCS+=("$f"); done < <(find docs -maxdepth 1 -name '*.md' | sort)

if [ "$checked" -eq 0 ]; then
    echo "ERROR: no release URLs were checked -- the extraction regex matched nothing, which almost certainly means this gate is silently vacuous." >&2
    exit 1
fi

echo "OK - every documented release URL resolves."
```

Use the same strict-mode, actionable stderr, explicit non-zero exit, and final `OK` signal. The new script must be read-only and execute before installation. Check `rustc`, `cargo`, `famp`, and `famp-gateway` on `PATH`; `FAMP_HOME`; default state; and existing broker/socket/service state. Print OS, architecture, UTC timestamp, and only redacted paths.

**Alternative POSIX baseline:** `scripts/release-artifact-source-gate.sh:30-53` uses `set -eu`, quoted variables, a cumulative `FAIL` flag, and one diagnostic per violation. Prefer this accumulation pattern so a single run reports every contamination reason.

---

### `crates/famp/tests/phase20_clean_box_preflight.rs` (test, process execution + file-I/O)

**Analog:** `crates/famp/tests/installer_checksum_gate.rs`

**Isolated environment pattern** (lines 399-408, 477-480):

```rust
fn run_installer(installer_path: &Path, cargo_home: &Path, home: &Path, base_url: &str) -> Output {
    Command::new("sh")
        .arg(installer_path)
        .env("CARGO_HOME", cargo_home)
        .env("HOME", home)
        .env("FAMP_DOWNLOAD_URL", base_url)
        .env("FAMP_NO_MODIFY_PATH", "1")
        .output()
        .expect("failed to spawn famp-installer.sh under sh")
}

fn write_installer(dir: &Path, name: &str, src: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, src).expect("write installer script to tempdir");
    path
}
```

Run the committed preflight through `sh` under controlled `HOME`, `PATH`, and `FAMP_HOME`, using `TempDir` for every filesystem case. Never touch the developer's real FAMP state.

**Control/falsification pair** (lines 493-516, 587-629):

```rust
#[test]
fn installer_accepts_a_matching_artifact() {
    // ... isolated control ...
    assert!(output.status.success(), "installer must exit 0 ...");
}

#[test]
fn installer_rejects_a_corrupted_artifact_without_installing() {
    // ... same harness, one contaminated input ...
    assert!(!rejecting_output.status.success(), "installer must exit non-zero ...");
    assert!(rejecting_stderr.contains("checksum mismatch")
        || rejecting_stdout.contains("checksum mismatch"));
}
```

Mirror the repository's non-vacuous pattern: one pristine control plus a separate negative case for every contamination class. Assert both exit status and the specific actionable diagnostic; add a redaction assertion against absolute home paths.

---

### `crates/famp/tests/follower_setup_doc_accuracy.rs` (test, file-I/O + request-response CLI probes)

**Primary analog:** `crates/famp/tests/gateway_setup_doc_accuracy.rs`

**Imports and repository-path pattern** (lines 29-39):

```rust
use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn gateway_setup_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/GATEWAY-SETUP.md")
}
```

Point the helper at `../../docs/FOLLOWER-SETUP.md`; use `Command::cargo_bin("famp")` for live `pair`, `send`, and `inspect` help checks.

**Semantic relation and ordering pattern** (lines 117-125, 222-234, 277-295):

```rust
let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");
let keyring_load_idx = normalized.find("loads its keyring").expect("...");
let ready_idx = normalized.find("famp-gateway: ready").expect("...");
assert!(keyring_load_idx < ready_idx, "...");

let send_example_idx = normalized.find("famp send --to agent:...").expect("...");
let inspect_tasks_idx = normalized.find("famp inspect tasks --id").expect("...");
let confirms_only_idx = normalized.find("A zero exit code confirms only").expect("...");
assert!(confirms_only_idx > send_example_idx && confirms_only_idx < inspect_tasks_idx, "...");
```

Use stable Phase 20 anchors to assert installer-before-pairing, consent-immediately-before-redeem, follower-as-redeemer, inviter status/pin before restart, ready-before-send, and receiver-specific inspection after each direction. Include negative assertions forbidding legacy `peer export/import`, source-build instructions, shared VPN/key copying, sender-exit-as-proof, and Phase 21 dependency.

**Pairing-message synchronization analog:** `crates/famp/tests/pair_cli.rs:426-483` enumerates the actual `PairingError` display values, checks jargon, includes a known-positive falsification control, and pins `CONSENT_WARNING` to `docs/QUARANTINE.md`. Reuse those authored constants/messages rather than duplicating freehand strings in the guide gate.

---

### Phase 20 evidence templates and records (evidence schema/record, human checkpoint)

**Analog:** `.planning/phases/18-cross-person-trust-bootstrap-pairing/18-VERIFICATION.md`

**Machine-readable human-item pattern** (lines 26-33):

```yaml
human_verification:
  - test: "PAIR-05 comprehension half ..."
    expected: "A non-expert reads a failure message and knows what to do next ..."
    why_human: "No automated test can measure human comprehension."
    deferred_to: "Phase 20 UAT-02"
    status: open
```

**Human checkpoint prose pattern** (lines 140-151):

```markdown
### Human Verification Required

#### 1. PAIR-05 comprehension half

**Test:** Give the seven pairing failure messages ...
**Expected:** A non-expert can act on each message unaided.
**Why human:** No automated test can measure human comprehension.
```

Use frontmatter for final classification and completeness fields, then human-readable evidence tables. Every evidence row should name criterion, owner, capture command/attestation, UTC timestamp, redacted evidence, and result. Templates contain unmistakable placeholders; populated records contain no default “pass.” Keep rehearsal and acceptance separate, and expose exactly one final outcome: `pass`, `product_or_guide_failure`, or `invalid`.

For `20-REHEARSAL.md`, additionally capture clean preflight, supported OS/architecture, downloaded binary versions, pairing/restart readiness, both task directions, and receiver-owned terminal JSON. For `20-ACCEPTANCE.md`, additionally capture the participant/network/no-VPN attestation, question/no-coaching log, seven-message comprehension responses, guide commit/digest, and the same two receiver-owned task proofs. Never store invite codes, keys, tokens, raw transcripts, or unredacted home paths.

## Shared Patterns

### Fail Closed and Prove the Gate Is Discriminating

**Sources:** `crates/famp/tests/installer_checksum_gate.rs:493-516,587-629`; `scripts/check-doc-release-urls.sh:103-111`

Every automated gate needs a clean control and a deliberately contaminated case that fails for the named reason. Zero matches, missing evidence fields, or an unrecognized final status are errors, never passes.

### Assert Relationships, Not Vocabulary

**Sources:** `crates/famp/tests/gateway_setup_doc_accuracy.rs:117-125,196-234,274-295`; `crates/famp/tests/pair_cli.rs:177-190`

Normalize Markdown and compare anchor offsets. Direction, ownership, and order are first-class assertions. Presence-only greps are insufficient because this repository has already shipped semantically inverted wiring while all relevant words were present.

### Production Surfaces Only

**Sources:** `docs/GATEWAY-SETUP.md:20-28,224-230,257-284`; `crates/famp/tests/pair_cli.rs:625-637`

Exercise published installers and shipping CLI commands. Tests may verify guide syntax and schemas, but they cannot substitute loopback execution for the clean supported host or a genuine second person.

### Receiver Owns End-to-End Evidence

**Source:** `docs/GATEWAY-SETUP.md:271-284`

`famp send` success is local acceptance only. Each receiving person runs `famp inspect tasks --id <task_id> --json` for the distinct task received on that machine and captures a terminal state.

### Explicit Human Boundary

**Source:** `.planning/phases/18-cross-person-trust-bootstrap-pairing/18-VERIFICATION.md:26-33,140-151`

Keep mechanized evidence separate from human comprehension and no-coaching claims. Human facts remain open until observed and recorded; automation must not synthesize them.

## No Analog Found

No proposed file is entirely without a repository analog. The evidence templates have only a partial analog: existing verification reports demonstrate structured human checkpoints, but Phase 20 must introduce the owner/capture/timestamp/redaction table and three-way outcome schema described in `20-RESEARCH.md`.

## Metadata

**Analog search scope:** `docs/`, `scripts/`, `crates/famp/tests/`, `.planning/phases/16-*`, `.planning/phases/18-*`
**Strong analogs inspected:** 5 (`GATEWAY-SETUP.md`, `gateway_setup_doc_accuracy.rs`, `installer_checksum_gate.rs`, `pair_cli.rs`, `18-VERIFICATION.md`), plus shell-style confirmation from `check-doc-release-urls.sh` and `release-artifact-source-gate.sh`
**Pattern extraction date:** 2026-08-05
