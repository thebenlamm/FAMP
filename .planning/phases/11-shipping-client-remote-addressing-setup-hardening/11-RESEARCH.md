# Phase 11: Shipping-Client Remote Addressing + Setup Hardening - Research

**Researched:** 2026-07-28
**Domain:** FAMP cross-host addressing (sender-side envelope construction), Ed25519/INV-10 trust boundary, cross-platform TLS (rustls-platform-verifier), Rust CLI + gateway integration
**Confidence:** HIGH (every claim traced to ground-truth source in-repo; the one semi-external fact — Apple SecTrust EKU enforcement — is cross-checked against the `rustls-platform-verifier` dep AND the 2026-07-28 dogfood)

## Summary

The design is settled (D-01..D-09, external design pass + zed source control). This research does **not** re-derive the fix. It resolves the one open sub-problem (D-05, own-domain source for the envelope `from`), confirms the concrete recipes the planner needs (cert generation, CI, gates), and pins the load-bearing coupling every plan must preserve.

The core mechanic is a **three-process agreement problem**. `famp send` (stamps envelope `from`/`to`), `famp peer export` (produces the pinned label the peer trusts), and `famp-gateway` (signs egress with its own key) are three separate OS processes that must agree on this host's federation domain. Ingress verifies on the envelope `from` (`verify.rs:41,62` → `peek_sender` → `keyring.get(from)`), so the domain in `from` MUST byte-equal the `--as` principal the peer pinned. Today nothing enforces that — `famp send` hardcodes `agent:local.bus/<identity>` (`send/mod.rs:425`), and the human types the export label by hand.

