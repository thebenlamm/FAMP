# Phase 20: Human Acceptance Gate - Research

**Researched:** 2026-08-05
**Domain:** clean-box documentation validation, cross-network UAT, and auditable human evidence
**Confidence:** HIGH

## User Constraints

### Locked decisions

- **D-01:** Write one linear follower-facing guide that starts from an unprepared supported machine, leads with the prebuilt-binary installer, and uses `famp pair` rather than the legacy `famp peer export/import` flow. Background and operator caveats may be linked, but the happy path must not require the follower to synthesize steps across multiple documents.
- **D-02:** The guide must keep roles explicit throughout (Ben/inviter and follower/redeemer), use copy-pasteable commands, explain each observable done-signal, place the consent warning at the actual pairing decision, and route failures through the shipped actionable messages rather than teaching cryptographic concepts.
- **D-03:** Before involving the second person, execute the exact follower path on a disposable clean environment that has neither prior FAMP state nor a Rust toolchain. The rehearsal must exercise downloaded release binaries, pairing, gateway readiness, bidirectional signed delivery, and receiver-side terminal task inspection. A preflight assertion must fail if FAMP state or Rust tooling is present.
- **D-04:** Semantic guide gates must assert role direction, pairing flow, install source, readiness signals, sender-exit non-proof, and receiver-owned terminal-state proof. Flag/string presence alone is insufficient because it would repeat the v1.0 inverted-wiring failure.
- **D-05:** Ben prepares only his own machine and sends the single pairing artifact plus the follower guide. The follower operates their own machine. Ben may observe and collect explicitly shared evidence but may not type commands, screen-control, edit the follower's state, hand-copy keys, or provide step-by-step recovery that is absent from the guide.
- **D-06:** Clarifying what an ordinary word means is allowed only if it does not reveal the next action. Any question about what command to run, what value to enter, or how to recover is recorded as a guide/comprehension failure; update the guide, reset the affected state, and rerun the acceptance event rather than coaching through it.
- **D-07:** The network topology is a hard precondition: two independently administered machines on different networks, no shared VPN/overlay, and no direct key-file or public-key-line exchange. The inviter URL must be publicly reachable by the follower before the invite's 24-hour clock begins.
- **D-08:** Use two distinct tasks, one initiated from each machine. For each direction, the receiving person captures their own `famp inspect tasks --id <task_id> --json` output showing a terminal state (`COMPLETED`, `FAILED`, or `CANCELLED`). Sender exit status, gateway logs, mailbox arrival alone, or a report relayed by Ben cannot substitute.
- **D-09:** Preserve a redacted acceptance record containing machine/OS and binary versions, proof of no Rust/prior FAMP state for the rehearsal, network-independence attestation, timestamps, pairing done-signals, both task IDs/directions, and both receivers' terminal-state outputs. Never capture short codes, private keys, auth tokens, or unredacted home paths.
- **D-10:** Separate three outcomes: pass, product/guide failure, and invalid run. Product/guide failures include confusing instructions or actionable-message comprehension failure; invalid runs include coaching, stale FAMP state, Rust/source-build fallback, same-network/VPN use, copied keys, or missing receiver-owned evidence. Only a fully clean rerun may close an invalid run.
- **D-11:** The seven Phase 18 pairing failure messages receive comprehension evidence during the clean rehearsal and/or real event by naturally encountered failures plus a scripted, non-mutating review with the second person. The person must state the next action in their own words without explanation; this closes PAIR-05's human half without deliberately damaging the live pairing attempt.

### the agent's Discretion

- Exact guide filename and section layout, clean-environment technology, redaction format, evidence template layout, and test implementation are left to planning, provided they satisfy the locked evidence and no-coaching rules above.

### Deferred Ideas

- Push-based agent notification remains Phase 21; Phase 20 must succeed through the current explicit inspection/processing path.
- A signed peer directory and automated NAT traversal remain outside v1.1 scope.

