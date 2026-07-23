# FAMP-Sec v1 — Security Plane Specification (DRAFT 0.2)

Status: draft for review, round 2. Incorporates review round 1: chain circularity fixed (B1), retargeted to open internet (B2), implicit peering closed (B3), directory retrieval + transport + pre-verification throttling added (G1–G3), approval delivery corrected and audience-bound (D1, D4), allowlist-endorsement worked example (D2), offline delivery made explicit (D3). Open calls remain in §13.

Conventions: MUST / MUST NOT / SHOULD / MAY per RFC 2119. All hashes are SHA-256. The only signature algorithm is Ed25519. There is no algorithm negotiation anywhere in this specification.

---

## 1. Prerequisites and scope

**PRE-1 (Layer binding).** FAMP-Sec is a Layer 2 (federation gateway) specification. Layer 1 (same-host UDS bus) remains unsigned and out of scope. The gateway is the sole enforcement point; a message that has passed the gateway is, from Layer 1's perspective, local traffic. This spec does not apply to, and imposes no requirements on, the local path.

**PRE-2 (Protocol-grade gateway).** FAMP-Sec presupposes the protocol-grade gateway: Ed25519 verification at ingress for every remote message. A chat-grade gateway (tailnet-trusting, unverified re-injection into the local path) is non-conformant with this spec by definition. This closes the previously tabled v1.0 gateway fork.

**PRE-3 (Reference implementation).** The reference implementation is a Rust library (`famp-sec`) with a thin sidecar wrapper. The library is the conformance target; the sidecar is a packaging convenience and adds no semantics.

**Security model.** Every remote message is authenticated but adversarial data. A remote message may propose actions and present evidence; it never confers authority, modifies receiver policy, or determines privileged control flow. The boundary enforced by this spec is between content and authority, between proposals and admitted operations, and between model reasoning and deterministic enforcement. No LLM evaluates any authorization predicate defined here.

---

## 2. What this spec does not enforce (deployment obligations)

A conforming gateway guarantees the invariants in §11 **for operations that pass through it**. It cannot guarantee:

- **DEP-1 — No alternate tool routes.** If the host grants the model a shell, filesystem, or network access outside the gateway (e.g. Claude Code), complete mediation is a deployment property, not a protocol property. Deployments MUST route all consequential operations through the gateway; the spec cannot verify that they do.
- **DEP-2 — Custodian isolation.** Credential custody (§8, stage E) assumes the custodian process is not model-steerable. Process isolation is the deployment's job.
- **DEP-3 — Model confidentiality.** The spec constrains what flows *out* through the gateway. It does not prevent confidential data already in a model's context from leaking through channels the gateway never sees (logs, telemetry, other hosts on the bus).
- **DEP-4 — Approval surface integrity.** §9 defines what an approval binds to; it cannot guarantee the approval device itself is uncompromised.

A conformance claim MUST state which DEP obligations the deployment satisfies and how.

---

## 3. Cryptographic conventions

### 3.1 Canonicalization

All signed and hashed JSON objects are canonicalized with JCS (RFC 8785) before the domain separator is prepended. Byte-exact canonicalization is the load-bearing property of this entire spec; there is exactly one canonicalizer.

### 3.2 Domain separators

Every signature and every content hash has a distinct 12-byte domain separator. Signing input is always `PREFIX ‖ JCS(object)` where `object` excludes its own `sig` field. Cross-purpose signature reuse is structurally impossible.

| Purpose | Prefix (12 bytes) | Operation |
|---|---|---|
| Envelope signature | `FAMP-sig-v1\0` | sign (unchanged from INV-10; existing interop vectors remain valid for the prefix, vectors regenerate for the new fields) |
| Capability signature | `FAMP-cap-v1\0` | sign |
| Invocation proof | `FAMP-inv-v1\0` | sign |
| Approval signature | `FAMP-apr-v1\0` | sign |
| Execution receipt | `FAMP-rcp-v1\0` | sign |
| Peer directory | `FAMP-dir-v1\0` | sign |
| Canonical operation hash | `FAMP-oph-v1\0` | hash |
| MCP descriptor hash | `FAMP-mcp-v1\0` | hash |

### 3.3 Canonical operation hash

The unit that capabilities scope to, invocations bind to, approvals approve, and receipts attest. For a proposed action `A`:

