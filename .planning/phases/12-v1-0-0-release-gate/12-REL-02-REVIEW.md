# REL-02: Post-Fix Adversarial Source Review — `v1.0.0-rc.1` Federation Trust Boundary

**Phase:** 12-v1-0-0-release-gate · **Plan:** 12-02 · **Requirement:** REL-02 (design review C §16 item 9)

## Reviewer Independence

Two genuinely independent adversarial passes were run over the bounded surface, neither
of which was shown `11-VERIFICATION.md`, the Phase 11 SUMMARYs, or any prior verdict:

1. **This executor**, working directly from source (`crates/famp-gateway/src/egress.rs`,
   `ingress.rs`, `main.rs`, `crates/famp-bus/src/broker/handle.rs`/`identity.rs`/
   `drain_walk.rs`, `crates/famp-core/src/identity.rs`), answering all eight adversarial
   questions from `12-RESEARCH.md` § REL-02 before reading anything Phase-11-verdict-shaped.
2. **`codex exec`** (a separate model, `codex-cli 0.145.0`), run in the background
   (`--sandbox read-only`, no network) against a brief containing ONLY the five bounded
   surfaces, the eight adversarial questions, and an explicit instruction to withhold
   any prior verdict and work from source alone. Full transcript preserved at
   `/private/tmp/claude-501/.../scratchpad/codex-rel02-output.txt` for this session
   (not committed — ephemeral scratch, not a phase artifact).

Both passes independently converged on the same "working as designed" verdict for
question (d) and the same byte-exact/case-sensitive verdict for question (e)'s core
comparisons, and both independently flagged the timestamp lexical-comparison issue
under question (h) — convergent evidence, not a single reviewer's hunch. Codex's pass
additionally surfaced four findings this executor's own pass had reasoned past or
under-weighted (own-domain fail-open reachability via the documented setup sequence,
the `Target`/envelope-`to` decoupling, the per-gateway-key/per-Principal-keyring scope
mismatch, and the silently-skipped single-peer route). **Every codex-sourced finding
below was independently re-verified against the cited source by this executor before
any disposition was assigned** — per the coordinator's explicit instruction, an AI
reviewer's claim that could not be reproduced from source would have been recorded as
a false positive rather than acted on. None were; all six of codex's claimed findings
reproduced exactly as cited (see `## Findings` for the verification trail).

`gemini` and `cursor` were not used (documented unauthenticated/ineligible on this
machine, per project memory `project_gsd_review_cli_state`).

## Reviewed Surface

`git diff ba6b166..HEAD -- crates/famp-gateway/src crates/famp-bus/src/broker/handle.rs`
is **empty** — the surface reviewed is byte-identical to what `v1.0.0-rc.1` tagged
(HEAD at review start: `8cde34a`). The precondition is satisfied without a delta.

Two fixes landed as a direct result of this review (see `## Triage`); the post-fix
working-tree diff is:

```
crates/famp-envelope/src/envelope.rs  | 45 +++++++++++++++++++++++++++++++----
crates/famp-envelope/src/timestamp.rs | 18 ++++++++++++++
crates/famp-gateway/src/main.rs       | 22 +++++++++++++++--
```

`crates/famp-envelope` is a Layer-0 protocol-primitive crate not in the plan's
pre-declared `files_modified` list — its inclusion is a documented deviation (Rule 1:
auto-fix a bug found during execution), justified in Triage row F-6 below. **Neither
fix touches canonical JSON, the `FAMP-sig-v1\0` domain-separation prefix, signature
input, or any wire-visible serialization.** The `envelope.rs`/`timestamp.rs` change is
scoped entirely to `SignedEnvelope::federation_format_ok`'s internal boolean
predicate — it changes how an already-parsed, already-signed `ts`/`expiry` pair is
COMPARED, never how it is encoded, canonicalized, or signed. `Timestamp`'s `Serialize`/
`Deserialize` impls, `shallow_validate`, and every signing/canonicalization path are
byte-for-byte unchanged.