## Summary

Phase 20 is primarily a documentation-and-evidence phase, not a new transport feature. The repository already ships the prebuilt installers, `famp pair`, gateway readiness output, remote send path, explicit Inbox behavior, and JSON task inspection needed for the event. The plan should compose those existing surfaces into one frozen follower guide and prove the guide at three different levels: semantic automated checks, a disposable clean-box rehearsal, then a blocking real-person event. [VERIFIED: repository inspection of `README.md`, `docs/PAIRING.md`, `docs/GATEWAY-SETUP.md`, Phase 16/18/19 verification artifacts, and CLI source]

The decisive architecture is an evidence ledger with provenance per observation. Machine-produced output must be pasted by the machine/person that owns the observation; attestations cover facts no command can prove (independent administration, different networks, no VPN, no coaching); and a run classifier prevents partial or invalid evidence from being called a pass. Automation can prepare and validate the ledger's shape, but it must never populate or approve UAT-02. [VERIFIED: Phase 20 CONTEXT.md D-05 through D-11]

**Primary recommendation:** implement three serial plans: (1) follower guide plus semantic gates and evidence templates, (2) a genuinely clean disposable-machine rehearsal that freezes the guide, and (3) a blocking human checkpoint that records—but cannot fabricate—the real second-person outcome.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Linear follower setup | Documentation / operator interface | Existing CLI | The guide sequences shipped commands and observable done-signals; protocol behavior remains owned by existing binaries. [VERIFIED: CONTEXT.md D-01/D-02] |
| Semantic guide validation | Rust integration tests | CLI help/source | Existing compiled doc gates launch the shipping CLI and assert ordering/direction, which is stronger than text-presence checks. [VERIFIED: `crates/famp/tests/gateway_setup_doc_accuracy.rs`] |
| Clean-box rehearsal | External disposable host | Checked-in runbook/evidence ledger | Absence of Rust and prior FAMP state is a host property and must be asserted before installation. [VERIFIED: DOC-07 and CONTEXT.md D-03] |
| Pairing and signed exchange | Existing CLI/gateway runtime | Human operators | `famp pair`, `famp-gateway`, and `famp send` already own the behavior; Phase 20 exercises them without redesign. [VERIFIED: Phase 18 verification and ROADMAP Phase 20 boundary] |
| Terminal-state proof | Receiving machine's broker inspector | Evidence ledger | `famp inspect tasks --id ... --json` exposes task state, while sender success proves only local acceptance. [VERIFIED: `docs/GATEWAY-SETUP.md` and `crates/famp/src/cli/inspect/tasks.rs`] |
| Human comprehension | Second person | Facilitator's question log | PAIR-05 explicitly leaves comprehension to Phase 20; software tests cannot replace the person's own explanation. [VERIFIED: Phase 18 verification] |
| Outcome classification | Checked-in acceptance procedure | Human checkpoint | The locked contract distinguishes pass, product/guide failure, and invalid run. [VERIFIED: CONTEXT.md D-10] |

## Standard Stack

### Core

| Component | Version / Location | Purpose | Why Standard |
|-----------|--------------------|---------|--------------|
| Shipping FAMP binaries | workspace `1.1.0-rc.1`; GitHub Release installers | Install, pair, run the gateway, exchange tasks, inspect terminal state | These are the exact product surfaces under acceptance; source builds are prohibited for DOC-07. [VERIFIED: root `Cargo.toml`, README installer section, CONTEXT.md D-03] |
| Rust integration-test harness | existing workspace tests | Compile semantic guide assertions against shipping CLI help and repository docs | Existing `gateway_setup_doc_accuracy.rs` already demonstrates direction/order/send-boundary assertions and process-level CLI checks. [VERIFIED: repository inspection] |
| Markdown evidence artifacts | Phase 20 directory and/or `docs/` | Frozen runbook, blank templates, rehearsal record, human acceptance record | Repository-native phase artifacts already carry structured frontmatter, criteria, evidence, and explicit human items. [VERIFIED: Phase 16 and Phase 18 planning/verification artifacts] |
| POSIX shell preflight | new small script under `scripts/` | Fail before installation when Rust tools or FAMP state/binaries already exist | Supported hosts already require a shell for the published installer; a preflight script makes the clean-box predicate reproducible. [VERIFIED: README prerequisites and installer path] |