```
op_hash(A) = SHA-256( "FAMP-oph-v1\0" ‖ JCS({
  "op_id":          A.op.op_id,
  "schema_hash":    A.op.schema_hash,
  "args":           A.args,
  "idempotency_key": A.idempotency_key   // omitted if absent
}) )
```

Any material change to the operation — including a single argument byte — changes `op_hash` and invalidates every invocation proof and approval bound to it.

---

## 4. Signed message envelope

### 4.0 Transport

Transport MUST provide TLS 1.3 confidentiality and integrity. TLS is explicitly outside the trust model: every authorization decision derives from the Ed25519 signatures defined in this spec, so a compromised CA, hijacked DNS name, or terminated TLS session can deny service but confers no authority and reveals no more than the (already envelope-signed) traffic it carried. TLS here is defense in depth — wire confidentiality and discovery integrity — not a trust anchor.

### 4.1 Structure

```jsonc
{
  "famp_sec": "1.0",
  "message_id": "<UUIDv7>",
  "conversation_id": "<UUIDv7>",
  "parent_message_id": "<UUIDv7>",        // OPTIONAL
  "task_id": "<string>",                  // OPTIONAL
  "sender":   { "domain": "<domain-id>", "key_id": "<key-id>" },
  "receiver": { "domain": "<domain-id>", "key_id": "<key-id>" },
  "issued_at":  "<RFC 3339 UTC, ms precision>",
  "expires_at": "<RFC 3339 UTC, ms precision>",
  "nonce": "<base64url, 16 bytes>",
  "body_schema_id": "famp-sec-body/1.0",
  "body_hash": "<hex SHA-256 of JCS(body)>",
  "artifact_hashes": [                    // OPTIONAL, MAY be empty; omit if empty
    { "artifact_id": "<string>", "hash": "<hex SHA-256>", "media_type": "<string>", "bytes": <int> }
  ],
  "directory_serial": <uint64>,           // sender's own domain directory serial (hint; see §10.4)
  "sig": { "key_id": "<key-id>", "signature": "<base64url Ed25519>" }
}
```

Signature: `Ed25519-sign( "FAMP-sig-v1\0" ‖ JCS(envelope minus "sig") )`, signed by `sender.key_id`.

The body travels as a sibling field `body` in the wire message `{ "envelope": ..., "body": ... }` and is bound by `body_hash`; it is not covered directly by the envelope signature. Artifacts (files, blobs) travel out of band or as attachments and are bound by `artifact_hashes`. `[CALL]` — inline body rather than fully detached, for single-request transport simplicity. Flip if you want streaming-sized bodies in v1.

### 4.2 Validation rules (normative, ordered)

An envelope is valid iff ALL of the following hold. Evaluation is fail-closed; the first failure terminates processing.

1. `famp_sec` is exactly `"1.0"`. Unknown versions → reject.
2. The object contains every required field, any subset of the defined OPTIONAL fields, and nothing else. Unknown fields → reject. Missing required fields → reject. (No Postel.)
3. `receiver.domain` and `receiver.key_id` identify this gateway, AND `sender.domain` is enrolled in the local trust bundle (§10.3). Wrong audience → reject. Unenrolled sender domain → drop with **no state created**: no directory fetch, no cache entry, no pin (no implicit peering, §10.3).
4. `sender.key_id` resolves through the trust process of §10 to an active, non-revoked key in a trusted domain roster.
5. `sig.key_id == sender.key_id` and the signature verifies over the canonical bytes.
6. Freshness: `issued_at ≤ now + skew` and `now < expires_at` and `expires_at − issued_at ≤ MAX_ENVELOPE_TTL`. Defaults: `skew = 120 s`, `MAX_ENVELOPE_TTL = 300 s`, both locally configurable, never negotiable on the wire. (Skew widened from 30 s: open-internet peers without NTP discipline drift by minutes, and 30 s produces mystery rejections.) On a freshness denial the gateway records its own clock reading in the local audit record as a diagnostic; the wire response stays uniform per INV-S13. **v1 assumes online delivery**: there is no store-and-forward, and a receiver offline longer than `MAX_ENVELOPE_TTL` misses the message — the sender retries with a fresh envelope. Raising the TTL is legal local config; replay-cache retention scales proportionally.
7. Replay: `(sender.key_id, nonce)` and `message_id` are each absent from the replay cache. Cache retention MUST cover `MAX_ENVELOPE_TTL + skew`. Present → reject.
8. `body_schema_id` is known to this gateway. Unknown schema → reject.
9. `SHA-256(JCS(body)) == body_hash`.
10. Every referenced artifact's bytes hash to its declared `artifact_hashes` entry before any component may read it.