**Primary recommendation (D-05):** Resolve own-domain from a **single host-level config value** (option **a**) — an env var `FAMP_OWN_DOMAIN` with an on-disk fallback `$FAMP_HOME/own-domain`, plus a `--domain <domain>` CLI override on `famp send`. Read it in `famp send` to stamp `from = agent:{own-domain}/{identity}`, and derive the `famp peer export` label from the same source. This is the **only** option that structurally enforces the `from == pinned-label` coupling, because three independent processes can only agree without drift by reading one shared source. Options (b) and (c) leave two human-managed inputs that can silently diverge into a `UnpinnedKey` reject (D-02's exact failure symptom). Sequence D-06 (error-chain) **first** — it is the force-multiplier on every subsequent two-machine debug.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01 (C5 pass-through for `to`):** In `build_envelope_value`, make the target conditional — if `--to` parses as a `Principal` (`agent:<domain>/<name>`), use it verbatim as envelope `to`; else wrap in `agent:local.bus/<name>` as today. One chokepoint covers `famp send`, `/famp-send`, and the MCP tool; no new required flag; `--to bob` unchanged; scales to N peers. (`--to <name> --domain <domain>` sugar acceptable; full-principal form primary.)
- **D-02 (BOTH `from` and `to` must be domain-qualified):** A `to`-only rewrite is INSUFFICIENT and strictly worse — ingress verifies on `from` (`verify.rs:62-63`, `peek_sender` returns `from`); a `local.bus` `from` → `UnpinnedKey`. A remote send MUST stamp `from = agent:{own-domain}/{identity}` too.
- **D-03 (typed unsigned `request`, NO local crypto):** Remote sends emit a typed `RequestBody` envelope via the sanctioned sign-then-strip / BUS-11 pattern (sign with throwaway key, strip signature → unsigned on the local bus; gateway re-signs at egress). Drives the FSM without reopening the unsigned-local-bus decision. Mirror the E2E / wire-proof-injector shape.
- **D-04 (gate the class upgrade on remote):** Bare-name local sends stay class `audit_log` (unchanged local-chat). Only domain-qualified (remote) sends emit the typed `request`.
- **D-06 (transport error chain, OBS-01) — SEQUENCE FIRST:** `error.rs:63` Display=`"reqwest failure"` + `egress.rs:211` `e.to_string()` discard the `#[source]`. Log the full `.source()` chain (or `{e:?}`). Cheapest fix and force-multiplier; first plan/wave.
- **D-07 (GATEWAY-SETUP.md, DOC-05):** correct all 8 findings (§4 back the remote principal; §3 pin under the sender agent principal; pin-before-launch/no-hot-reload; duplicate-pubkey keyring brick; move "ready" after keyring load; CA:FALSE+serverAuth cert recipe for BOTH platforms; macOS host-firewall pre-auth). Strengthen the DOC-04 accuracy gate beyond flag-grep.
- **D-08 (fixtures + CI, TEST-03):** regenerate `crates/famp/tests/fixtures/cross_machine/*` to CA:FALSE+serverAuth EKU; add a macOS CI leg exercising the Apple-verifier path.
- **D-09 (shipping-surface test, TEST-03):** integration test driving the FIXED `famp send` cross-host + a NEGATIVE test that a `local.bus`-authority envelope through the federated path yields a typed error (not a silent drop). DELETE throwaway artifacts (`wire_proof_inject.rs`, `probe_tls.rs`) once it exists.

### Claude's Discretion
- Exact flag ergonomics (`--to agent:...` vs `--domain`), the precise own-domain config key/name, and test file layout — subject to D-01..D-05.

### Deferred Ideas (OUT OF SCOPE)
- None new. Explicitly out of scope: relay, public-internet reachability, cross-person trust, signed directory, replay-cache/freshness enforcement (nonce/expiry format-validated only), capability/approval/tool-admission plane. Do NOT reopen the v0.9 unsigned-local-bus decision.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ADDR-01 | Shipping `famp send`/`famp_send` can address a remote principal | D-01 chokepoint at `build_envelope_value` (`send/mod.rs:413`); parse `--to` as `Principal`, split bus-Target-leaf from envelope-`to` (see Pattern 1) |
| ADDR-02 | Remote send drives a signed cross-host delivery to a terminal task state | D-03 typed `RequestBody` via sign-then-strip (E2E `unsigned_value`/`build_request`, `e2e_cross_host_delivery.rs:406-436`); gateway signs at egress (`sign_federation_fields`) |
| ADDR-03 | Own-domain source for envelope `from` (the open design sub-problem) | **§ D-05 Resolution below** — recommend option (a), host-level config; enforces `from == pinned-label` |
| OBS-01 | Transport error chain no longer swallowed | D-06: `error.rs:63` + `egress.rs:211`; log `.source()` chain / `{e:?}` |
| DOC-05 | GATEWAY-SETUP.md corrected + accuracy gate strengthened | D-07: 8 findings mapped to exact doc sections below; semantic gate beyond flag-grep |
| TEST-03 | Fixtures regen + macOS CI leg + shipping-surface integration test | D-08/D-09: fixtures are ECDSA/no-EKU today (verified); macOS matrix leg ALREADY exists (`ci.yml:105-118`) |
| UAT-01 | Live two-machine re-run with real `famp send`, no injector | Definition of Done; opus laptop ↔ zed server; final human gate before v1.0.0 |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Parse remote target `--to agent:d/n` | CLI client (`famp send`) | — | Sender authors the destination (Invariant A, signed-recipient truthfulness) |
| Stamp envelope `from`/`to` (domain-qualified) | CLI client (`famp send`) | Config (own-domain source) | Sender authors provenance; ingress verifies on `from` |
| Split bus-route-target (leaf) vs envelope-`to` (full principal) | CLI client + local bus | — | Bus routes to local gateway proxy mailbox by bare leaf name; envelope carries remote principal |
| Federation-sign at boundary | `famp-gateway` egress | `famp-crypto` | BUS-11: local bus unsigned; crypto transition only at egress |
| Verify inbound + pin lookup | `famp-gateway` ingress | `famp-keyring` | TRUST-02: `keyring.get(peek_sender(from))`, hard-reject unpinned |
| Own-domain agreement across 3 processes | Host config (shared) | — | Only a shared read source prevents drift between send / export / gateway |
| Channel-encryption TLS (cross-host) | `famp-transport-http` (rustls) | OS trust store + `--trust-cert` | Apple SecTrust (macOS) vs webpki (Linux) enforce EKU/CA differently |

## Standard Stack

All work is **internal crates** — no new external dependencies. No `## Package Legitimacy Audit` needed (nothing installed).

### Core (already in-tree, reused)
| Crate | Purpose | Why Standard |
|-------|---------|--------------|
| `famp-envelope` | `UnsignedEnvelope<RequestBody>`, `peek_sender`, `AnySignedEnvelope`, `Causality` | The typed envelope surface D-03 mirrors; `peek_sender` at `crates/famp-envelope/src/peek.rs:28` |
| `famp-core` | `Principal` (`agent:{authority}/{name}`), `validate_authority`, `validate_name_or_instance_id` | Parsing `--to` as `Principal`; authority/name charset rules (`identity.rs:210,245`) |
| `famp-crypto` | Ed25519 sign/verify, `FampSigningKey`, `key_id`, `FAMP-sig-v1\0` domain prefix (INV-10) | Gateway egress signing; never hand-rolled |
| `famp-keyring` | TOFU pin store; `keyring.get(principal)` | Ingress trust decision (TRUST-02) |
| `famp-transport-http` | rustls 0.23 client via `rustls-platform-verifier` 0.5 | Cross-host wire; the EKU/CA cross-platform gotcha lives here |
| `famp-bus` | `BusMessage::Send { to: Target, envelope }`, `Target::Agent{name}` | The split-addressing seam (bus Target ≠ envelope `to`) |

**Version verification:** N/A — no installs. `rustls-platform-verifier = "0.5"` confirmed in `crates/famp-transport-http/Cargo.toml:28`.

## Architecture Patterns

### System Architecture Diagram (shipping send → cross-host delivery)

```
famp send --to agent:hostb.test/bob --new-task "..."   [Machine A]
  │  build_envelope_value (send/mod.rs:413) — D-01/D-03/D-04
  │   ├─ parse --to as Principal? YES → remote path
  │   │    envelope.from = agent:{OWN_DOMAIN}/{identity}   ← D-05 (the open bit)
  │   │    envelope.to   = agent:hostb.test/bob            ← verbatim (D-01 C5)
  │   │    class = "request", typed RequestBody, sign-then-strip (D-03)
  │   │    bus Target    = Agent{ name: "bob" }            ← LEAF only (split-addressing)
  │   └─ parse fails → local path (unchanged: agent:local.bus/<name>, audit_log)
  ▼
Local bus A ──(unsigned, BUS-11)──► gateway A's "bob" proxy mailbox
  ▼  run_egress drains (egress.rs:228)
  │   sign_federation_fields (egress.rs:89): +from_domain/to_domain/nonce/expiry, Ed25519 sign
  │   transport.send(recipient = envelope.to)  ← add_peer keyed by FULL principal (main.rs:255)
  ▼  HTTPS POST (rustls; Apple SecTrust on macOS / webpki on Linux)
Gateway B ingress ──► verify_inbound_any (verify.rs:58): peek_sender(from) → keyring.get → verify_strict
  │   strip_relay_fields (BUS-11) ──► local bus B ──► REAL bob mailbox
  ▼
famp inbox / famp inspect tasks  [Machine B]
```

Reader can trace input→output by the arrows. File-to-responsibility mapping is in the Responsibility Map above.

### Pattern 1: Split-addressing (the load-bearing mechanic)

**What:** The bus routing target and the envelope `to` are two different things. The bus `Target::Agent{name}` must be the **bare leaf** (`"bob"`) so the local broker routes to gateway A's `bob` proxy mailbox; the envelope `to` must be the **full remote principal** (`agent:hostb.test/bob`) so egress transport lookup (keyed by full principal, `main.rs:255-261`) and receiver verification see the real destination.

**When to use:** Every remote send. The E2E already relies on exactly this split — `send_bus_envelope(sock, bind_as, to_name, envelope)` sends to `Target::Agent{ name: to_name }` (bare "bob") while the envelope carries `to = agent:hostb.test/bob` (`e2e_cross_host_delivery.rs:519-543, 708-709`).

**Example (proof obligation from grounded design report, Invariant appendix):**
```
route_target_local          = proxy(leaf(E.to))   // bus Target = "bob"
recipient_signed            = E.to                 // agent:hostb.test/bob
recipient_transport         = E.to
recipient_observed_after_verify = E.to
```
All four must be equal for the remote leaf. D-01's `build_envelope_value` change: parse `--to` as `Principal`; on success set envelope `to` = the full principal and bus `Target::Agent{ name = principal.name().leaf }`; on parse-failure keep today's `agent:local.bus/<name>` + bare Target.

### Pattern 2: Typed request via sign-then-strip (D-03)

**What:** Build `UnsignedEnvelope::<RequestBody>::new(id, from, to, AuthorityScope::Advisory, ts, body)`, `.sign(throwaway_key)`, `.encode()`, parse to `Value`, `.remove("signature")`. Yields the exact BUS-11-compliant unsigned wire `Value` the bus stores — no "encode unsigned" accessor exists.

**When to use:** Remote (domain-qualified) sends only (D-04). Bare-name sends keep the `audit_log` path verbatim.

**Example:** `crates/famp-gateway/tests/e2e_cross_host_delivery.rs:406-436` (`unsigned_value` + `build_request`) and `crates/famp-gateway/src/egress.rs:308-332` (`plain_request_value`). The fixed `famp send` MUST produce this shape. `RequestBody` requires `scope`, `bounds` (≥2 of 8 fields set — `two_key_bounds()`), optional `natural_language_summary`.

### Anti-Patterns to Avoid
- **Gateway rewrites the signed `to`/`from` (C1/C4):** provenance lie — receiver verifies a principal the sender never authored. Rejected by the design pass. (Exception: the *placeholder* `local.bus` authority under option-b is a filled-in default, not a rewrite of an authored value — see D-05 fallback.)
- **`to`-only rewrite (leaving `from = local.bus`):** D-02 — trades `UnknownRecipient` for `UnpinnedKey`, a symptom that looks like a trust-bootstrap bug. Both `from` and `to` must be qualified.
- **Making all local chat fire the FSM:** D-04 — bare-name sends stay `audit_log`. Only remote sends upgrade to `request`.
- **`--as agent:domain/name` to carry own-domain:** IMPOSSIBLE. `famp send --as` becomes the broker `Hello{bind_as}`, charset-validated `[A-Za-z0-9._-]+` (`identity.rs:245-263`, `register.rs:188`), which rejects `:` and `/`. This is *why* D-05 needs a separate source. (Note: `famp peer export --as` is different — it parses a full `Principal`, so it CAN carry the domain: `export.rs:36`.)

## D-05 Resolution (ADDR-03) — the phase's real design deliverable

### The coupling to preserve (invariant, not optional)

`verify_inbound_any` (`verify.rs:58-66`) does `let from = peek_sender(bytes); keyring.get(&from)`. The peer's keyring was pinned via `famp peer import` of a line produced by `famp peer export --as <principal>` (`export.rs:36-60`, `format_export_line`). Therefore:

> **envelope `from` MUST byte-equal the `<principal>` the peer pinned the sender's gateway key under.**

The gateway signs egress with its **own** key (`~/.famp/gateway/identity.ed25519`, `main.rs:214,224`); the peer pins THAT key under the export label. In the E2E this label is `agent:hosta.test/alice` (the sender *agent* principal), and the envelope `from` is `agent:hosta.test/alice` — they match by construction because both are hand-written constants (`e2e:86, 657`). GATEWAY-SETUP.md §3 currently tells the human to export under `agent:hostA.example/gateway` — **finding #2 confirms this is wrong**: it must be the sender agent principal, because ingress verifies on the agent `from`, not a `/gateway` label. Two independent human inputs, nothing enforcing agreement.

### Candidate analysis (against INV-10, coupling-enforcement, local-bus-unsigned, smallest-change)

| Option | Mechanism | Enforces `from==label`? | Processes touched | Verdict |
|--------|-----------|-------------------------|-------------------|---------|
| **(a) host-level own-domain config** | `famp send` reads it to stamp `from`; `famp peer export` derives the label from it | **YES** — single source; 3 processes read 1 value → cannot drift | `send`, `peer export` (+ gateway may read for validation) | **RECOMMENDED** |
| (b) `--domain` on gateway + egress rewrites `local.bus`→own-domain before sign | CLI stays `from=local.bus`; gateway fills authority pre-sign | **NO** — gateway `--domain` flag ≠ `peer export --as` (separate processes, no shared read) | `send` (to-only), `famp-gateway` (new flag + rewrite) | Fallback only |
| (c) derive own-domain from `peer export` identity/label | Reuse the export `--as` principal | **NO** — `export --as` is per-invocation, not persisted; `identity.ed25519` carries no domain; `famp send` (separate process) has nothing to read | — | **REJECT** — no stable source |

**Why (a) wins the smallest-change test too, despite adding a config surface:** own-domain is a per-host fact shared by three separate OS processes (`famp send`, `famp peer export`, `famp-gateway`). Three processes can only agree without drift via a shared read source. Option (b) appears smaller (localizes to the gateway) but does **not** solve the coupling — `peer export` still can't see the gateway's `--domain`, so the human can still pin `agent:X/alice` while the gateway signs-as `agent:Y/alice` → `UnpinnedKey`. Any real fix for (b) re-introduces a shared config → collapses into (a). Option (c) has no stable persisted source at all.

### Recommended concrete shape (planner may tune the exact key names — Claude's Discretion per CONTEXT)

- **Source & precedence:** `--domain <domain>` CLI flag (highest) → `FAMP_OWN_DOMAIN` env → `$FAMP_HOME/own-domain` file (one line, DNS-style authority) → error with an actionable hint if a remote send is attempted without any source. Resolve alongside `resolve_famp_home` (`main.rs:207`, `home::resolve_famp_home`).
- **`famp send`:** when `--to` parses as a `Principal` (remote), require own-domain; stamp `from = agent:{own-domain}/{identity}` and emit typed `request` (D-03). Validate own-domain via `Principal`/`validate_authority` before use.
- **`famp peer export`:** derive the exported label's authority from the SAME own-domain source (so `famp peer export --as alice` emits `agent:{own-domain}/alice`), OR validate that a supplied full `--as` principal's authority equals the configured own-domain and warn/reject on mismatch. This is what *closes* the coupling: pin-label authority and future `from` authority provably share one value. Also fixes finding #2's `/gateway`-vs-agent label bug.
- **Gateway (optional hardening):** egress MAY assert `envelope.from.authority() == configured own-domain` before signing, converting a future drift into a loud local error instead of a remote `UnpinnedKey`.

**Confidence:** HIGH on the coupling analysis and the process-agreement argument (code-traced). MEDIUM on the exact config key/precedence — that is explicitly Claude's Discretion; the planner should confirm the key name with the user or pick and document.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unsigned typed wire `Value` from `UnsignedEnvelope<B>` | A bespoke "serialize unsigned" path | Sign-then-strip (`unsigned_value`, `plain_request_value`) | BUS-11: no "encode unsigned" accessor exists; the sanctioned pattern is proven in the E2E + egress tests |
| Principal parsing / authority validation | Regex on `--to` | `str::parse::<Principal>()` + `validate_authority` (`identity.rs`) | Charset/label rules are already strict + tested; DNS-label edge cases |
| Ed25519 signing / canonical bytes | Any new signing | `famp::sign_value` / `sign_federation_fields` (egress) | INV-10 domain prefix + RFC 8785 JCS; hand-rolling breaks byte-exactness |
| Cross-host TLS trust | Custom cert validation | `rustls-platform-verifier` (already wired) | It's the source of the EKU/CA cross-platform behavior — configure certs to it, don't bypass |
| TOFU pin lookup | New trust map | `famp-keyring` `keyring.get` | TRUST-02 hard-reject semantics already implemented + tested |

**Key insight:** Everything the fix needs already exists and is tested; the phase is *wiring*, not building. The only genuinely new surface is the own-domain config read.

## Runtime State Inventory

This is a code-change phase with two committed-artifact touchpoints (fixtures, docs). No live datastore migration.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no datastore keys the rename touches; envelope `from`/`to` are constructed per-send, not stored under a renamed key. | None (verified — `from`/`to` are synthesized in `build_envelope_value`, not persisted state) |
| Live service config | Two-machine dogfood hosts (opus laptop, zed `home-devbox`) hold hand-generated TLS certs + peer-pinned keyrings from the 2026-07-28 run. The pinned label there was under the corrected agent principal. | UAT-01 re-run regenerates certs to the CA:FALSE+serverAuth recipe; re-pin if own-domain config changes the label authority |
| OS-registered state | macOS host firewall must pre-authorize `famp-gateway` inbound (finding #6) — a per-machine `socketfilterfw` registration, not in git. | D-07 doc must instruct it; UAT-01 operator performs it |
| Secrets/env vars | New `FAMP_OWN_DOMAIN` env (proposed) + `$FAMP_HOME/own-domain` file. Gateway `identity.ed25519` unchanged (no domain in it). | Document the new config; no secret rotation |
| Build artifacts | `~/.cargo/bin/famp` and `famp-gateway` binaries must be rebuilt — `famp send` behavior compiled in. Per project convention, `just install` after MCP-surface or shipping-binary changes. | Plan a `just install` + binary-freshness step before UAT-01 (see Memory: "Binary freshness before docs-verify") |

**Throwaway artifacts (D-09):** `crates/famp-gateway/tests/wire_proof_inject.rs` and `crates/famp-transport-http/examples/probe_tls.rs` are **NOT in the git tree** (`git ls-files` — verified). They were local/uncommitted dogfood scaffolding. D-09's "delete the injector" is already satisfied for git; the plan should `git ls-files | grep` to confirm and skip the delete step, focusing on *adding* the shipping-surface test.

## Common Pitfalls

### Pitfall 1: The `to`-only rewrite trap (D-02)
**What goes wrong:** Qualifying only `to` and leaving `from = agent:local.bus/<identity>`. Ingress does `peek_sender` → `agent:local.bus/alice` → `keyring.get` → `None` → `UnpinnedKey` reject.
**Why:** `verify.rs:41,62` keys the trust lookup off `from`, not `to`.
**How to avoid:** Stamp BOTH. This is the whole reason D-05 exists.
**Warning signs:** Receiver logs `UnpinnedKey { principal: agent:local.bus/... }` — a trust bug that's actually an addressing bug.

### Pitfall 2: Bus Target = full principal (routing miss)
**What goes wrong:** Setting `Target::Agent{ name: "agent:hostb.test/bob" }`. The local broker has no holder by that literal name → no proxy mailbox match → drop.
**Why:** The bus routes by bare registered/backed name; the gateway backs the bare leaf `bob` (`main.rs:193`, `e2e:667`).
**How to avoid:** Bus Target = leaf only; envelope `to` = full principal (Pattern 1).

### Pitfall 3: Certs that pass on Linux, fail on macOS (and vice versa) — findings #5/#8
**What goes wrong:** `rustls-platform-verifier` 0.5 (`Cargo.toml:28`) delegates to **Apple SecTrust on macOS** and **webpki on Linux**. Apple rejects a no-EKU cert (`EkuError`) and *tolerates* CA:TRUE-as-leaf; webpki *tolerates* absent EKU and *rejects* CA:TRUE-as-leaf (`CaUsedAsEndEntity`). A naive `openssl req -x509` cert fails on **both** in opposite ways. Current fixtures are **ECDSA P-256, self-signed, no EKU, no explicit basicConstraints** (verified via `openssl x509 -text` on `alice.crt`) — Linux-conditionally-green only.
**How to avoid:** The canonical recipe (below) sets `CA:FALSE` (critical) + `extendedKeyUsage=serverAuth`. Regenerate fixtures to it (D-08); use it verbatim in the doc (D-07).
**Warning signs:** `"reqwest failure"` in the log (finding #7 — which is exactly why D-06 must land first, so the real `EkuError`/`CaUsedAsEndEntity` surfaces).

### Pitfall 4: Debugging cross-host without the error chain (D-06, sequence first)
**What goes wrong:** `error.rs:63` Display=`"reqwest failure"`; `egress.rs:211` does `e.to_string()` discarding `#[source]`. Every TLS/connect/status fault logs identically → each of findings #5/#6/#8 cost a full two-machine round-trip to diagnose.
**How to avoid:** Land D-06 as the FIRST wave. Log `{e:?}` or walk `.source()`. The fix's own re-test (UAT-01) is far cheaper to debug with it in place.

### Pitfall 5: Keyring load-once / duplicate-pubkey brick (findings #3/#4)
**What goes wrong:** Gateway loads `peers.keyring` once at startup — no hot-reload (`main.rs:229`). Pinning must precede launch. Re-exporting under a corrected label leaves two lines with one pubkey → `Keyring::load_from_file` fails closed (`duplicate pubkey`). And `"ready"` prints (`main.rs:201`) *before* the keyring loads (`main.rs:229`) → false-success health signal.
**How to avoid (D-07 doc + D-04 sequencing):** Document pin-before-launch. Move the "ready" print to AFTER keyring load. Warn about stripping the stale export line before re-pinning.

## Code Examples

### Remote-target parse + split-addressing (D-01, primary edit site `send/mod.rs:413`)
```rust
// In build_envelope_value / target construction (illustrative — planner writes real code):
let (bus_target, env_to, remote) = match args.to.as_deref().map(str::parse::<Principal>) {
    Some(Ok(p)) => {
        // Remote: bus routes to local proxy by LEAF; envelope carries FULL principal.
        (Target::Agent { name: p.name().to_string() }, p.to_string(), true)   // leaf accessor per famp-core
    }
    _ => {
        // Local (unchanged): agent:local.bus/<name>, audit_log path (D-04).
        (Target::Agent { name: raw.clone() }, format!("agent:local.bus/{raw}"), false)
    }
};
let from = if remote {
    format!("agent:{own_domain}/{identity}")   // D-05: own_domain from config
} else {
    format!("agent:local.bus/{identity}")       // unchanged
};
```

### Typed request via sign-then-strip (D-03)
```rust
// Source: crates/famp-gateway/tests/e2e_cross_host_delivery.rs:406-436 (unsigned_value + build_request)
let env = UnsignedEnvelope::<RequestBody>::new(id, from, to, AuthorityScope::Advisory, ts, body);
let dummy = FampSigningKey::from_bytes([42u8; 32]);
let bytes = env.sign(&dummy)?.encode()?;
let mut v: serde_json::Value = serde_json::from_slice(&bytes)?;
v.as_object_mut().unwrap().remove("signature");   // BUS-11: unsigned on the bus
```

### Canonical cross-platform cert recipe (D-07 doc + D-08 fixtures)
```bash
# Source: 10-HUMAN-UAT.md "Canonical cert recipe" — verified against both verifiers in the 2026-07-28 dogfood
openssl req -x509 -newkey rsa:2048 -nodes -days 800 \
  -keyout <host>.key.pem -out <host>.cert.pem -subj "/CN=<host>" \
  -addext "subjectAltName=IP:<tailnet-ip>,DNS:<hostname>" \
  -addext "basicConstraints=critical,CA:FALSE" \
  -addext "keyUsage=critical,digitalSignature,keyEncipherment" \
  -addext "extendedKeyUsage=serverAuth"
```
For fixtures, use `127.0.0.1`/`localhost` SANs (the E2E binds loopback, `e2e:339`). The old `cargo run --example _gen_fixture_certs` generator referenced in the fixtures README is **gone** (not in `crates/famp/examples/`) — regenerate via this openssl recipe or a small `rcgen` example that sets `IsCa::NoCa` + `ExtendedKeyUsagePurpose::ServerAuth`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `famp send` hardcodes `agent:local.bus/<name>` + `audit_log` | Conditional remote path: domain-qualified `from`/`to` + typed `request` | This phase (D-01/D-03/D-04) | Shipping client can finally address a remote principal |
| "Self-signed is fine" cert guidance | CA:FALSE + serverAuth EKU (both verifiers) | This phase (D-07/D-08) | Certs work on macOS Apple SecTrust AND Linux webpki |
| Transport logs `"reqwest failure"` | Full `.source()` chain / `{e:?}` | This phase (D-06) | Cross-host faults become one-line diagnoses |
| DOC-04 flag-grep accuracy gate | + semantic checks (wiring direction, pin label, ordering, cert policy) | This phase (D-07) | Gate can catch inversions the grep can't |

**Deprecated/outdated:**
- `_gen_fixture_certs` example — removed; fixtures README is stale on both the generator AND the "Ed25519" claim (certs are actually ECDSA P-256, verified).
- GATEWAY-SETUP.md §3 `--as agent:hostA.example/gateway` label — wrong (finding #2); must be the sender agent principal.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Own-domain config key names (`FAMP_OWN_DOMAIN` / `$FAMP_HOME/own-domain` / `--domain`) | D-05 Resolution | Cosmetic — explicitly Claude's Discretion; planner picks + documents. No invariant risk. |
| A2 | `famp peer export` should derive its label authority from the own-domain source | D-05 Resolution | If rejected, coupling falls back to human discipline + gateway-side assertion (still safer than today, but not structurally enforced) |
| A3 | Regenerating fixtures + relying on the EXISTING macOS matrix leg (`ci.yml:105-118`) satisfies D-08's "add a macOS CI leg" | Environment / TEST-03 | If the cross-host E2E is somehow excluded from the macOS nextest run, a new explicit job is needed — planner must confirm the test executes on macos-latest (see Open Q2) |

**All other claims are code-traced (`[VERIFIED: codebase grep]`) or cross-checked against the dogfood UAT.**

## Open Questions

1. **Exact own-domain config surface (key names, precedence, error UX).**
   - Known: a shared host-level source is required (D-05 option a); `--domain` override + env + file is a natural shape.
   - Unclear: the precise names and whether `peer export` derives-vs-validates the label.
   - Recommendation: planner picks, documents in PLAN, and (per Ben's profile) states the pick with a one-line why rather than asking — unless the user wants to weigh in on the env/file name.

2. **Does the cross-host E2E (`e2e_cross_host_delivery.rs`) actually execute on the macos-latest matrix leg today, and is it currently green there?**
   - Known: it is NOT `#[ignore]`d (verified); `cargo nextest run --workspace --profile ci` runs on `[ubuntu-latest, macos-latest]` (`ci.yml:105-118`); the outbound client uses `rustls-platform-verifier` (Apple SecTrust on macOS).
   - Unclear: whether it's been passing on macOS with the current no-EKU ECDSA fixtures (it arguably should NOT, per finding #5) — possibly the `--trust-cert` extra-root path masks the EKU check, or the macOS leg has been silently red/non-blocking.
   - Recommendation: FIRST verification step of the D-08 plan — run the E2E on macOS (or inspect recent CI runs) BEFORE regenerating fixtures, to confirm the failure mode and prove the regen fixes it (falsification-with-a-control: current fixtures should fail on macOS, regenerated should pass).

3. **UAT-01 requires a terminal FSM state via the real `famp send`.** The grounded design report (B) notes `audit_log` won't drive the FSM, but D-03/D-04 upgrade remote sends to typed `request`, and the E2E proves `request→commit→deliver→ack` reaches `COMPLETED`. The open bit: does the shipping `famp send` need to drive the *full* cycle (commit/deliver/ack too), or does UAT-01 accept "request delivered + FSM advances past REQUESTED on both sides"? CONTEXT D-03 says "drives the FSM"; UAT-01 DoD says "task FSM reaching a terminal state."
   - Recommendation: confirm with the user whether UAT-01's terminal-state clause requires the receiver agent to actually reply (commit/deliver/ack) — that's a human-in-the-loop behavior at dogfood time, not a `famp send` code requirement. Likely: `famp send` must produce a *reply-able* typed `request`; the terminal state comes from the remote agent replying, exactly as the E2E's four legs simulate.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `openssl` CLI | D-07/D-08 cert generation | ✓ (macOS system) | LibreSSL/OpenSSL | `rcgen` example (in-tree dep) |
| `rustls-platform-verifier` | Cross-host TLS trust | ✓ | 0.5 | — (load-bearing; do not swap) |
| macOS + Linux CI runners | D-08 dual-platform gate | ✓ | `ubuntu-latest` + `macos-latest` matrix already present | — |
| `cargo-nextest` | CI test runner | ✓ | via `taiki-e/install-action` | plain `cargo test` (note: nextest `--list` hangs on `-p famp`, per Memory — use `cargo test --test e2e_cross_host_delivery` locally) |
| Two real machines (Tailscale) | UAT-01 live re-run | ✓ (opus laptop + zed `home-devbox`) | — | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None blocking.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo-nextest` (CI profile) |
| Config file | `.config/nextest.toml` (ci profile: `fail-fast=false`, 120s slow-timeout) |
| Quick run command | `cargo test -p famp send::` (unit: target-parse, from-stamp) |
| Full suite command | `just ci` (fmt-check, lint, build, test, spec-lint, all gates) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADDR-01 | `--to agent:d/n` → env `to` full principal, bus Target leaf | unit | `cargo test -p famp build_envelope_value` | ❌ Wave 0 (extend `send/mod.rs` tests) |
| ADDR-01 | `--to bob` unchanged (local, audit_log) | unit (regression) | existing `build_envelope_value_*` tests | ✅ `send/mod.rs:512-727` |
| ADDR-02/03 | remote send → typed `request`, `from=agent:{domain}/id` | unit | `cargo test -p famp` (new) | ❌ Wave 0 |
| ADDR-02 | shipping `famp send` drives cross-host delivery | integration (2-process) | `cargo test -p famp-gateway --test <new_shipping_e2e>` | ❌ Wave 0 (D-09) |
| ADDR-01 (neg) | `local.bus`-authority via federated path → typed error, not silent drop | integration | same new test file | ❌ Wave 0 (D-09) |
| OBS-01 | transport error surfaces `.source()` chain | unit | `cargo test -p famp-transport-http error` | ❌ Wave 0 |
| DOC-05 | guide flags match binary + semantic checks | doc-accuracy gate | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✅ extend (`10-03` gate exists) |
| TEST-03 | fixtures verify on macOS (Apple) + Linux (webpki) | integration | existing `e2e_cross_host_delivery` on both matrix legs | ✅ regen fixtures + confirm macOS |
| UAT-01 | live two-machine, real client | manual (human gate) | dogfood re-run | ✅ 10-HUMAN-UAT.md carries it |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched-crate>` + `just lint` (Rust-touching — per Memory, `just lint` promotes nursery lints; plain clippy is not enough).
- **Per wave merge:** `just ci` (full gate).
- **Phase gate:** `just ci` green on both OS legs before `/gsd-verify-work`; UAT-01 live re-run before v1.0.0 tag.

### Wave 0 Gaps
- [ ] Extend `crates/famp/src/cli/send/mod.rs` `#[cfg(test)]` — remote-target parse, `from`/`to` qualification, split bus-Target, own-domain resolution + missing-domain error.
- [ ] New `crates/famp-gateway/tests/<shipping_surface_e2e>.rs` — drives fixed `famp send` (not the raw bus client) cross-host; NEGATIVE `local.bus`-via-federation → typed error (D-09). Reuse the `Side`/`ChildGuard`/poll harness from `e2e_cross_host_delivery.rs`.
- [ ] `crates/famp-transport-http` unit test asserting the error Display includes the underlying source (OBS-01).
- [ ] Regenerate `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` to CA:FALSE+serverAuth (+update stale README).
- [ ] Extend the doc-accuracy gate with semantic assertions (wiring direction, pin-label = agent principal, ordering, cert policy present).

## Security Domain

INV-10 / Ed25519 signing is the trust boundary — this phase touches it directly (constructing the `from` the receiver's key lookup keys off).

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Ed25519 envelope signature (gateway egress); TOFU-pinned peer keys (`famp-keyring`) |
| V4 Access Control | yes | TRUST-02 hard-reject of unpinned sender (`verify.rs:42-44,63-65`) — no auto-pin, no fallback |
| V5 Input Validation | yes | `Principal` parse + `validate_authority`/`validate_name_or_instance_id` on `--to` and own-domain |
| V6 Cryptography | yes | `famp-crypto` `verify_strict`, RFC 8785 JCS, `FAMP-sig-v1\0` prefix — never hand-rolled |
| V9 Communications | yes | rustls 0.23; CA:FALSE+serverAuth certs (channel encryption; the real trust is the Ed25519 layer, not TLS) |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Forged/mismatched `from` (own-domain drift) → `UnpinnedKey` DoS-on-self | Spoofing / DoS | D-05 single-source own-domain enforces `from == pinned-label`; optional gateway pre-sign assertion |
| Gateway rewriting signed `to`/`from` (provenance lie) | Tampering / Repudiation | Rejected by design (C1/C4); sender authors both fields (Invariant A) |
| Post-sign field mutation | Tampering | All federation fields inserted BEFORE `sign_value` (`egress.rs:104-132`); verify_strict rejects mismatch |
| TLS cert that silently passes one platform | Spoofing | CA:FALSE+serverAuth recipe verified on both Apple SecTrust + webpki |
| Silent drop of `local.bus`-authority envelope on federated path | Repudiation / observability gap | D-09 negative test: must yield a typed error, not a black hole |

## Project Constraints (from CLAUDE.md)
- Rust stable, latest; `ed25519-dalek`, `serde` + custom JCS canonicalizer, `axum`/`hyper` transport — all already in-tree; no new deps.
- Every message signed at the federation boundary (INV-10); unsigned cross-host rejected. Domain-separation prefix `FAMP-sig-v1\0`.
- Spec authority **v0.5.2**; document any diff with reviewer rationale.
- **MCP surface changes → run `just install` before closing the PR** (installed `~/.cargo/bin/famp` is what agents read; `target/release/famp` is not the deploy target). The D-01 chokepoint touches the `famp_send` MCP path → `just install` required.
- Protocol-primitive crates (`famp-canonical/crypto/core/fsm/envelope`) are transport-neutral — this phase edits CLI (`famp send`), gateway, and transport-http, NOT the primitives.
- From project memory: `.planning/` is gitignored → run executors NON-isolated on main (sequential); Rust-touching executors run `just lint` (not plain clippy) + `just ci`; `just install` + binary-freshness before docs-verify/UAT.

## Sources

### Primary (HIGH confidence — code-traced this session)
- `crates/famp/src/cli/send/mod.rs` (`build_envelope_value` L413-509, `local.bus` hardcode L425, `--as`/`act_as` L92-99) — primary edit site
- `crates/famp-gateway/src/verify.rs` (L41,58-66) — ingress verifies on `from`; proves D-02
- `crates/famp-gateway/src/egress.rs` (`sign_federation_fields` L89, `relay_one` L191, `e.to_string()` L211) — D-06 + D-05 option-b analysis
- `crates/famp-gateway/src/main.rs` (peer map cross-product L255-261, keyring load L229, ready print L201) — findings #4, D-05
- `crates/famp-transport-http/src/error.rs` (L63 `"reqwest failure"`) + `Cargo.toml:28` (`rustls-platform-verifier 0.5`) — D-06, Pitfall 3
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (ground-truth shape: `unsigned_value` L406, `build_request` L421, `send_bus_envelope` L519, domain-qualified ALICE/BOB L86-92) — D-03, split-addressing
- `crates/famp-core/src/identity.rs` (`validate_authority` L212, `validate_name_or_instance_id` L245-263) — `--as` charset (why D-05 needs a separate source)
- `crates/famp/src/cli/peer/export.rs` (L36-60) — pin-label = full Principal; the coupling target
- `.github/workflows/ci.yml` (L52-57 build matrix, L105-118 test matrix with macos-latest) — D-08 macOS leg already exists
- `.config/nextest.toml`, `Justfile` (L42 lint, L218 ci) — validation gates
- `openssl x509 -text` on `crates/famp/tests/fixtures/cross_machine/alice.crt` — confirmed ECDSA P-256, no EKU (D-08)
- `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` — the 8 findings + canonical cert recipe (dogfood ground truth)

### Secondary (MEDIUM confidence)
- `.planning/phases/11.../DESIGN-RESEARCH-B-grounded.md` — grounded C2/C5 design pass + invariants A–E (trustworthy; code sketch not relied upon)
- `docs/GATEWAY-SETUP.md` — the file to correct (its §3/§4 are the wrong-baseline being fixed)

### Cross-checked (training + empirical)
- `rustls-platform-verifier` → Apple SecTrust (macOS) / webpki (Linux) EKU+CA divergence — training knowledge, CONFIRMED empirically by dogfood findings #5/#8 and the `Cargo.toml` dep.

## Metadata

**Confidence breakdown:**
- Standard stack (internal crates, no installs): HIGH — all present + tested in-tree
- Architecture / split-addressing: HIGH — traced through E2E ground truth + all four gateway/transport files
- D-05 coupling analysis + option ranking: HIGH — three-process-agreement argument is code-grounded; exact config names MEDIUM (Claude's Discretion)
- Cert cross-platform behavior: HIGH — dep + dogfood + openssl inspection converge
- macOS CI leg sufficiency: MEDIUM — leg exists; whether the E2E currently runs/passes there is Open Q2 (first plan verification step)

**Research date:** 2026-07-28
**Valid until:** 2026-08-27 (30 days — internal codebase, stable; re-verify if `send/mod.rs` or the gateway egress/verify path changes before planning)