### Supporting

| Component | Purpose | When to Use |
|-----------|---------|-------------|
| `serde_json` already in `famp` | Validate/render `inspect tasks --json` output in tests | Automated fixture/schema checks only; the real output still comes from each receiver. [VERIFIED: `crates/famp/Cargo.toml` and `inspect/tasks.rs`] |
| `assert_cmd` already in dev-dependencies | Compare documented commands with live CLI help | Guide semantic tests that need process-level command validation. [VERIFIED: `crates/famp/Cargo.toml` and existing doc tests] |
| `tempfile` already in dev-dependencies | Isolate documentation/evidence test fixtures | Tests must not touch the operator's real `~/.famp`. [VERIFIED: `crates/famp/Cargo.toml` and existing integration tests] |

No new external package is needed. [VERIFIED: repository-native requirements and existing dependencies]

## Package Legitimacy Audit

Not applicable: the recommended design installs no new Rust crate, npm package, or Python package. [VERIFIED: recommended stack above]

## Architecture Patterns

### System Architecture Diagram

```text
Frozen follower guide + one invite artifact
                 |
                 v
   Clean-box preflight (before install)
      | fail: not clean -> INVALID
      v
Release installer -> pair redeem -> inviter status/pin -> restart gateways
                                              |
                                              v
                         gateway ready signals on both hosts
                                              |
                  +---------------------------+---------------------------+
                  |                                                       |
          Ben sends Task A                                      follower sends Task B
                  |                                                       |
                  v                                                       v
 follower processes + inspects terminal                     Ben processes + inspects terminal
                  |                                                       |
                  +---------- receiver-owned JSON evidence ---------------+
                                              |
                              comprehension review + question log
                                              |
                         PASS / PRODUCT-GUIDE FAILURE / INVALID
```

This flow keeps the automated and human trust boundaries visible: the clean-box predicate is checked before mutation, and terminal proof is captured at each receiving host rather than inferred from the sender. [VERIFIED: CONTEXT.md D-03, D-08, D-10]

### Recommended Project Structure

```text
docs/
└── FOLLOWER-SETUP.md                    # one linear DOC-06 happy path
scripts/
└── phase20-clean-box-preflight.sh       # read-only, fail-closed pre-install assertion
crates/famp/tests/
└── follower_setup_doc_accuracy.rs       # semantic role/order/CLI/doc gate
.planning/phases/20-human-acceptance-gate/
├── 20-REHEARSAL-TEMPLATE.md             # blank evidence schema
├── 20-REHEARSAL.md                      # populated clean-box result
├── 20-ACCEPTANCE-TEMPLATE.md            # blank human-event schema
└── 20-ACCEPTANCE.md                     # populated only from the real event
```

Names are recommendations within the agent's discretion; keeping templates separate from populated records makes fabricated/default values conspicuous. [VERIFIED: CONTEXT.md discretion plus repository artifact patterns]

### Pattern 1: Freeze before scarce human execution

**What:** Finish the guide, its semantic tests, and a successful clean-box rehearsal; record the tested commit hash and guide digest; only then generate the live invite and start UAT. [VERIFIED: CONTEXT.md D-03, D-07, and Specific Ideas]

**Why:** The invite clock begins at creation and lasts 24 hours; creating it after reachability and install readiness avoids wasting the acceptance window. [VERIFIED: `docs/PAIRING.md` and pairing source]

### Pattern 2: Evidence cells have an owner and capture method