No LLM is invoked before or during envelope validation.

---

## 5. Message body

### 5.1 Normative sections

```jsonc
{
  "proposed_actions": [ <ProposedAction> ],   // MAY be empty; omit if empty
  "capability_proofs": [ <Capability> ],
  "provenance": <Provenance>,
  "approvals": [ <Approval> ],
  "content": { ... }                          // free-form payload; see 5.3
}
```

Exactly these top-level fields are permitted. Unknown top-level fields → reject the message. `assertions` / `evidence` taxonomies are NOT part of the core spec; they may be defined later as an optional profile carried inside `content`.

### 5.2 ProposedAction

```jsonc
{
  "action_id": "<string, unique in message>",
  "op": {
    "type": "abstract",
    "op_id": "<string, e.g. 'email.send'>",   // no wildcards anywhere in v1
    "schema_hash": "<hex SHA-256>"            // per §7
  },
  "args": { ... },                            // MUST validate against the pinned schema
  "idempotency_key": "<string>",              // OPTIONAL
  "capability_ref": "<hex SHA-256 of the Capability object>",   // must match an entry in capability_proofs
  "invocation_proof": <InvocationProof>,      // §6.3
  "approval_ref": "<single_use_id>"           // OPTIONAL; must match an entry in approvals
}
```

A ProposedAction carries no authority. It is a typed request that the gateway may admit or deny. The receiver's model MUST NOT extract operations from prose; only `proposed_actions` entries are candidates for execution.

### 5.3 content and provenance

`content` is arbitrary JSON payload for the receiving agent to interpret. The `provenance` section labels it:

```jsonc
{
  "labels": [
    { "path": "<JSON pointer into content>", "integrity": "<label>", "origin": "<string>" }
  ],
  "arg_derivations": [
    { "action_id": "<string>", "arg": "<name>", "sources": ["<JSON pointer | 'sender_asserted'>"] }
  ]
}
```

Sender-supplied labels are **claims**, not facts. Their treatment at ingress is governed by §6.5 (cross-domain downgrade). `arg_derivations` lets an honest sender declare which content influenced which proposed argument; a dishonest sender's declarations are harmless because the downgrade rule taints everything cross-domain regardless.

---

## 6. Capabilities and invocation binding

### 6.1 Capability object (FAMP-native v1 encoding)

Biscuit is NOT the v1 wire encoding. Capability semantics are expressed in FAMP's own JCS + Ed25519 form — one canonicalizer, one signed serialization. A Biscuit encoding profile MAY be added when attenuation chains ship (v1.1+); it MUST preserve these semantics exactly.

```jsonc
{
  "cap_version": 1,
  "cap_id": "<UUIDv7>",
  "issuer": { "domain": "<domain-id>", "key_id": "<key-id>" },
  "holder_key": "<base64url Ed25519 public key>",     // proof-of-possession key, NOT bearer
  "audience": { "domain": "<domain-id>", "key_id": "<key-id> | 'any'" },
  "task_id": "<string>",                              // OPTIONAL binding
  "conversation_id": "<UUIDv7>",                      // OPTIONAL binding
  "op_scope": [
    { "op_id": "<exact string>", "schema_hash": "<hex SHA-256>" }
  ],
  "constraints": {                                    // all OPTIONAL, all enforced deterministically
    "arg_allow":   { "<arg>": ["<literal>", ...] },   // exact-match allowlists
    "arg_max":     { "<arg>": <number> },             // numeric ceilings
    "max_uses":    <uint>,
    "rate":        { "per_seconds": <uint>, "max": <uint> }
  },
  "not_before": "<RFC 3339>",
  "expires_at": "<RFC 3339>",
  "chain": [],                                        // MUST be empty in v1; §6.2
  "approval_required": <bool>,                        // hint; local policy may require approval regardless
  "sig": { "key_id": "<key-id>", "signature": "<base64url>" }
}
```

Signature: `Ed25519-sign( "FAMP-cap-v1\0" ‖ JCS(capability minus "sig") )` by `issuer.key_id`.

Constraint vocabulary is closed. A capability containing any constraint key not defined above MUST be rejected (unknown predicate → deny, never ignore).

`[CALL]` — `op_scope` is exact `op_id` strings only. No wildcards, no prefixes, no patterns in v1. Wildcard-scope exploitation was in the threat model; the mitigation is to not have wildcards.