## Findings

Findings are grouped by adversarial question (a)-(h) from `12-RESEARCH.md` § REL-02.
Each cites file:line evidence and states whether it reproduced from source (both
reviewers independently, one reviewer only, or false-positive).

### (a) Does the signed federated `from` derive from the broker's authenticated identity?

**F-1.** Partially. The broker (`crates/famp-bus/src/broker/handle.rs:392,419`, via
`resolve_op_identity` and `is_self_authored` in `drain_walk.rs:197`) binds only the
**leaf name** (`from.rsplit('/').next()`) to the connection's authenticated identity —
the **authority/domain** portion of `from` is left entirely client-supplied at this
layer. The domain is bound separately, at egress, ONLY when own-domain is configured
(`egress.rs:258`, `RelayError::FromDomainMismatch`). When own-domain is unset, the
domain is signed verbatim from client input. Reproduced independently by both
reviewers; not a distinct finding beyond (c) — see F-3 for disposition.
Severity: high (subsumed into F-3).

### (b) Does `BusMessage::Send` separate the local mailbox routing target from the envelope `to`, and can they disagree?

**F-2.** Yes on both counts, confirmed at source. `Target` (`crates/famp-bus/src/proto.rs:51-54`)
and the envelope's own `to` field are independent — `handle.rs::send` never compares
`Target::Agent{name}` against `envelope["to"]` (`handle.rs:378-436`); it uses `Target`
only to select the mailbox (`send_agent`/`send_channel`, `handle.rs:432-434`) and
appends the envelope content unchanged. Confirmed EXPLICITLY documented, not
accidental: `crates/famp/src/cli/send/mod.rs:513-515`'s own doc comment states
"the broker routes by `BusMessage::Send.to: Target`, not by the envelope `to`
field" for channel posts specifically. Codex-sourced; reproduced by this executor.
Disposition and exploit-scenario analysis in Triage row F-1.
Severity: medium (see Triage for why it does not rise to high).

### (c) Own-domain UNCONFIGURED behavior — fail-open or fail-closed?

**F-3.** Fail-open on BOTH sides, confirmed at source:
- Egress: `if let Some(expected) = own_domain { ... }` — `None` skips the comparison
  entirely and signs `from` verbatim (`crates/famp-gateway/src/egress.rs:258-266`).
- Ingress: the `else` branch logs "own-domain unset; skipping to-authority check" and
  continues (`crates/famp-gateway/src/ingress.rs:265,278-283`).
- Startup: `OwnDomainNotSet` is converted to plain `None` with only an `eprintln!`
  warning, never a hard exit (`crates/famp-gateway/src/main.rs:234-246`).