Use a table whose rows include `criterion`, `owner`, `capture command/attestation`, `timestamp`, `redacted evidence`, and `result`. Examples:

| Criterion | Owner | Capture |
|-----------|-------|---------|
| follower received Task A terminal state | follower | follower runs and shares `famp inspect tasks --id <A> --json` |
| Ben received Task B terminal state | Ben | Ben runs `famp inspect tasks --id <B> --json` |
| different networks/no VPN | both people | signed-off plain-language attestation, not IP-address collection |
| no coaching | facilitator/question log | record all questions and classify before recovery |

This prevents a Ben-relayed report or sender-side status from silently replacing receiver evidence. [VERIFIED: CONTEXT.md D-05, D-08, D-09]

### Pattern 3: Semantic documentation gates assert relations, not vocabulary

The test should normalize Markdown whitespace, locate ordered anchors, invoke relevant `--help` surfaces, and assert concrete direction pairs. At minimum it must prove: installer before pairing; consent immediately before redeem/code; follower is redeemer and Ben is inviter; no legacy peer export/import in the happy path; pin/restart before ready; A-to-B send is followed by B-owned inspection; B-to-A send is followed by A-owned inspection; sender exit is explicitly non-proof; terminal states are enumerated; and Phase 21 notification is not required. [VERIFIED: CONTEXT.md D-01/D-04 and `gateway_setup_doc_accuracy.rs` pattern]

### Pattern 4: Three-way outcome classifier

The record should expose exactly one final status:

- `pass`: every hard precondition and evidence row is satisfied.
- `product_or_guide_failure`: the product, guide, or shipped recovery language failed under an otherwise valid run.
- `invalid`: the protocol was contaminated by coaching, stale state, source-build fallback, disallowed topology/key exchange, or missing owner evidence.

The validator should reject `pass` if any required cell is blank or redaction scan fails; it must not convert failure/invalid to pass. [VERIFIED: CONTEXT.md D-10]

### Pattern 5: Non-mutating comprehension cards

Keep the seven current redeemer-facing messages synchronized with their actual `PairingError` display text in an automated test. During rehearsal/live review, show each message without causing the corresponding fault and ask, “What would you do next?” Record a short paraphrase and pass/fail, without teaching the answer first. [VERIFIED: `crates/famp/src/pairing/mod.rs` identifies seven messages and Phase 18 verification leaves human comprehension open]

### Anti-Patterns to Avoid

- **One giant human checkpoint:** It makes guide defects surface only after the scarce event begins. Complete semantic gates and rehearsal first. [VERIFIED: DOC-07 ordering]
- **Loopback/container accepted as UAT:** Existing loopback E2Es prove mechanics but cannot prove a different person/network or no coaching. [VERIFIED: Phase 18/ROADMAP acceptance contract]
- **Raw transcript dumping:** Terminal logs can expose home paths, codes, tokens, and unrelated content. Capture only named fields and redact before check-in. [VERIFIED: CONTEXT.md D-09]
- **Generating deliberate live pairing failures:** It can burn the five-attempt budget or invalidate the run. Use non-mutating comprehension cards. [VERIFIED: CONTEXT.md D-11 and `docs/PAIRING.md`]
- **Editing the guide mid-run and continuing:** That is a guide failure; fix, reset affected state, and begin a clean run against a newly frozen guide. [VERIFIED: CONTEXT.md D-06/D-10]
- **Treating `famp send` exit zero as remote success:** The CLI confirms only local broker acceptance. [VERIFIED: `docs/GATEWAY-SETUP.md`]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pairing | New exchange format, key copy, or fingerprint comparison | Existing `famp pair invite/redeem/status/revoke` | The shipped flow already enforces the accepted trust and consent boundaries. [VERIFIED: Phase 18 verification] |
| Installer/integrity | New downloader or checksum implementation | Published `*-installer.sh` assets | Phase 16 already verified checksum-fail-closed installers and public release URLs. [VERIFIED: Phase 16 verification] |
| Delivery proof | Log correlation or sender receipt heuristic | Receiver's `famp inspect tasks --id ... --json` | It directly reports the receiver-side FSM state required by UAT-02. [VERIFIED: CONTEXT.md D-08] |
| Reachability | NAT traversal, VPN, or new relay | Existing public inviter endpoint and gateway configuration | Automated NAT traversal and shared VPN are out of scope; the inviter must already be reachable. [VERIFIED: CONTEXT.md D-07 and deferred ideas] |
| Human comprehension scoring | NLP or automated proxy | Person paraphrases next action; facilitator records result | PAIR-05 explicitly requires genuine human comprehension. [VERIFIED: Phase 18 verification] |