### 6.2 Chain field (attenuation-shaped, single-hop enforced)

`chain` is a list of **parent**-capability hashes, root first. In v1 it MUST be empty — a v1 capability is always a root grant — and verifiers MUST reject `len(chain) != 0`; reject, not ignore. (Draft 0.1 defined the sole entry as the self-hash, which is circular: an object's hash cannot appear inside its own preimage. Nobody can implement that; nobody should try.) v1.1 attenuation appends parent hashes and relaxes the check to per-link monotonic narrowing — a verifier change, not a wire change. The `cap_hash` used in invocation proofs (§6.3) is computed over the capability minus `sig` and is unaffected.

### 6.3 Invocation proof

A capability alone authorizes nothing. Each ProposedAction carries a fresh proof of possession of `holder_key`, bound to the exact operation:

```jsonc
{
  "op_hash": "<hex, per §3.3>",
  "cap_hash": "<hex SHA-256 of the Capability minus sig>",
  "nonce": "<base64url, 16 bytes>",
  "expires_at": "<RFC 3339>",
  "sig": "<base64url Ed25519 by holder_key over 'FAMP-inv-v1\0' ‖ JCS(proof minus sig)>"
}
```

Validation: signature verifies against the capability's `holder_key`; `op_hash` recomputes from the ProposedAction; `cap_hash` matches `capability_ref`; `now < expires_at` with `expires_at − now ≤ MAX_INVOCATION_TTL` (default 120 s); `(holder_key, nonce)` absent from the invocation replay cache. Binding `cap_hash` into the proof prevents a valid invocation from being replayed against a different (broader) capability for the same operation.

This kills the bearer weakness: a stolen capability is useless without the holder's private key, and a stolen invocation proof authorizes exactly one operation once.

### 6.4 Integrity labels (v1 vocabulary, closed)

Two-tier, per the PACT-lite decision:

- `high_integrity` — reachable ONLY by: verified local user input; output of a registered deterministic parser/validator (endorsement, §6.6); a valid human approval (§9).
- `tainted` — everything else: all remote content, all tool output, all model-generated text. Paraphrase and summarization do not launder taint; derived values inherit it.

### 6.5 Cross-domain downgrade rule (normative)

Provenance labels are trusted only within a trust domain. At cross-domain ingress, every value in `content` and every sender-asserted label is downgraded to `tainted`, regardless of the sender's authenticated identity and regardless of what the sender claimed. Re-elevation to `high_integrity` happens only via local endorsement (§6.6). A remote peer asserting `high_integrity` is an impersonation attempt surface, not information.

### 6.6 Endorsement

The only v1 endorsement mechanisms are: (a) a registered deterministic parser whose code identity (hash) is pinned in local gateway config, applied to a tainted value and producing a validated typed value; (b) a valid exact-operation human approval. A model's judgment that a value "looks safe" is not endorsement and has no representation in this spec.

**Worked example — the usability valve.** Follow the chain §6.5 → §7.1 literally and every remote-sourced `authority_destination` requires a human tap: every email recipient, every path, every amount, forever. That is the correct default and an unusable steady state. The escape is mechanism (a): register a parser, e.g. `recipient_allowlist` (hash-pinned in gateway config), that checks a tainted recipient string against a local allowlist and, on exact match, emits a `high_integrity` typed recipient. Result: mail to known recipients flows with no approval; mail to unknown recipients still requires a tap — "approve every email" becomes "approve only new recipients." The same pattern covers paths under an approved prefix, amounts under a standing ceiling, and any other value checkable against local ground truth. Allowlists are local config; by INV-S1 remote content cannot grow them — adding an entry is itself a local high-integrity act (operator edit or approved operation).

---

## 7. Abstract operations and the MCP binding profile

### 7.1 Abstract operations

An operation is `(op_id, schema_hash)`. `op_id` is a receiver-meaningful string; `schema_hash` pins the exact argument schema. The receiver maintains a local operation registry mapping `(op_id, schema_hash)` → executor. An action whose `(op_id, schema_hash)` is not in the registry is denied — including the case where `op_id` is known but the hash differs (schema drift → deny, never coerce).

Each registry entry also declares **argument roles** (closed vocabulary):

| Role | Examples | Provenance contract |
|---|---|---|
| `authority_destination` | email recipient, payee, upload target, hostname | `high_integrity` OR exact approval |
| `resource_selector` | file path, record ID, calendar ID | `high_integrity` OR exact approval |
| `amount` | money, quota deltas | `high_integrity` OR exact approval; also subject to `arg_max` |
| `executable` | code, shell strings, SQL | `high_integrity` OR exact approval (expect: nearly always approval) |
| `content_payload` | email body, document text, summary | `tainted` permitted |
| `neutral` | formatting flags, page size | `tainted` permitted |

This table is the entire v1 information-flow policy. Full IFC (confidentiality lattices, compartments, declassification, causal flows) is explicitly out of scope for v1 and reserved for a v2 profile.

### 7.2 MCP descriptor hash (normative profile)

MCP defines no canonical tool hash and servers mutate schemas silently. FAMP therefore computes its own:

```
mcp_schema_hash = SHA-256( "FAMP-mcp-v1\0" ‖ JCS({
  "name":         tool.name,
  "title":        tool.title,          // omitted if absent
  "description":  tool.description,    // omitted if absent
  "inputSchema":  tool.inputSchema,
  "outputSchema": tool.outputSchema,   // omitted if absent
  "annotations":  tool.annotations     // omitted if absent
}) )
```

The description IS included deliberately: tool descriptions are the classic MCP poisoning vector, and pinning them means a silently mutated description (rug pull) changes the hash and every capability scoped to it stops matching — deny. MCP's versioning instability becomes a tripwire instead of a hole. Binding an MCP tool to FAMP = register `(op_id := "mcp:" + server_id + ":" + tool.name, schema_hash := mcp_schema_hash)` plus hand-authored argument roles. Roles are never inferred from the descriptor (the descriptor is attacker-influencable; roles are local policy).

---

## 8. Gateway admission algorithm (normative)

The gateway is deterministic. No stage invokes an LLM. Processing is fail-closed: any check failing at any stage terminates with a denial. Stages are ordered cheap→expensive to bound resource abuse from garbage traffic.

```
STAGE A — THROTTLE + PARSE (no crypto yet)
  A1. Per-source (network address) rate limit. Exceeded → drop, no state.
  A2. Enforce transport size cap (config; default 1 MiB envelope+body).
  A3. Parse JSON. Malformed → deny.
  A4. Envelope rules §4.2 items 1–3 (version, strict fields, audience,
      sender-domain enrollment). Unenrolled domain → drop, no state.
  A5. Per-claimed-sender-key rate limit, applied BEFORE signature verify to
      bound Ed25519 verify cost (directories are public; key_ids are
      harvestable). Caveat: the key_id is unverified here, so this limit
      MUST throttle (delay/shed), never penalize or blacklist the key —
      otherwise a third party spoofing a victim's key_id can starve the
      legitimate holder. Reputation consequences attach only post-B2.

STAGE B — AUTHENTICATE
  B1. Resolve sender key via §10 (roster lookup, serial check, revocation).
  B2. Verify envelope signature (§4.2 items 4–5).
  B3. Freshness + replay (§4.2 items 6–7).
  B4. Body schema known; body hash matches; artifact hashes match (§4.2 items 8–10).

STAGE C — LABEL
  C1. Apply cross-domain downgrade (§6.5): all body content → tainted.
  C2. Record sender arg_derivations as advisory metadata (audit only).

STAGE D — PER-ACTION ADMISSION (for each ProposedAction, independently)
  D1. Schema: (op_id, schema_hash) in local registry; args validate against
      pinned schema; unknown arg fields → deny this action.
  D2. Capability resolution: capability_ref matches an entry in
      capability_proofs; capability signature verifies; issuer is authorized
      by local policy to issue for this op (§10.3); chain length == 0;
      audience matches this gateway; not_before ≤ now < expires_at.
  D3. Invocation binding (§6.3): holder signature, op_hash recomputation,
      cap_hash match, freshness, nonce replay check.
  D4. Scope: (op_id, schema_hash) ∈ capability.op_scope.
  D5. Constraints: every arg_allow / arg_max satisfied; max_uses and rate
      counters (persistent, per cap_id) not exceeded.
  D6. Provenance contracts: for each argument, role contract (§7.1) satisfied
      by the argument's label. Cross-domain, this means every
      authority_destination / resource_selector / amount / executable
      argument requires an approval (D7) or a local high_integrity source —
      by construction, remote values alone can never fill them.
  D7. Approval: if capability.approval_required, or local policy requires it,
      or D6 requires it — a valid Approval (§9) must be present whose op_hash
      equals this action's op_hash. Absent/invalid/consumed → deny.
  D8. Local policy hook: deterministic receiver policy (allow/deny/require-
      approval) evaluated over (sender, op_id, args, labels, counters).
      Policy version recorded.
  D9. Precondition hook: optional deterministic environment checks
      (balance sufficient, path exists, quota available).

STAGE E — EXECUTE (admitted actions only)
  E1. Hand (op_id, schema_hash, args) to the credential custodian.
      The custodian holds tool credentials; the agent process and model
      never receive them. Custodian executes with minimum credential.
  E2. Consume: mark approval single_use_id consumed; increment use/rate
      counters; add invocation nonce to replay cache. (Consumption is
      atomic with execution admission — a denied action consumes nothing.)

STAGE F — RECEIPT + AUDIT
  F1. Receipt (§9.2) signed by the gateway over what was actually executed.
  F2. Append-only audit record: raw envelope bytes, decision per action per
      stage, capability + approval hashes, policy version, executor result
      hash. Model summaries are never the audit record.

DENIAL SEMANTICS
  Externally: a single uniform denial code per action ("denied"), no stage
  detail, no timing-differentiated fast paths within Stage D (evaluate all
  of D for an action even after first failure, or pad — [CALL: I spec
  "evaluate-all-then-deny" for uniformity; costs cycles, buys silence).
  Internally: full per-stage detail to audit only.
  Rate-limit drops (A1/A5) and unenrolled-domain drops (A4) predate any
  authorization decision and leak nothing about one; they do not violate
  INV-S13.
```

The `ALLOW` predicate is the conjunction of A–D. The model's role ends at producing ProposedActions; it has no vote in A–F.

---

## 9. Approvals and receipts

### 9.1 Approval object

```jsonc
{
  "op_hash": "<hex, §3.3>",
  "approver": { "domain": "<domain-id>", "key_id": "<key-id>" },
  "audience": { "domain": "<domain-id>", "key_id": "<key-id>" },   // the gateway this approval is redeemable at
  "render_template_hash": "<hex SHA-256>",
  "issued_at": "<RFC 3339>",
  "expires_at": "<RFC 3339>",              // RECOMMENDED ≤ 10 min
  "single_use_id": "<base64url, 16 bytes>",
  "sig": { "key_id": "...", "signature": "<over 'FAMP-apr-v1\0' ‖ JCS(minus sig)>" }
}
```

Rules: approver key MUST be registered in local config as an approval key (approval keys are distinct from agent keys); `audience` MUST identify the redeeming gateway — `op_hash` binds no receiver, so without an audience an approval harvested at one gateway could be redeemed at any other that honors the same approver key; `single_use_id` is consumed atomically at execution; any change to the operation changes `op_hash` and orphans the approval. Delivery channel is irrelevant by construction: validity derives entirely from a locally-registered approval key's signature over the operation hash and audience, plus single-use consumption. An approval MAY therefore be carried in the requesting agent's own `body.approvals` — the relaying agent can withhold it (fail-closed, harmless) but cannot forge, alter, redirect, or replay it.

### 9.2 Canonical rendering (anti-blind-signing)

For every registry entry, local config registers a rendering template: a deterministic function `render(op_id, schema_hash, args) → text` whose definition is hashed (`render_template_hash`). The approval surface MUST derive its display exclusively from the canonical operation bytes via the registered template — no sender-supplied display strings, ever. The approval embeds `render_template_hash`, making the audit record show not just what was approved but what the human was shown. Template mismatch at verification → deny.

`[CALL]` — v1 template language: none. A template is code shipped with the approval surface, identified by hash. A declarative template DSL is a v1.1 question; inventing one now is scope creep.

### 9.3 Execution receipt

```jsonc
{
  "receipt_id": "<UUIDv7>",
  "message_id": "<UUIDv7>",
  "action_id": "<string>",
  "op_hash": "<hex>",
  "decision": "executed | denied",
  "result_hash": "<hex SHA-256 of canonical result>",   // omitted when denied
  "executed_at": "<RFC 3339>",
  "policy_version": "<string>",
  "sig": { "key_id": "<gateway key>", "signature": "<over 'FAMP-rcp-v1\0' ‖ ...>" }
}
```

The executor, not the requester, produces the authoritative record. Receipts return to the sender and persist in local audit. Transparency-log anchoring: deferred, field layout already compatible (hash-chainable by receipt_id order).

---

## 10. Identity, peer directory, revocation

### 10.1 Identity model

Bare Ed25519 keys, organized per trust domain. A domain is: a root key + a signed roster. The target environment is the open internet — peers reachable across the public network; nothing about a shared LAN or mesh VPN is assumed.

Web PKI and DNS are used for **publication only** (§10.5), never in the trust path: all authority derives from the `FAMP-dir-v1\0` roster signature under the pinned domain root key, so a compromised CA or hijacked DNS record can deny service but confers nothing. No DIDs. No JWKS — JWKS would reintroduce JOSE key representations and their algorithm-agility surface immediately after §3 removed negotiation from the protocol.

Note: a published roster enumerates a domain's agents. Acceptable for most deployments, stated here so it surprises no one; a deployment that objects can serve its directory behind authenticated fetch restricted to enrolled peers.

### 10.2 Peer directory

```jsonc
{
  "domain": "<domain-id>",
  "serial": <uint64, strictly monotonic>,
  "issued_at": "<RFC 3339>",
  "root_key_id": "<key-id>",
  "peers": [
    { "key_id": "<key-id>", "pubkey": "<base64url>", "agent_id": "<string>",
      "roles": ["agent" | "gateway" | "approver" | "issuer"], "added_at": "<RFC 3339>" }
  ],
  "sig": { "key_id": "<root key>", "signature": "<over 'FAMP-dir-v1\0' ‖ JCS(minus sig)>" }
}
```

Verifier rules: cache the highest serial seen per domain; MUST reject any directory with `serial ≤ cached` (equal included — re-presentation of the current directory is a no-op, not an update; `[CALL]` strictly-greater to make rollback detection unambiguous); a key absent from the current roster is revoked — immediately, no TTL wait. This is the revocation mechanism: compromise → publish serial+1 without the key → every conformant verifier rejects it on next directory fetch. Directory freshness: verifiers SHOULD refetch at a configured interval (default 5 min) and MUST refetch before admitting any action gated on a key first seen since the last fetch.

### 10.3 Cross-domain trust

A trust bundle is local config pinning `{domain-id → root pubkey, allowed_roles, allowed_op_ids}` — trusting a domain is scoped, not total: the bundle states which ops that domain's issuers may issue capabilities for.

**No implicit peering (normative).** Inbound traffic never enrolls a domain. A message from a domain absent from the trust bundle is dropped at stage A with no state created — no directory fetch, no cache entry, no pin. Enrollment is operator-initiated only, via the out-of-band peer exchange flow (`peer_export`/`peer_import`). TOFU, where used, means: the operator enrolls a domain by identifier without pre-supplying its root key, and the key is pinned on first *outbound connection to* that already-enrolled domain (v0.8 precedent); a TOFU-pinned domain starts restricted to a configured minimal op set until explicitly promoted. Root key change on any pinned domain → hard fail + operator alert, never silent re-pin.

### 10.4 directory_serial hint

The envelope's `directory_serial` is the sender's claim of its own current roster serial. If it exceeds the receiver's cached serial, the receiver SHOULD refetch the directory before proceeding (cheap freshness signal). It is a hint; authorization never depends on it. A `directory_serial` hint exceeding the cached serial takes precedence over a negative-cache hit: the refetch trigger fires, and on success the higher-serial roster supersedes the stale negative entry (§10.5). The hint is attacker-settable, so it may only *trigger* a rate-limited refetch — never directly admit a key or bump the cached serial without a verified directory.

### 10.5 Directory retrieval

The directory is published at `https://<domain-host>/.well-known/famp-directory.json` and fetched over HTTPS (TLS 1.3, §4.0). TLS provides discovery integrity and confidentiality only; trust derives from the roster signature (§10.1). Retrieval rules:

- **Fail closed.** When a refetch is required (unknown `key_id` under an enrolled domain, or `directory_serial` hint exceeding cache) and the fetch fails, every action gated on the missing information is denied. A stale-but-valid cached roster remains usable, within the refresh interval, for keys it already contains.
- **Per-domain fetch rate limit** (default 1 fetch / 30 s / domain, config). Unknown key_ids under an enrolled domain are an amplification vector: an attacker who knows a trusted domain name can mint fresh key_ids to force a refetch per message. The rate limit caps this at the configured fetch frequency regardless of message volume.
- **Negative caching (serial-scoped).** A key_id fetched-for and absent from the roster is cached as unknown, keyed to `(domain, key_id, roster_serial)` where `roster_serial` is the serial of the directory that was checked. Repeat presentations are denied from cache with no fetch **only while the cached serial is the highest the verifier has seen for that domain**. A successful fetch at a higher serial, or a verified higher serial observed by any other path, supersedes and clears negative entries for that domain — a key legitimately added in a later roster is never shadowed by an earlier "unknown" verdict. Absent a serial advance, the entry holds for at least one refresh interval. This bounds the amplification vector (a probed key_id costs at most one fetch per refresh interval) without letting a pre-probed key_id blackout a genuinely new agent past the next roster publication.
- Fetch failures and negative-cache hits are audit events; the wire response remains uniform.

---

## 11. Conformance invariants

A conforming FAMP-Sec gateway enforces, and its test vectors demonstrate:

- **INV-S1.** No remote content modifies gateway policy, the operation registry, argument roles, rendering templates, or trust bundles.
- **INV-S2.** No remote content creates or expands authority; capabilities only ever narrow.
- **INV-S3.** No LLM output is an input to any authorization predicate (stages A–D).
- **INV-S4.** Every consequential operation passes stages A–F; there is no second entry point in the gateway. (Routes outside the gateway: DEP-1.)
- **INV-S5.** `len(chain) == 0` in v1; violation → reject.
- **INV-S6.** Every capability is audience-bound and time-bounded; every invocation is holder-bound, operation-bound, and single-use.
- **INV-S7.** Every authority_destination / resource_selector / amount / executable argument was sourced from local high_integrity provenance or an exact-operation approval — never from remote content alone.
- **INV-S8.** All cross-domain content enters as tainted; only §6.6 endorsement elevates.
- **INV-S9.** Unknown fields, unknown constraint keys, unknown schemas, unknown versions, chain anomalies → deny. Nothing unknown is ignored.
- **INV-S10.** One canonicalizer (JCS), one signature algorithm (Ed25519), one hash (SHA-256), zero negotiation.
- **INV-S11.** Approvals bind to the canonical operation hash and the registered rendering template; consumed atomically; single-use.
- **INV-S12.** A key absent from the current highest-serial roster is dead immediately.
- **INV-S13.** External denial responses are uniform; denial detail exists only in local audit.
- **INV-S14.** The executor signs the authoritative receipt; requester-supplied logs are never the record.
- **INV-S15.** Persisted artifacts retain `{content_hash, origin, integrity_label, parent_hashes}` so stored remote content re-enters as tainted, not clean.
- **INV-S16.** Credentials live in the custodian; no model-reachable process holds a reusable tool credential.
- **INV-S17.** Inbound traffic never enrolls a domain, pins a key, or creates gateway state prior to enrollment. Peering is operator-initiated only.

---

## 12. Explicitly out of scope for v1

Multi-hop attenuation (wire-ready via chain field; verifier work in v1.1). Biscuit/UCAN encoding profiles. Full IFC: confidentiality labels, compartments, declassification policy. Causal-flow / denial-channel tracking beyond uniform denials. DualView-style dual rendering (INV-S15 metadata persistence only). Automated contract synthesis (roles and templates are hand-authored). Transparency-log anchoring. Template DSL for rendering. Store-and-forward / offline delivery — v1 assumes online receivers (§4.2 item 6); a mailbox profile is a v1.1+ question.

---

## 13. Open [CALL] register

| # | Section | Call made | Flip if |
|---|---|---|---|
| 1 | §4.1 | Body inline as sibling field, hash-bound | v1 needs streaming/large bodies |
| 2 | §6.1 | op_scope exact strings, no wildcards | scoping hundreds of ops per cap becomes real |
| 3 | §8 | Evaluate-all-then-deny in stage D for timing uniformity | measured cost is unacceptable |
| 4 | §9.2 | Rendering templates are hashed code, no DSL | multi-implementation approval surfaces arrive |
| 5 | §10.2 | Directory serial strictly greater (reject equal) | re-presentation churn is a problem in practice |
| 6 | §4.2/§6.3 | TTL defaults: envelope 300 s, invocation 120 s, skew 120 s; clock diagnostic in local audit only | field drift data says otherwise |
| 7 | §10.5 | Directory fetch limits: 1 / 30 s / domain; negative cache = one refresh interval | amplification math at real peer counts differs |
| 8 | §8 A5 | Pre-verify per-key limit throttles only, never blacklists (spoofable key_id) | measured verify-flood cost demands harsher shedding |