**Reachability confirmed against the documented deployment flow**, not merely
hypothetical: `docs/GATEWAY-SETUP.md` § 4 "Start each gateway" (line 168) precedes
§ 5 "Configure your own-domain" (line 224) — the documented sequence starts the
gateway process BEFORE the operator configures own-domain, and own-domain is resolved
exactly once at gateway startup (`main.rs:339`, comment: "resolve this host's own-domain
federation authority ONCE at startup"). A gateway started per § 4 and never restarted
after § 5 would run its entire session with `own_domain = None`. Reproduced
independently by codex; this executor's own pass initially reasoned past this as
"already accepted" without confirming the doc-sequencing angle — codex's citation of
the §4/§5 ordering is the sharper, source-grounded version of the same finding.
Severity: high. Disposition in Triage row F-2 (documented-accept, not fixed).

### (d) Exact order of ingress checks; is any state created before the last rejection?

**F-4.** Order confirmed: (1) URL principal parse (`BadPrincipal`), (2) signature
verification via `verify_inbound_any` (`InvalidSignature`/`UnpinnedKey`,
`ingress.rs:230-242`, `verify.rs:36,62`), (3) misaddressed-recipient
(`ingress.rs:253-264`), (4) foreign-domain when configured (`ingress.rs:265-283`),
(5) federation-format (`ingress.rs:284-287`), (6) JSON re-parse + wrapper strip
(`ingress.rs:296-315`), (7) registry lookup (`ingress.rs:322-326`, a plain
`HashMap::get_mut` — read-only), (8) `Send` (mailbox append, the only mutation).
**No mailbox or registry state is created before any of the five reject paths.**
Corroborated by `crates/famp-gateway/tests/inbound_destination_validation.rs`'s three
tests (foreign-domain, misaddressed-recipient, well-formed-delivers — all three assert
mailbox state directly, not just a status code) and `ingress.rs`'s own unit test
(`invalid_signature_and_unpinned_key_map_to_distinct_4xx_with_no_registry_mutation`).
One coverage gap noted, not a defect: `MalformedFederationFields` has no dedicated
integration test asserting mailbox-untouched (its code position — before the registry
lock at `ingress.rs:322` — guarantees the property structurally, but it is untested
directly). Reproduced independently by both reviewers.
**Verdict: working as designed. No finding.**

### (e) Exact-equality vs. one-character-difference behavior at each boundary

**F-5.** All five boundary comparisons are byte-exact, case-sensitive `==`/`!=` with
no normalization — confirmed at `crates/famp-core/src/identity.rs:18-22` (`Principal`
stores `authority`/`name` as plain `String`, `#[derive(PartialEq, Eq)]`, no case-fold)
and `validate_authority` (`identity.rs:212-242`, ASCII-alphanumeric-or-hyphen-per-label,
rejects empty authority and empty labels — so a trailing dot or empty authority never
survives parsing to reach a comparison at all). Table of comparisons and near-miss
behavior:

| Boundary | Expression | file:line | Near-miss (case/char diff) |
|---|---|---|---|
| Broker `from` leaf ↔ authenticated identity | `sender == reader` | `drain_walk.rs:204` | any diff rejects |
| Egress `from` authority ↔ own-domain | `got != expected` | `egress.rs:260` | any diff rejects |
| Signed `to` ↔ URL-path recipient | `envelope_to != recipient` | `ingress.rs:254` | any diff (authority OR name) rejects |
| Ingress `to` authority ↔ own-domain | `got != own_domain.as_ref()` | `ingress.rs:267` | any diff rejects |
| `--backs` authority ↔ `--peer` domain | `.find(\|(d,_)\| d == domain)` | `main.rs:293` | any diff → "no matching --peer domain", startup-fatal |

Byte-exact/case-sensitive equality itself is documented, intentional behavior — **not
a finding.** One genuine sub-finding surfaced under this question: the bare-positional-
name + single-`--peer` fallback (`main.rs:273`, pre-fix) built `agent:{domain}/{name}`
and **silently skipped** the route on a `Principal`-parse failure (`if let Ok(...)`),
since `--peer`'s own parser validates only non-emptiness of the domain
(`main.rs:158-161`), never Principal-legality. Reproduced independently by codex;
this executor confirmed by tracing `registry.rs::back` (no name-charset validation)
and reproducing the exact silent-skip + `println!("... ready ...")` + never-exit
behavior live (the RED-state regression test hung indefinitely rather than failing
cleanly, because the buggy gateway looks healthy forever). Severity: medium
(availability/config, not a trust-boundary bypass). **Disposition: FIXED** — see
Triage row F-6... wait, see the "Triage" table's **F-config** row below.

### (f) Signing-key scope vs. ingress trust-lookup scope

**F-6.** Confirmed mismatch. The gateway's Ed25519 signing key is loaded from ONE path
per `$FAMP_HOME` (`crates/famp/src/cli/peer/identity.rs:14`,
`gateway_identity_path`), and every per-principal `run_egress` task calls the same
`load_or_generate` (`main.rs:341,397-403`) — i.e. **one key per gateway process**,
shared across every locally-backed principal under that gateway. Ingress, however,
verifies by looking the sender up in `Keyring::get(&from)`
(`crates/famp-gateway/src/verify.rs:36,62`), a `HashMap<Principal, TrustedVerifyingKey>`
(`crates/famp-keyring/src/lib.rs:29,115`) — trust is pinned **per full Principal**
(per agent), not per-domain or per-gateway. Confirmed reachable functional break:
`Keyring::load_from_file` explicitly rejects the SAME pubkey pinned under two
different principals (`famp-keyring/src/lib.rs:74-80`, `DuplicatePubkey`) — so an
operator backing 2+ agents under one gateway (the CLI's `<principal-name>...` accepts
`≥1`, per `main.rs`'s own usage line) cannot have both agents trusted by a peer once
the peer's keyring file is reloaded (e.g. any gateway restart on the peer's side).
Reproduced independently by codex; this executor confirmed both the single-signing-key
load site and the `DuplicatePubkey` file-load check. **Not reachable through the
documented v1.0 flow**: `docs/GATEWAY-SETUP.md` §4's worked example backs exactly ONE
principal per gateway on each side (`bob` on A, `alice` on B). Fails CLOSED
(`DuplicatePubkey` is an explicit typed startup error), not open. Severity: medium
(functional/interop limitation, not a security hole). Disposition in Triage row F-3
(documented-accept).

### (g) Egress durability — retried, durable, or silently dropped?

**F-7.** Confirmed: `run_egress`'s `Await` (`AWAIT_TIMEOUT_MS = 1_000`,
`egress.rs:38-42,336-344`) advances the broker's mailbox cursor as part of producing
`AwaitOk` — this happens server-side, before `relay_one` ever runs client-side. If
`relay_one` then fails (transport, sign, or domain-mismatch error), the loop only
`eprintln!`s and moves on (`egress.rs:359-367`); there is no requeue, no retry, and
the module's own doc explicitly states this contract (`egress.rs`, module doc + the
`ClientSuppliedFederationField` variant's own comment at line 268-274: "the drain
loop never re-queues or retries a failed relay"). A `SendOk` is returned to the local
sender at mailbox-append time (`handle.rs:481`, before egress or HTTP ever runs), so
the local sender has no visibility into a downstream relay failure. Reproduced
independently by both reviewers. Severity: high per codex's framing (a full fix needs
an outbox/ack state machine); this executor assesses the OPERATOR-FACING consequence
as already mitigated — see Triage row F-4.