**Key insight:** this phase is credible only if it reuses production surfaces and makes non-automatable facts explicit; adding a parallel “test” path would weaken the acceptance claim. [VERIFIED: ROADMAP Phase 20 goal]

## Common Pitfalls

### Pitfall 1: The preflight runs after installation

**What goes wrong:** installation itself creates FAMP/Cargo paths, so the clean-box evidence becomes ambiguous. **How to avoid:** run and capture a read-only preflight before any installer, pairing, daemon, or gateway command. It should fail if `rustc`, `cargo`, `famp`, or `famp-gateway` resolves; if `$FAMP_HOME` is set or the resolved/default FAMP home exists; or if an existing broker/socket/service is detected. [VERIFIED: CONTEXT.md D-03; exact service checks are repository-informed recommendation]

### Pitfall 2: A disposable container is called a fresh-machine proof

**What goes wrong:** a container misses supported-host firewall, shell-profile, service-manager, and macOS Gatekeeper behavior. **How to avoid:** use a genuinely disposable supported VM or independently administered physical host for DOC-07; containers remain useful only for automated script tests. [VERIFIED: Phase 16 research and verification]

### Pitfall 3: Pairing succeeds but the running gateway retains its old keyring

**What goes wrong:** pins are durable but the process loads its keyring once. **How to avoid:** put the documented restart after redeemer success and inviter `famp pair status`, then require a fresh ready signal before sends. [VERIFIED: `docs/PAIRING.md` and `docs/GATEWAY-SETUP.md`]

### Pitfall 4: Remote traffic waits forever because Phase 19 blocks auto-wake

**What goes wrong:** the test assumes `famp await` will wake on remote-origin traffic. **How to avoid:** the guide must direct explicit Inbox/task processing and inspection; do not depend on Phase 21. [VERIFIED: Phase 19 verification and CONTEXT.md deferred ideas]

### Pitfall 5: “Terminal on both sides” is interpreted as inspecting the same task from both hosts

**What goes wrong:** D-08 requires two distinct tasks and receiver-owned proof for each direction. **How to avoid:** label Task A (`Ben -> follower`, inspected by follower) and Task B (`follower -> Ben`, inspected by Ben) as separate evidence rows with separate IDs. [VERIFIED: CONTEXT.md D-08]

### Pitfall 6: Secrets enter git through copy-pasted output

**What goes wrong:** pairing artifacts and full terminal transcripts may contain short codes, paths, or tokens. **How to avoid:** never paste the invite artifact; whitelist evidence fields; replace home prefixes with `<REDACTED_HOME>` before staging; run a fail-closed scan for five-word-code headings, key/token labels, private-key material, and absolute home paths. Human review remains required because pattern scans cannot prove absence. [VERIFIED: CONTEXT.md D-09; scan details are repository-informed recommendation]

### Pitfall 7: The comprehension review becomes coaching

**What goes wrong:** explaining the message before the follower paraphrases it invalidates PAIR-05 evidence. **How to avoid:** show one message, ask the same neutral question, record first response, and reveal no correction until the run is classified. [VERIFIED: CONTEXT.md D-06/D-11]

## Code Examples

### Semantic ordering test

```rust
// Source: repository pattern in crates/famp/tests/gateway_setup_doc_accuracy.rs
let doc = std::fs::read_to_string(path).expect("read follower guide");
let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");
let install = normalized.find("famp-installer.sh").expect("binary install");
let redeem = normalized.find("famp pair redeem").expect("pair redeem");
let send = normalized.find("Ben sends Task A").expect("first direction");
let receiver_proof = normalized
    .find("Follower runs `famp inspect tasks --id")
    .expect("receiver-owned proof");
assert!(install < redeem && redeem < send && send < receiver_proof);
```

The actual assertions should use stable semantic anchors authored in the new guide, not headings alone. [VERIFIED: existing doc-gate pattern]

### Fail-closed clean-box preflight shape

```sh
# Source: repository-informed recommendation; run before any mutation
for tool in rustc cargo famp famp-gateway; do
  if command -v "$tool" >/dev/null 2>&1; then
    printf 'INVALID: %s already exists on PATH\n' "$tool" >&2
    exit 1
  fi
done

test -z "${FAMP_HOME:-}" || {
  printf 'INVALID: FAMP_HOME is already set\n' >&2
  exit 1
}
test ! -e "$HOME/.famp" || {
  printf 'INVALID: prior FAMP state exists\n' >&2
  exit 1
}
```

The production script must also report OS/architecture and timestamp without emitting an unredacted home path. [VERIFIED: CONTEXT.md D-09; script shape is recommended]

### Machine-owned task evidence

```bash
# Source: shipping CLI documented in docs/GATEWAY-SETUP.md
famp inspect tasks --id <task_id> --json
```

The ledger should retain only the JSON for the task received on that machine and explicitly assert its state is one of `COMPLETED`, `FAILED`, or `CANCELLED`. [VERIFIED: CONTEXT.md D-08]

## State of the Art

| Old approach in this repository | Current Phase 20 approach | Impact |
|---------------------------------|---------------------------|--------|
| `famp peer export/import` TOFU bootstrap | `famp pair` short-code bootstrap | The follower never copies key blobs or compares fingerprints. [VERIFIED: Phase 18 verification] |
| Flag/string doc greps | Ordered, directional semantic assertions plus live CLI help | Prevents syntactically present but inverted instructions from passing. [VERIFIED: `gateway_setup_doc_accuracy.rs` history/comments] |
| Sender exit as implied delivery | Receiver-owned terminal FSM JSON | Aligns evidence with the actual end-to-end boundary. [VERIFIED: `docs/GATEWAY-SETUP.md`] |
| Remote traffic waking `famp await` | Remote traffic remains durable for explicit Inbox processing | Phase 20 must include explicit handling and cannot depend on push notification. [VERIFIED: Phase 19 verification] |

## Assumptions Log

No material product or compliance claim is assumed. Recommended filenames, the exact preflight implementation, and evidence-table schema are design recommendations within the explicitly delegated discretion; the planner may refine them without reopening locked decisions.

## Resolved Execution Parameters

All research questions are resolved at planning time without inventing external facts:

1. **Rehearsal OS/host:** the exact supported OS and architecture are execution-time inputs owned by Plan 20-02's blocking checkpoint. The checkpoint accepts only macOS arm64/x86_64 or Linux x86_64 within the published release boundary, and requires the real host's pre-install clean signal before installation or invitation. Repository automation validates the predicate but cannot select or attest the external host.

2. **Participant and schedule:** identity, availability within the invitation window, independently administered machine/network, and no-VPN/no-key-copy facts are execution-time inputs owned by Plan 20-03's blocking checkpoint. The checkpoint must write those owner-attributed attestations to `20-ACCEPTANCE.md` before invite creation; absent inputs block execution and cannot be inferred by an auto task.

3. **Explicit received-task processing:** the concrete shipping CLI path is `famp inbox list --as <receiver>` to read the Gateway-origin new task and its task ID, then `famp send --as <receiver> --to <sender-principal> --task <task-id> --body <result> --terminal` to send the final reply and close the task, then `famp inspect tasks --id <task-id> --json` on the receiving owner machine for terminal proof. The host-agent equivalent is `famp_inbox`, followed by `famp_send` with `mode: "reply"`, the inbox task ID, and default `expect_reply: false`, then the same inspector. This is grounded in `crates/famp/src/cli/inbox/mod.rs`, `crates/famp/src/cli/send/mod.rs`, and README's MCP reply contract; Phase 21 notification is not required.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Published FAMP release installers | DOC-07/UAT-02 | Yes | `1.1.0-rc.1` workspace/release line | No source-build fallback permitted for a valid run. [VERIFIED: Phase 16 verification] |
| Publicly reachable inviter gateway | pairing/UAT-02 | Must be confirmed immediately before invite | runtime external | No NAT/VPN workaround inside this phase. [VERIFIED: CONTEXT.md D-07] |
| Disposable supported clean host | DOC-07 | External, not provable in this research session | — | None; absence invalidates rehearsal. [VERIFIED: DOC-07] |
| Genuine second person + independent host/network | UAT-02/PAIR-05 | External, not provable in this research session | — | None; automation cannot substitute. [VERIFIED: UAT-02 and Phase 18 verification] |

**Missing dependencies with no fallback:** the clean external host and real participant are execution-time resources. Their absence blocks completion, not planning.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust integration tests (`cargo test`) plus POSIX shell checks; blocking human verification for UAT-02 |
| Config file | root `Cargo.toml`, `crates/famp/Cargo.toml`, and existing `justfile` recipes |
| Quick run command | `cargo test -p famp --test follower_setup_doc_accuracy` |
| Full suite command | `cargo test --workspace --no-fail-fast` followed by the clean-box rehearsal; UAT remains a separate checkpoint |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOC-06 | One correct linear follower guide using binary install + `famp pair`, roles/done-signals/receiver proof semantically correct | compiled doc-accuracy integration | `cargo test -p famp --test follower_setup_doc_accuracy` | No - Wave 0 |
| DOC-07 | Exact guide succeeds on clean supported machine with no Rust or FAMP state | external rehearsal + preflight evidence | `scripts/phase20-clean-box-preflight.sh` before installer, then runbook commands | No - Wave 0; cannot be fully automated locally |
| UAT-02 | Second person unassisted, different networks, two directions, receiver-owned terminal proof | blocking human acceptance | no automated substitute; validate completed evidence ledger shape/redaction afterward | No - Wave 0 |
| PAIR-05 human half | Person explains next action for each of seven current failure messages | scripted human comprehension review | automated test synchronizes displayed messages; human checkpoint records paraphrases | Partial infrastructure exists; human record absent |

### Sampling Rate

- **Per guide/test commit:** `cargo test -p famp --test follower_setup_doc_accuracy`.
- **Before rehearsal:** full relevant pairing, installer, gateway doc, and auto-wake tests; then preflight on the untouched disposable host.
- **Before human event:** freeze commit/digest after successful rehearsal; re-run semantic gate and public reachability check before invite creation.
- **Phase gate:** full workspace suite green plus populated clean rehearsal and real-person acceptance records; human evidence must be reviewed for ownership and redaction.

### Wave 0 Gaps

- [ ] `docs/FOLLOWER-SETUP.md` - DOC-06 linear guide.
- [ ] `crates/famp/tests/follower_setup_doc_accuracy.rs` - semantic direction/order/CLI gate.
- [ ] `scripts/phase20-clean-box-preflight.sh` and focused test - clean-state fail-closed assertion.
- [ ] `20-REHEARSAL-TEMPLATE.md` / `20-ACCEPTANCE-TEMPLATE.md` - evidence schema and three-way outcome classification.
- [ ] Message synchronization test/fixture for the seven pairing comprehension prompts.
- [ ] External clean rehearsal evidence - cannot exist before execution.
- [ ] Real-person acceptance evidence - cannot exist before the blocking event.

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Yes | Existing mutual key pinning and signed envelopes; no alternate trust path. [VERIFIED: Phase 18 verification] |
| V3 Session Management | No | No web user session is introduced by this documentation/UAT phase. [VERIFIED: phase boundary] |
| V4 Access Control | Yes | Consent occurs before code redemption; only explicitly paired peers are admitted. [VERIFIED: pairing consent/order tests] |
| V5 Input Validation | Yes | Existing CLI parsing, signed-wire validation, and strict evidence status/schema checks; human-provided evidence is treated as data. [VERIFIED: CLI source and recommended ledger validation] |
| V6 Cryptography | Yes | Reuse existing Ed25519/JCS and pairing implementation; never copy or hand-roll keys. [VERIFIED: Phase 18 verification] |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Evidence spoofed by the wrong machine/person | Spoofing / Repudiation | Owner field plus receiver-produced JSON and explicit attestations. [VERIFIED: D-08/D-09] |
| Secret leakage in checked-in evidence | Information Disclosure | Whitelisted evidence fields, redaction instructions, automated scan, final human review. [VERIFIED: D-09] |
| Coaching changes an “unassisted” result | Tampering / Repudiation | Question log and invalid-run classification; rerun after guide repair. [VERIFIED: D-05/D-06/D-10] |
| Reused/stale state creates false success | Spoofing | Pre-install clean-box assertion; invalidation rather than cleanup-and-continue. [VERIFIED: D-03/D-10] |
| Pairing fault simulation consumes attempts | Denial of Service | Non-mutating message review; no deliberate live failures. [VERIFIED: D-11 and five-attempt limit] |
| Remote content unexpectedly triggers agent work | Elevation of Privilege | Phase 19 Local-only auto-wake remains in force; explicit human processing only. [VERIFIED: Phase 19 verification] |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/20-human-acceptance-gate/20-CONTEXT.md` - locked phase and evidence decisions.
- `.planning/ROADMAP.md` and `.planning/REQUIREMENTS.md` - Phase 20 goal and DOC-06/DOC-07/UAT-02 contract.
- `.planning/phases/18-cross-person-trust-bootstrap-pairing/18-VERIFICATION.md` - pairing mechanics and open PAIR-05 human half.
- `.planning/phases/16-distribution/16-CONTEXT.md` and `16-VERIFICATION.md` - release/install boundaries and clean-machine limits.
- `.planning/phases/19-auto-wake-gate/19-VERIFICATION.md` - remote Inbox/no-auto-wake behavior.
- `README.md`, `docs/PAIRING.md`, `docs/GATEWAY-SETUP.md`, `docs/QUARANTINE.md` - shipping operator surfaces.
- `crates/famp/tests/gateway_setup_doc_accuracy.rs`, `crates/famp/tests/installer_checksum_gate.rs`, `crates/famp/tests/pair_cli.rs` - reusable validation patterns.
- `crates/famp/src/cli/inspect/tasks.rs`, `crates/famp/src/pairing/mod.rs` - JSON inspection and seven pairing messages.

### Secondary (MEDIUM confidence)

- None. No external web research was necessary; this phase is constrained by repository-native product behavior and a human event.

### Tertiary (LOW confidence)

- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all recommended components already exist in the repository or are new Markdown/shell/test artifacts using established patterns.
- Architecture: HIGH - directly derived from locked Phase 20 decisions and earlier verified phase boundaries.
- Pitfalls: HIGH - each material risk is documented in the phase context, shipping docs, or verification artifacts.

**Research date:** 2026-08-05
**Valid until:** 2026-09-04 (repository-specific behavior; re-check if CLI, release tag, or pairing/gateway docs change)