### (h) Numeric-field precision, width, and comparison contract

**F-8 (the confirmed, fixed defect).** `federation_format_ok`
(`crates/famp-envelope/src/envelope.rs`, pre-fix lines 519-546) compared
`expiry.0 <= self.inner.ts.0` via raw lexical byte-string comparison. `shallow_validate`
(`crates/famp-envelope/src/timestamp.rs:19-47`) independently accepts EITHER a `Z`
suffix or a `+HH:MM`/`-HH:MM` offset for `ts` and `expiry`, with no requirement that
the two share a representation, and does not bound subsecond digits. Concretely
reproduced (both reviewers, independently, converging on the same root cause):
`expiry = "2026-04-13T00:00:01+01:00"` (actual UTC instant `2026-04-12T23:00:01Z`,
one hour BEFORE `ts = "2026-04-13T00:00:00Z"`) lexically compares GREATER than `ts`
(byte 19 is `1` vs `0`), so `expiry.0 <= ts.0` evaluates false and the envelope was
wrongly treated as well-formed — a validation defect at a trust-boundary function.
**Disposition: FIXED**, see Triage row F-5.

**F-9 (documented-accept, no code change).** Missing active expiry/replay enforcement:
`federation_format_ok`'s own doc comment already states this is a "D-04
well-formedness check ONLY... does NOT reject an expired (past) `expiry` and does NOT
consult any replay cache." `REQUIREMENTS.md:101` names `INGRESS-01` ("protocol-grade
ingress — freshness-window + replay-cache enforcement") as the explicit v1.1 home for
this. Not a v1.0 defect — an already-scoped deferral. Codex independently confirmed
the same scope-boundary read ("missing active expiry/replay enforcement is an
accepted, documented scope boundary").

**F-10 (documented-accept, no code change).** Arbitrary nonce width: `nonce` need only
be non-empty (`egress.rs`'s `federation_format_ok` nonce check); no UUID form or width
is enforced. Bounded by the existing 1 MiB ingress body-size limit
(`ingress.rs:44-47`, `ONE_MIB`) and moot without the replay cache `INGRESS-01` will
add (an unbounded nonce has no exploitable effect while nothing consults it for
uniqueness). Codex independently reached the same conclusion.

## Triage

Every finding above is assigned exactly one disposition. No finding is left untriaged.

| ID | Question(s) | Disposition | Rationale |
|----|-------------|-------------|-----------|
| F-5 | (h) — timestamp lexical comparison (F-8) | **fixed** | Regression test `federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders` (`crates/famp-envelope/src/envelope.rs`) written RED-first, confirmed failing against the unfixed code (`cargo test -p famp-envelope --lib`, 1 failed), then fixed by adding `crate::timestamp::is_canonical_utc_form` (whole-second, `Z`-suffixed, exactly 20 bytes) as a precondition on BOTH `ts` and `expiry` before trusting the lexical `<=` comparison inside `federation_format_ok`. Confirmed GREEN (`cargo test -p famp-envelope --lib`, 41/41). Highest priority per the coordinator's explicit ordering (auth/trust-boundary → data integrity → config): this is a trust-boundary validation function, and both independent reviewers converged on it. Zero wire-visible change — see `## Reviewed Surface` for the confirmation that `Timestamp` serialization, canonicalization, and signing are byte-for-byte untouched. |
| F-config | (e) — silently-skipped invalid single-peer route (part of F-5 above) | **fixed** | Regression test `invalid_single_peer_domain_fails_startup_instead_of_silently_dropping_route` (`crates/famp-gateway/tests/route_config_fail_closed.rs`) written RED-first; against the unfixed code the test HUNG rather than failing cleanly, because the buggy gateway silently drops the route, prints "ready", and never exits — itself the concrete proof of the defect (an operator would see a healthy-looking process with a permanently dead route). Fixed by changing the `if let Ok(principal) = ...` silent-skip in `build_route_map`'s single-peer branch (`crates/famp-gateway/src/main.rs`) to `std::process::exit(1)` with an actionable message naming both the bad `--peer` domain and the affected backed name — matching the fail-closed philosophy already used by every other branch in this exact function (duplicate peer domain, duplicate `--backs`, ambiguous multi-peer bare names, `--backs` with no matching peer all already exit(1)). Confirmed GREEN (`cargo test -p famp-gateway --test route_config_fail_closed`, 6/6, ~1.8s — no hang). Config-tier priority per the coordinator's ordering; lowest blast radius of the two fixes (startup-time-only, `main.rs`-local). |
| F-1 | (b) — `Target`/envelope-`to` decoupling | **documented-accept** | Real and reproduced at source (F-2), but re-examined for actual exploit impact: the divergence lets a local sender's envelope land in a DIFFERENT backed principal's outbound mailbox than its own `to` claims, which could let an unbacked local agent get relay reach by targeting an already-backed principal's mailbox. However, this is a property of the **local bus's pre-existing, v0.9-era trust model** ("drop crypto on the local path... any locally registered client is trusted" — `ARCHITECTURE.md`), not a NEW cross-host trust-boundary defect: any locally-registered client already has ambient authority to DM any other local mailbox by design, and the `--backs` list was never framed as a security allowlist — it is a routing-convenience map (which REMOTE principal names this gateway proxies), documented as such in `main.rs`'s own module doc. A real fix (binding a local sender's own identity to which egress task may relay on its behalf) is an architectural redesign of the mailbox-routing model. Evidence against a narrower fix: `handle/tests.rs`'s own `audit_log_envelope` test helper hardcodes `to = "agent:example.test/dave"` regardless of the `Target` used across 15+ existing `Target::Agent{name: "bob"}`-shaped test call sites — enforcing Target/`to` agreement at the broker level would require rewriting a large fraction of `handle/tests.rs`'s existing fixtures, which is disproportionate blast radius for a release-gate fix and risks introducing new bugs into a widely-shared test helper. Forward-pointer: a future v1.1/v2.0 gateway-egress-authorization pass (alongside `INGRESS-01`/`PEER-01`) is the right place to scope a proper fix, if the multi-agent-per-gateway topology is ever formally supported. |
| F-2 | (a), (c) — own-domain fail-open when unconfigured | **documented-accept** | Real, and the doc-sequencing angle (§4 before §5 in `GATEWAY-SETUP.md`) is a genuinely sharper version of this finding than Phase 11 recorded. It is not, however, an undiscovered defect: `ingress.rs:280-282`'s own code comment already names this "T-11-29 residual — accepted posture until own-domain is configured", i.e. Phase 11 consciously chose this over a hard startup requirement. A proper fix (mandatory own-domain at gateway startup) is an architecture/deployment-contract change: it would deliberately break `crates/famp-gateway/tests/e2e_shipping_surface.rs`'s own-domain-unset regression control (`e2e_shipping_surface.rs:34,294`, explicitly preserved for that purpose), which is a bigger, riskier change than a release gate should make absent a forcing incident. Blast-radius/exploitability check: exploiting the gap requires a peer whose key is ALREADY pinned in this gateway's keyring but who addresses a FOREIGN domain this gateway does not own — under the locked v1.0 "own-two-machines, hand-operated, single peer" scope (`STATE.md`'s v1.0 Architectural Invariant #3), a pinned key belongs by definition to the one deliberate two-machine relationship this milestone targets, so the exploit scenario itself requires stepping outside that locked scope. Forward-pointer: recommend (i) reordering `GATEWAY-SETUP.md` §4/§5 so own-domain is configured before first gateway start, and (ii) considering a mandatory `--domain`/own-domain-at-startup requirement alongside the already-planned `INGRESS-01`/`PEER-01` v1.1 protocol-grade-ingress hardening pass. Neither is done in this plan — out of scope per the "no federation logic changes absent a real defect, and no architecture changes in a release gate" fence. |
| F-3 | (f) — per-gateway signing key vs. per-Principal keyring pin | **documented-accept** | Real functional limitation, confirmed against source (F-6), but (i) fails CLOSED (`DuplicatePubkey` is an explicit typed startup error on the peer's side, not a silent trust bypass), (ii) is not reachable through the documented/tested v1.0 flow (single agent per gateway, per `GATEWAY-SETUP.md` §4's worked example), and (iii) a real fix (per-agent signing keys, or gateway/domain/key-id-scoped trust with a separate principal-authorization layer) is a signing-key-model architecture change squarely outside a release gate's "no federation logic changes absent a real defect" fence. Forward-pointer: document the single-agent-per-gateway limitation explicitly in a future `GATEWAY-SETUP.md` revision, or design a proper multi-agent-per-gateway trust model in v1.1/v2.0 if that topology is ever needed. |
| F-4 | (g) — egress non-durability / lost-on-relay-failure | **documented-accept** | Real (confirmed at F-7), and codex assesses it "high severity" — this executor agrees on the underlying fact but not on the disposition, because the operator-facing consequence is **already mitigated** by this same phase's own prior plan: REL-01 (12-01, commits `44a19e7`/`2f743c9`, already landed and CI-green before this plan ran) added the exact fire-and-forget confirmation-boundary language to `docs/GATEWAY-SETUP.md` §6, `famp send --help`, and README, stating explicitly that a successful `famp send` "does not confirm that the gateway has drained, signed, and relayed the envelope... egress is a decoupled background drain loop... that the CLI process never waits on." The phase's own threat register (`12-02-PLAN.md` T-12-02-05) names exactly this mitigation path ("REL-01... documents the resulting operator-visible boundary regardless of disposition"). A real fix (an outbox/ack state machine, or deferring cursor commitment until remote acceptance) is a substantial reliability-architecture change, not a bounded release-gate fix. Forward-pointer: a future reliability milestone should design durable/retried egress delivery. |
| F-9 | (h) — missing active expiry/replay enforcement | **documented-accept** | Already an explicitly-scoped v1.1 deferral (`INGRESS-01`, `REQUIREMENTS.md:101`); `federation_format_ok`'s own doc comment states the D-04 well-formedness-only scope. Not a new finding requiring any disposition beyond citing the existing deferral. |
| F-10 | (h) — arbitrary nonce width | **documented-accept** | Bounded by the existing 1 MiB ingress body-size cap and moot absent the replay cache `INGRESS-01` will add; not independently exploitable in v1.0. |
| — | (d) — ingress check order / no state before rejection | **documented-accept (no defect)** | Confirmed working as designed by both reviewers; the one coverage-gap observation (no dedicated mailbox-untouched test for `MalformedFederationFields`) is a test-coverage note, not a behavioral defect — the property is guaranteed by code position (before the registry lock), corroborated by three existing `inbound_destination_validation.rs` tests covering the other two reject paths with state-based assertions. |
| — | (e) — byte-exact/case-sensitive comparisons | **documented-accept (no defect)** | Intentional, documented behavior (see F-5 table); the one genuine sub-finding under this question (silently-skipped invalid single-peer route) is triaged separately as F-config above (fixed). |

**No disposition in this table appeals to when the tag ships or to any release-process
urgency.** Every accept rationale rests on: an existing named prior decision (T-11-29),
a documented/tested deployment-scope boundary (single-agent-per-gateway,
own-two-machines), an already-shipped mitigation (REL-01), an already-scoped v1.1
deferral (`INGRESS-01`), or a blast-radius/architecture-change judgment call explained
on its own technical merits.

## Outcome

REL-02 is closed with **two confirmed, fixed defects** (not a zero-finding outcome) and
**eight documented-accept dispositions**, all ten with written, source-grounded rationale.
The phase's hard constraint ("no federation logic changes unless REL-02 surfaces a real
defect") was honored precisely: the two changes that DID land are each traceable to a
specific, independently-reproduced defect with a RED-first regression test, and every
other candidate finding — including four genuinely real ones (F-1, F-2, F-3, F-4) — was
deliberately NOT converted into a code change because a real fix for each would exceed
this release gate's scope (architecture change, deployment-contract change, or
already-mitigated via a prior plan in this same phase).

### Control test re-run (post-fix, evidence)

| Command | Result |
|---|---|
| `cargo test -p famp-envelope --lib` | 41/41 passed (incl. the new RED-then-GREEN test) |
| `cargo test -p famp-gateway --lib` | 18/18 passed |
| `cargo test -p famp-gateway --test inbound_destination_validation --test route_config_fail_closed` | 3/3 + 6/6 passed (incl. the new RED-then-GREEN test) |
| `cargo test -p famp-bus` | all binaries green (81 lib + all integration/property-test binaries, 0 failed) |
| `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | clean, 0 warnings, 0 errors |

`git diff --name-only` shows changes under `crates/` limited to exactly the two fixed
findings' files (`crates/famp-envelope/src/envelope.rs`, `crates/famp-envelope/src/timestamp.rs`,
`crates/famp-gateway/src/main.rs`) plus their regression tests
(`crates/famp-gateway/tests/route_config_fail_closed.rs`) — the scope fence held: no
file was touched that is not named in a `fixed` triage row.
