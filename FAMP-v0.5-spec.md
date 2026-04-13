# FAMP — Federated Agent Messaging Protocol

## Core Semantic Specification

**Version 0.5.0-draft** · April 2026

---

## Abstract

FAMP defines semantics for communication among autonomous AI agents within a trusted federation. It specifies identity, causality, negotiation, commitment, delegation, and provenance. It does not specify transport, serialization, discovery infrastructure, or internal agent architecture.

### Normative scope

**Bilateral commitment.** FAMP v0.4 defines task-level commitment as a bilateral act between two agents: one initiator and one committer. Conversations may have multiple participants, but commitments are pairwise. Multi-party commitment aggregation (e.g., "task requires commitments from agents A, B, and C before proceeding") is out of scope for v0.4 and is deferred to a future profile or extension.

**Intra-federation.** FAMP v0.4 defines intra-federation semantics normatively. Inter-federation semantics are defined only at the identity-verification level (Section 5.4): an agent may verify a foreign principal's federation membership and reject or accept messages accordingly. Cross-federation delegation, policy inheritance, and provenance interoperability are not defined in v0.4.

**Interaction semantics, not application semantics.** FAMP standardizes how agents negotiate, commit, delegate, and report. It does not standardize what agents do — domain payloads, artifact formats, and application-level schemas are intentionally delegated to capability-level declarations (Section 6). Interoperability happens at the level of commitment, causality, delegation, and capability advertisement. Domain payload interoperability is a capability concern, not a protocol concern.

### Layer structure

The protocol is organized into three layers:

| Layer | Scope | Concern |
|-------|-------|---------|
| **L1 — Identity & Presence** | Who exists, what they can do, whether they're reachable | Identity, capability posture, federation membership |
| **L2 — Conversation & Causality** | How messages relate to each other | Message exchange, causal linkage, semantic acknowledgment |
| **L3 — Task & Commitment** | Who owes what to whom, under what bounds | Negotiation, commitment, delegation, provenance, terminality |

Each layer depends on the one below it. An agent may implement L1 alone (discoverable but passive), L1+L2 (conversational but non-committing), or all three (full participant).

---

## 1. Environment Assumptions

1. Participants are autonomous agents with independent goals, policies, and resource constraints.
2. Participants belong to a trusted federation. Trust establishes identity, not correctness, competence, or authorization.
3. Agents may refuse work, negotiate terms, counter-propose, or abandon negotiation before commitment.
4. Delegation is normal, not exceptional. The protocol must handle it structurally, not as a convention.
5. Message meaning derives from explicit causal references, not from arrival order or transcript position.
6. Intermediaries may exist, but protocol semantics must not depend on them.

What trusted federation does NOT mean:

- Shared memory or shared state
- Authority inheritance by default
- Stable capabilities
- Permission to re-delegate without authorization
- Tolerance for missing provenance

---

## 2. Design Objectives

### 2.1 Commitment clarity

At any point, an observer must be able to determine whether an agent has: received a message, understood it, entered negotiation, committed to work, delegated work, delivered work, or declined responsibility.

### 2.2 Bounded autonomy

Agents plan and reason locally. All externally visible commitments must be bounded and legible.

### 2.3 Delegation visibility

Recursive delegation is permitted. Silent subcontracting is a protocol violation.

### 2.4 Causal integrity

Message meaning derives from typed causal edges, not from sequence position.

### 2.5 Evolvability without semantic drift

Extensions may add vocabulary. They may not alter the meaning of core semantics.

---

## 3. First-Class Protocol Objects

FAMP defines seven first-class objects.

### 3.1 Agent

A protocol principal capable of sending, receiving, negotiating, committing, delegating, and acting. An agent has a principal identity (enduring) and one or more instance identities (per runtime incarnation).

### 3.2 Conversation

A causally linked set of messages between two or more agents. Conversations have identity, participants, and lifecycle, but no commitment semantics of their own. Conversations are the L2 container. Tasks are the L3 container.

### 3.3 Task

A bounded unit of requested work with explicit ownership, bounds, terminal conditions, and delegation rules. Tasks exist within conversations but have their own state machine.

### 3.4 Proposal

A candidate commitment surface offered during negotiation. A proposal binds nothing. Only a `commit` message creates obligation.

### 3.5 Commitment

A binding obligation created by a `commit` message, referencing a specific proposal. A commitment binds the sender to a specific scope, bounds, policies, delegation permissions, and reporting obligations.

### 3.6 Artifact

An immutable referenced object used as input or output. Artifact identifiers MUST resolve to immutable content. Content-addressed identifiers (SHA-256 hash) are RECOMMENDED.

### 3.7 Policy

A machine-readable constraint governing handling, execution, retention, disclosure, or delegation. Policies attach to tasks, not to messages.

---

## 4. Core Invariants

These are mandatory. An implementation that violates any of these is non-conformant.

### INV-1 Triple identity

Every message MUST identify: sender principal, sender instance, and message ID.

### INV-2 Explicit causality

Every non-initial message MUST declare a typed causal relation to at least one prior message or task event.

### INV-3 No implied commitment

Receipt does not imply acceptance. Negotiation does not imply commitment. Commitment does not imply delegation authority.

### INV-4 Bounded tasks

Every task MUST declare at least two explicit bounds from: deadline, budget, hop limit, policy domain, authority scope, maximum artifact size, confidence floor.

### INV-5 Single terminal state

A task MUST end in exactly one terminal state: `completed`, `rejected`, `failed`, `cancelled`, or `expired`.

### INV-6 Delegation visibility

If any nontrivial portion of a task is delegated, the existence of that delegation MUST be representable in protocol and MUST be preserved in provenance.

### INV-7 Artifact immutability

Artifact identifiers MUST resolve to immutable content.

### INV-8 Extension containment

Extensions MUST NOT redefine core semantics, alter terminal state meanings, erase delegation visibility, or weaken provenance obligations.

### INV-9 Unknown-critical fail-closed

If a message contains an extension marked `critical` and the recipient does not understand it, the recipient MUST reject the message. Silent acceptance of unknown critical extensions is a protocol violation.

### INV-10 Mandatory signatures

Every FAMP message MUST carry an Ed25519 signature over its canonical form. Unsigned messages MUST be rejected. Signature verification failure MUST result in message rejection with `unauthorized` error.

### INV-11 Negotiation bounds

Every task MUST have a finite negotiation round limit. The default is 20 rounds. When the limit is reached without a `commit`, the task transitions to `EXPIRED`.

---

# LAYER 1 — IDENTITY & PRESENCE

## 5. Identity Model

### 5.1 Principal identity

The enduring identity of an agent within a federation. A principal persists across restarts, upgrades, and instance changes.

Format:
```
agent:<authority>/<name>
```

The authority is a DNS domain controlled by the federation operator. The name is unique within that authority.

Examples:
```
agent:bst-house.org/ocr-transcriber
agent:friedlam.com/site-manager
agent:internal.acme.corp/invoice-processor
```

### 5.2 Instance identity

A specific runtime incarnation of a principal. Instances are ephemeral. Multiple instances of the same principal MAY exist concurrently.

Format:
```
agent:<authority>/<name>#<instance-id>
```

The instance-id is opaque to the protocol. It MUST be unique among concurrent instances of the same principal.

Why instance identity is required:

- Commitments must be attributable to a specific instance for debugging and accountability.
- If two instances of the same principal issue conflicting commits, the protocol must distinguish them.
- Failure recovery requires knowing which instance failed.
- Replay detection requires instance-level granularity.

### 5.3 Authority scope

Each message MAY declare the authority scope under which it is sent. Authority is never inherited by implication.

Defined authority levels:

| Level | Meaning |
|-------|---------|
| `advisory` | Information only, creates no obligation |
| `negotiate` | May exchange proposals but not commit |
| `commit_local` | May commit the sender's own resources |
| `commit_delegate` | May commit and may delegate downstream |
| `transfer` | May transfer task ownership to another agent |

An agent operating at `negotiate` scope cannot issue a valid `commit`. A `commit` message with authority `advisory` or `negotiate` is a protocol violation.

Authority scope governs task-scoped messages normatively. For standalone and conversation-scoped messages (discovery, capability refresh, federation control, cancellation attempts), authorization is governed by federation policy, not by the protocol's authority ladder. The protocol does not define who may send a capability refresh notice or a federation control message — those are federation-level administrative concerns.

### 5.4 Federation membership

A principal's membership in a federation is attested by the federation operator, not by the principal itself. The mechanism of attestation (signed certificates, registry entries, etc.) is federation-specific and outside the protocol core.

An agent receiving a message from a principal in a foreign federation MUST verify:

1. Message signature is valid against the sender's public key.
2. The sender's public key is attested by the sender's federation.
3. The sender's federation is in the recipient's trust list.

## 6. Capability Posture

### 6.1 Agent Card

An agent publishes an Agent Card to make itself discoverable. The card describes the agent's current capability posture — not a permanent contract, but a snapshot that may drift.

```json
{
  "famp": "0.5.0",
  "principal": "agent:bst-house.org/ocr-transcriber",
  "name": "BST Manuscript Transcriber",
  "description": "Transcribes historical Hebrew, Aramaic, and Yiddish manuscripts from page images.",
  "capabilities": [ "..." ],
  "endpoints": [ "..." ],
  "public_key": "<Ed25519 public key, base64url>",
  "issued": "2026-04-12T00:00:00Z",
  "expires": "2026-07-12T00:00:00Z",
  "card_version": 3,
  "min_compatible_version": 2,
  "signature": "<self-signature, base64url>"
}
```

### 6.2 Capability claim classes

A capability declaration MUST distinguish:

| Class | Meaning |
|-------|---------|
| `intrinsic` | The agent can perform this in principle |
| `available` | The agent can perform this right now |
| `authorized` | The agent is permitted to perform this under current policy |
| `delegable` | The agent may delegate this capability downstream |

An agent may have an intrinsic capability that is currently unavailable (GPU offline), or available but unauthorized under the requesting task's policy.

### 6.3 Capability versioning

Each capability MUST have a version identifier. When an agent's capability schema changes:

- The card version increments.
- The card declares a `min_compatible_version`.
- In-flight conversations that were initiated under a prior card version continue under the terms of the proposal/commit that referenced that version.
- An agent MAY reject a new request if the requester's understanding is based on an expired card version.

### 6.4 Card distribution

The protocol does not prescribe discovery infrastructure. Valid mechanisms include:

- Well-known URL: `https://<authority>/.well-known/famp/<name>.json`
- Federation registry with semantic search
- Direct in-band exchange via `describe` message
- Static configuration

---

# LAYER 2 — CONVERSATION & CAUSALITY

## 7. Messages

### 7.1 Envelope

```json
{
  "famp": "0.5.0",
  "id": "<UUIDv7>",
  "from": "agent:example.com/alice#inst-7a3b",
  "to": "agent:example.com/bob",
  "scope": "<standalone | conversation | task>",
  "conversation": "<conversation ID, if scoped>",
  "task": "<task ID, if task-scoped>",
  "class": "<message class>",
  "causality": {
    "rel": "<relation type>",
    "ref": "<message ID or task event ID>"
  },
  "authority": "commit_local",
  "commitment": "<commitment ID, if referencing or creating a commitment>",
  "terminal_status": "<completed | failed, required on final deliver only>",
  "ts": "<ISO 8601>",
  "validity_window": "<ISO 8601 duration>",
  "idempotency_key": "<optional>",
  "signature": "<Ed25519 signature, base64url>",
  "extensions": [],
  "body": {}
}
```

**`signature` field (REQUIRED).** Every FAMP message MUST be signed by the sender's private key. The signature covers the canonical JSON representation of the entire message excluding the `signature` field itself (all remaining fields, keys sorted, no whitespace). Unsigned messages MUST be rejected. The REQUIRED algorithm is Ed25519 (non-negotiable in v0.5 to avoid downgrade attacks).

The signature makes messages non-repudiable. A `commit` message signed by an agent is proof of obligation. A `delegate` message signed by the delegating agent is proof of delegation. A `deliver` with `terminal_status` signed by the delivering agent is proof of terminal report. This is the foundation for verifiable provenance: if an agent claims no delegation occurred but a signed `delegate` message exists in the conversation graph, the claim is provably false.

**`commitment` field.** A commitment has a stable identity distinct from the message ID of the `commit` message that created it. The commitment ID is assigned by the committing agent at commit time and MUST remain stable across supersession chains. When a commit is superseded (Section 11.3), the new commit references the same commitment ID with an incremented version, preserving lineage. All subsequent messages that reference a commitment — `delegate`, `deliver`, `control` with `cancels` — use the commitment ID, not the commit message ID.

This prevents the fragility of overloading message identity with semantic identity. A commitment may be created, superseded, partially fulfilled, and eventually completed across many messages, but its identity is stable throughout.

**`terminal_status` field.** Required on `deliver` messages with `fulfills` relation that represent a final delivery (not `interim: true`). The value is an enumerated protocol-level field, not a body field:

| Value | Task state transition |
|-------|----------------------|
| `completed` | → `COMPLETED` |
| `failed` | → `FAILED` |

This field MUST NOT appear on any other message class. It MUST NOT appear on interim deliveries. Its value set is closed and MUST NOT be extended (extensions that need additional terminal states must propose them as core protocol changes, not as extension values).

This eliminates the "single exception to envelope purity" identified in review. The state machine is now driven entirely by `(class, relation, terminal_status)` — all envelope-level fields. No body inspection is required for any state transition.

### 7.2 Message scope

Messages are scoped to one of three contexts:

| Scope | Container | Use |
|-------|-----------|-----|
| `standalone` | None | Discovery, capability refresh, federation control, unsolicited advisories |
| `conversation` | Conversation ID | General conversation, information exchange, negotiation before task creation |
| `task` | Task ID (implies conversation) | Task-scoped negotiation, commitment, delegation, delivery, reporting |

Standalone messages require no conversation. This resolves the problem of forcing discovery pings, key rotation notices, and federation control messages into pseudo-conversations.

### 7.3 Causal relations

Every non-initial message MUST declare exactly one primary causal relation. It MAY declare additional secondary relations.

| Relation | Meaning | Valid on class |
|----------|---------|----------------|
| `initiates` | Starts a new conversation or task | `request`, `propose` |
| `replies_to` | General response to a prior message | any |
| `proposes_against` | Counter-proposal modifying terms of a prior proposal | `propose` |
| `commits_against` | Commitment accepting a specific proposal | `commit` |
| `fulfills` | Delivery satisfying a commitment | `deliver` |
| `updates` | Progress or interim status on ongoing work | `deliver` (with `interim: true`) |
| `delegates_from` | Delegation originating from a commitment | `delegate` |
| `supersedes` | Replaces a prior message (the prior message is void) | any (same sender only) |
| `cancels` | Requests or enacts cancellation of a prior commitment or task | `control` |
| `closes` | Closes a conversation | `control` |
| `acknowledges` | Semantic acknowledgment of receipt and processing | `ack` |

Each relation type is valid only on the message classes listed. A `commits_against` relation on a `propose` message, for example, is a protocol violation. This constraint ensures the state machines can determine what happened from the (class, relation) pair without inspecting the body.

**Relation-to-state-machine mapping:**

| Relation | Envelope context | Task state transition |
|----------|-----------------|---------------------|
| `initiates` | task-scoped message | → `REQUESTED` |
| `commits_against` | — | `REQUESTED` → `COMMITTED` |
| `fulfills` | `terminal_status: completed` | `COMMITTED` → `COMPLETED` |
| `fulfills` | `terminal_status: failed` | `COMMITTED` → `FAILED` |
| `updates` or `fulfills` | `interim: true`, no `terminal_status` | no transition (stays `COMMITTED`) |
| `cancels` | task in `REQUESTED` state | `REQUESTED` → `REJECTED` |
| `cancels` | task in `COMMITTED` state | `COMMITTED` → `CANCELLED` |
| timeout | — | any non-terminal → `EXPIRED` |

Every state transition is determined by the tuple `(class, relation, terminal_status, current_state)` — all envelope-level fields. No body inspection is required for any state transition.

A `control` message with `cancels` relation sent against a task in `REQUESTED` state constitutes a rejection. A `control` message with `cancels` relation sent against a task in `COMMITTED` state constitutes a cancellation. The same message class and relation type; the current task state determines the terminal state.

### 7.4 Semantic acknowledgment

Semantic acknowledgment is a protocol-level act, distinct from transport-level delivery confirmation.

A semantic `ack` (message class `ack`, relation `acknowledges`) tells the sender:

- The message was received by the agent (not just the transport)
- The message was parseable
- The causal reference was resolved (the referenced message exists in the recipient's state)
- The message was attached to the correct conversation/task context

A semantic `ack` does NOT mean:

- The recipient agrees with the content
- The recipient will act on it
- The recipient has committed to anything

An `ack` MAY include a disposition indicating how the message was classified:

| Disposition | Meaning |
|-------------|---------|
| `accepted` | Message processed normally |
| `duplicate` | Message ID already seen, ignored |
| `stale` | Message arrived outside validity window |
| `malformed` | Message could not be parsed |
| `orphaned` | Causal reference could not be resolved |
| `refused` | Message understood but rejected on policy grounds |

This replaces the gap where transport ack ("bytes received") and semantic processing ("agent understood") were conflated.

## 8. Message Classes

FAMP defines nine message classes. This is more than ADMP's seven and fewer than FIPA's twenty-two. Each class corresponds to a distinct protocol-level act that the state machines need to distinguish.

### 8.1 `announce`

Scope: standalone or conversation.

Declares presence, identity continuity, and optional session posture. Used at federation join, instance startup, and reconnection.

### 8.2 `describe`

Scope: standalone or conversation.

Requests or provides an Agent Card. May be used for capability refresh, version check, or initial discovery.

### 8.3 `ack`

Scope: any.

Semantic acknowledgment. Confirms receipt and processing disposition (Section 7.4).

### 8.4 `request`

Scope: conversation or task.

Asks for information or action. Does not create commitment. A `request` opens negotiation space.

### 8.5 `propose`

Scope: conversation or task.

Offers candidate terms for work. A proposal is a structured object that can be accepted, countered, or rejected. Proposals bind nothing until a `commit` references one.

### 8.6 `commit`

Scope: task (required).

Creates a binding obligation. The `commit` MUST reference a specific proposal via `commits_against`. The commitment binds the sender to the scope, bounds, policies, and delegation permissions specified in that proposal.

### 8.7 `deliver`

Scope: task (required).

Provides work output. A delivery MUST reference the commitment it fulfills via `fulfills`. Deliveries may be partial (flagged `interim: true`) or final.

### 8.8 `delegate`

Scope: task (required).

Explicitly delegates a portion or all of a task to another agent. This is a first-class protocol act, not a body convention. Section 12 defines delegation semantics in full.

### 8.9 `control`

Scope: any.

Cancels, expires, supersedes, or closes prior protocol state. Used for lifecycle management that doesn't fit the negotiation→commitment→delivery path.

---

# LAYER 3 — TASK & COMMITMENT

## 9. Task Model

### 9.1 Task instantiation

A task is created exclusively by an explicit protocol act: an agent sends a `request` or `propose` message with `scope: task` and relation `initiates`. The task ID is assigned by the initiator.

Task creation is always explicit. A conversation-scoped negotiation does NOT implicitly become a task. If agents are negotiating in conversation scope and decide to formalize the work, one agent MUST send a new task-scoped `propose` with `initiates` relation. This creates the task. Prior conversation-scoped messages are context, not task history.

This rule prevents divergence between implementations that might "upgrade" conversations into tasks implicitly versus those that require explicit instantiation. There is exactly one path to task creation: an `initiates` message at task scope.

### 9.2 Task ownership

A task has exactly one current owner at any time. The owner is the agent accountable for the task's terminal state. Ownership and execution are distinct: the owner may execute directly or delegate.

The initial owner is the agent that receives and commits to the task (the committing agent), not the agent that initiated it. The initiator requests work; the committer owns it.

Ownership transfer is an explicit protocol act (Section 12.3). At no point may a task have zero owners or ambiguous ownership.

### 9.3 Task bounds

Every task MUST declare at least two bounds from the following set:

| Bound | Semantics |
|-------|-----------|
| `deadline` | Absolute time after which the task expires |
| `budget` | Maximum resource expenditure (tokens, USD, compute-hours) |
| `hop_limit` | Maximum delegation depth |
| `policy_domain` | Named policy set governing execution |
| `authority_scope` | Maximum authority level for the task |
| `max_artifact_size` | Maximum output size |
| `confidence_floor` | Minimum acceptable confidence for results |
| `recursion_depth` | Maximum subtask nesting depth |

The requirement for at least two bounds is a protocol-level defense against unbounded work. A single bound (e.g., deadline only) permits unlimited resource consumption before the deadline. Two bounds create a tighter feasibility envelope.

### 9.4 Task decomposition

A task owner may decompose a task into subtasks if and only if:

1. The task's delegation permissions allow it.
2. Each subtask inherits the parent's policy domain unless explicitly overridden with equal or stricter policy.
3. Each subtask's bounds are within the parent's bounds (a subtask cannot have a later deadline than its parent).
4. Provenance links subtasks to parent.

### 9.5 Task state machine

```
                    request / propose (scope: task, rel: initiates)
                         │
                         ▼
                   ┌───────────┐
          ┌───────►│ REQUESTED │
          │        └─────┬─────┘
          │              │ (negotiation cycles here:
          │              │  propose with proposes_against)
          │              │
    ┌─────┴─────┐        │
    │ REJECTED  │◄───────┤ (control with cancels, on REQUESTED)
    └───────────┘        │
                         │ commit (with commits_against)
                         ▼
                   ┌───────────┐
                   │ COMMITTED │◄──── deliver (interim: true, rel: updates)
                   └─────┬─────┘
                         │
            ┌────────────┼────────────┐
            │            │            │
     deliver│(final,     │deliver     │control
     status:│completed)  │(status:    │(rel: cancels)
            │            │ failed)    │
            ▼            ▼            ▼
     ┌───────────┐ ┌──────────┐ ┌───────────┐
     │ COMPLETED │ │  FAILED  │ │ CANCELLED │
     └───────────┘ └──────────┘ └───────────┘

     Any non-terminal state ──(timeout)──► EXPIRED
```

Terminal states: `COMPLETED`, `FAILED`, `CANCELLED`, `REJECTED`, `EXPIRED`.

Exactly one terminal state per task (INV-5).

### 9.6 Terminal precedence rule

**Once a task has entered a terminal state and the terminal report has been semantically acknowledged (via `ack` with `accepted` disposition), the terminal state is final. No subsequent message may alter it.**

This means:

- A late-arriving `control` with `cancels` sent after a valid terminal `deliver` has been acknowledged is rejected with `conflict` error.
- A `supersedes` targeting a terminal delivery is a protocol violation.
- A delegation `transfer` after terminal state is a protocol violation.
- An `EXPIRED` timeout that fires after a terminal delivery has been acknowledged does not override the terminal state.

The acknowledgment requirement prevents races: if an agent sends a final delivery but the recipient hasn't acknowledged it, a concurrent cancellation from the task owner is still valid. The terminal state crystallizes at the point of semantic acknowledgment, not at the point of sending.

**Default terminal conflict resolution.** If a final `deliver` and a `control` with `cancels` are both sent before either is semantically acknowledged, the following default rule applies: **the final delivery takes precedence.** The rationale: work has been completed and results exist; cancellation after completion is wasteful and should have arrived earlier. The task enters the terminal state indicated by the delivery's `terminal_status`.

Federations MAY override this default with an explicit terminal conflict resolution policy (e.g., "task owner cancellation always wins"). But a conformant implementation that has no federation-specific policy MUST apply the delivery-wins default. This ensures deterministic behavior across federations without requiring every federation to define a race resolution policy.

### 9.7 Conversation state machine

Conversations have a simpler lifecycle than tasks because conversations carry no commitment semantics:

```
OPEN ──► CLOSED
```

A conversation is `OPEN` from its first message until any participant sends a `control` with `closes` relation, or until a federation-defined timeout. An `OPEN` conversation may contain zero or more tasks in various states.

**Conversation closure effects:**

- Closing a conversation does NOT affect the validity or state of tasks within it. Tasks in `COMMITTED` state remain committed. Deliveries, progress updates, and terminal reports on existing tasks remain valid after conversation closure.
- After closure, no new tasks may be created within the conversation (a task-scoped `initiates` on a closed conversation is rejected with `conflict`).
- After closure, no new conversation-scoped messages may be sent (rejected with `conflict`). Only task-scoped messages for existing tasks and standalone messages remain valid.
- If all tasks within a conversation have reached terminal states, the conversation SHOULD be closed. Implementations MAY auto-close conversations after a federation-defined idle timeout.

The deliberate simplicity here is the result of separating conversation state from task state. The conversation is a container. The task is where commitment, delegation, and terminality live. This avoids the overloaded state machine problem identified in review.

---

## 10. Negotiation Semantics

### 10.1 Principle

No agent is required to accept requested work as framed. Any agent may respond with:

- Refusal (via `control` with appropriate disposition)
- Clarification request (via `request`, relation `replies_to`)
- Counter-proposal (via `propose`, relation `proposes_against`)
- Conditional acceptance proposal (via `propose` with explicit conditions)
- Decomposition proposal (via `propose` suggesting subtask structure)
- Redirection (via `propose` suggesting a different agent)

### 10.2 Proposal structure

A proposal is a candidate commitment surface. It MUST contain:

- **Scope**: what work is proposed
- **Bounds**: at least the bounds required by INV-4

A proposal SHOULD contain:

- **Terms**: deadline, budget, quality targets
- **Delegation permissions**: whether the committing agent may delegate, and if so, under what constraints
- **Artifact expectations**: expected input/output formats and sizes
- **Policy references**: applicable policies
- **Natural language summary**: human/LLM-readable description of the proposal

### 10.3 Counter-proposal grammar

This is where negotiation protocols break. The following rules govern counter-proposals.

**Rule 1: A counter-proposal MUST reference the proposal it modifies.**

The relation `proposes_against` with `ref` pointing to the prior proposal creates an explicit modification chain.

**Rule 2: A counter-proposal MUST be interpretable as a complete proposal.**

It is not a diff. It is a full candidate commitment surface that happens to be linked to a prior one. This avoids the ambiguity of partial modifications where it's unclear which terms from the original survive.

Rationale: diff-based counter-proposals seem efficient but create interpretation disputes. "I modified the deadline but not the budget" — did the budget carry over? What if the original proposal had an implicit budget? Full-proposal counters are verbose but unambiguous.

**Rule 3: A counter-proposal MAY include a `modifications` field listing which terms changed.**

This is a courtesy for efficient processing, not a normative requirement. The full proposal is the source of truth, not the diff.

**Rule 4: Counter-proposal chains are capped.**

Each task MUST have a negotiation round limit. The default is 20 rounds (total messages with `proposes_against` relation within the task). The limit MAY be set explicitly in the initiating `request` or `propose` message, or declared in the Agent Card as `max_negotiation_rounds`.

When the limit is reached without a `commit`, the task transitions to `EXPIRED`. This prevents a buggy or malicious agent from flooding an orchestrator with infinite counter-proposals.

Agents SHOULD also declare `max_concurrent_tasks` in their Agent Card. A recipient MAY reject new task initiations with `capacity_exceeded` if they are at their declared limit. These are protocol-level defenses against resource exhaustion during negotiation.

**Rule 5: A later proposal supersedes an earlier one from the same sender only if it explicitly declares `supersedes` relation.**

Without explicit supersession, all proposals from a sender remain valid candidates for commitment. This allows an agent to offer multiple options simultaneously ("I can do X for $5 by Friday, or Y for $3 by Monday").

### 10.4 Partial acceptance

An agent MAY commit against a subset of a proposal's scope if:

- The commit message explicitly states the accepted subset.
- The commit references the proposal via `commits_against`.
- The commit constitutes a complete, bounded commitment on its own terms.

The remaining scope is not implicitly committed. It may be the subject of further negotiation, a separate proposal, or may lapse.

**Overlap and conflict in partial acceptance.** The protocol does not prevent overlapping partial commitments. If agent B commits to rows 1–500k and agent C commits to rows 400k–900k of the same dataset, the protocol records both commitments faithfully. Detecting, preventing, or resolving scope overlap is the responsibility of the orchestrating agent, not the protocol. The protocol provides the causal graph and commitment records needed to detect overlap; it does not enforce non-overlap as an invariant. Federations that require non-overlapping scope partitioning SHOULD define this as a policy constraint on task decomposition.

### 10.5 Negotiation closure

Negotiation remains non-binding until a `commit` is issued. A `commit` message:

- MUST reference exactly one proposal via `commits_against`.
- MUST be sent at `commit_local` or higher authority scope.
- Binds the sender to the terms of the referenced proposal (or the explicitly stated subset).
- Transitions the task from `REQUESTED` to `COMMITTED`.

---

## 11. Commitment Semantics

### 11.1 Commit is the only binding act

Only a `commit` message creates protocol-level obligation. Everything before it — requests, proposals, counter-proposals, acknowledgments — is non-binding.

### 11.2 What a commit binds

A commit binds the sender to a tuple of:

- Task scope (what work)
- Accepted bounds (deadline, budget, etc.)
- Accepted policies (handling, retention, disclosure rules)
- Delegation permissions (may delegate, may not, may delegate with restrictions)
- Reporting obligations (progress frequency, interim deliveries, final report format)
- Expected terminal condition (what constitutes completion)
- **Capability snapshot**: the committing agent's capability posture at time of commitment

**Capability snapshot binding.** A commitment is made against the agent's capability posture as advertised at the time of commitment. If the agent's capabilities subsequently change (model upgrade, tool removal, policy shift), the commitment remains governed by the original capability posture. An agent MUST NOT claim inability to fulfill a commitment due to a capability change that occurred after commitment. If the agent genuinely loses the ability to perform committed work, it MUST report failure via `deliver` with `terminal_status: failed` — not silently evade the obligation via capability drift.

### 11.3 No silent widening

After commitment, the sender MUST NOT widen scope, relax policy, expand delegation rights, or extend bounds without:

- A new proposal (relation `proposes_against` the original commit)
- Acceptance of that proposal by the counterparty
- A new commit superseding the original

This creates a renegotiation cycle within a committed task. The task remains `COMMITTED` during renegotiation. The new commit `supersedes` the old one.

This answers the open question from ADMP v0.2 ("Can committed conversations renegotiate?"): yes, through explicit superseding commits, without returning to the `REQUESTED` state.

### 11.4 Conditional commitments

Conditional commitments are allowed. Conditions MUST be:

- Explicit in the commit message body
- Machine-evaluable where possible
- Time-bounded (a condition that can never be evaluated is a defect)

Examples:

- "Committed if access token remains valid until 2026-04-15T00:00:00Z"
- "Committed if downstream delegation depth ≤ 1"
- "Committed if total artifact output ≤ 10MB"

If a condition becomes false, the commitment lapses. The committing agent MUST report the lapse via a `control` message (relation `cancels`, disposition `condition_failed`).

### 11.5 Competing commits

If two instances of the same principal issue conflicting commits for the same task, the conflict MUST be resolved by:

1. Explicit `supersedes` relation from the later commit (if intentional)
2. Federation-defined conflict resolution policy
3. Task owner decision

NOT by message arrival order (INV-2 and the causal integrity objective require this).

---

## 12. Delegation Semantics

Delegation is a first-class protocol act, not a body convention. This section defines it with the rigor it requires.

### 12.1 Delegation is explicit

Any delegation of task work MUST be represented as a `delegate` message in the protocol. The existence of delegation is visible in the conversation graph.

### 12.2 Delegation rights are separate from execution rights

An agent authorized to perform work is not automatically authorized to delegate it. Delegation rights are specified in the commitment (Section 11.2) and may be:

- Forbidden: agent must execute directly
- Permitted: agent may delegate with specified constraints
- Required: agent is expected to delegate (e.g., an orchestrator)

### 12.3 Delegation forms

The protocol recognizes three delegation forms:

| Form | Ownership | Accountability | Downstream commitment |
|------|-----------|---------------|----------------------|
| `assist` | Stays with delegator | Delegator fully accountable | Downstream contributes, delegator integrates |
| `subtask` | Delegator retains parent task ownership | Downstream owns subtask, delegator owns parent | Downstream commits to subtask bounds |
| `transfer` | Moves to delegate | Delegate assumes full accountability | Delegate commits to original task bounds |

**Transfer and commitment lineage.** When a `transfer` delegation occurs:

1. The original commitment held by the transferring agent is closed. It is no longer active — the transferring agent's obligation under it ceases.
2. The delegate MUST issue a new `commit` message against the original task, creating a new commitment (with a new commitment ID) that binds the delegate to the task bounds.
3. Provenance MUST link the new commitment to the closed original commitment and the `delegate` message that triggered the transfer.
4. Until the delegate issues the new `commit`, the task is in a transitional state: ownership has been offered but not accepted. The task remains `COMMITTED` under the original commitment. If the delegate declines (via `control` with `cancels`), ownership reverts to the transferring agent.
5. **Transfer timeout.** A transfer offer MUST include a `transfer_deadline` (default: 5 minutes from the `delegate` message timestamp). If the delegate has not issued a `commit` by the deadline, the transfer lapses automatically: ownership reverts to the transferring agent, the original commitment reactivates, and the transferring agent MUST send a `control` message with `supersedes` relation voiding the lapsed `delegate`. This prevents tasks from entering permanent limbo when delegates ghost or are unreachable.

This prevents dual accountability ambiguity: at any point, exactly one commitment is active for a task, held by exactly one owner. The transfer timeout ensures this property holds even under delegate failure.

### 12.4 Delegation message structure

A `delegate` message MUST include:

- **form**: `assist`, `subtask`, or `transfer`
- **commitment_ref**: the commitment under which delegation is authorized
- **downstream**: the delegate agent's principal identity
- **scope**: what portion of the task is delegated
- **bounds**: bounds for the delegated work (MUST be within parent bounds)
- **delegation_ceiling**: constraints on further re-delegation

### 12.5 Delegation ceiling

Each delegation MAY specify:

- **max_hops**: maximum further delegation depth (0 = no re-delegation)
- **max_fanout**: maximum number of concurrent downstream delegates (absent = unlimited)
- **allowed_delegates**: set of permitted downstream agents
- **forbidden_delegates**: set of excluded agents
- **policy_inheritance**: whether parent task policies apply to delegates

A delegation with `max_hops: 0` means the delegate must execute directly.

**Fan-out.** The protocol defines `max_fanout` as an optional ceiling constraint. If absent, the protocol does not constrain parallel delegation — an agent with `max_hops: 1` may delegate to an arbitrary number of agents simultaneously. Federations concerned about resource amplification SHOULD require `max_fanout` in their delegation policies. The protocol surfaces the constraint; enforcement is federation-level.

### 12.6 Provenance obligations

Any terminal report on a task where delegation occurred MUST include:

- That delegation occurred (the `delegate` message is part of the conversation graph)
- Which delegation form was used
- The task lineage (parent → subtask → sub-subtask chain)
- Whether downstream results were advisory or binding input to the final output

The protocol requires preservation of provenance, not full disclosure of internal reasoning. An agent is not required to expose its chain-of-thought, prompt, or model architecture. It IS required to expose that delegation happened and what the dependency structure was.

### 12.7 Silent subcontracting prohibition

An agent MUST NOT present delegated work as purely first-party work when the downstream contribution materially shaped the result.

In a trusted federation, this is primarily about auditability and debugging, not adversarial fraud. When a multi-agent pipeline produces a bad result, the first question is "which agent's work was the source of the error?" Without delegation visibility, that question is unanswerable.

---

## 13. Freshness, Replay, and Supersession

These three concepts are distinct and MUST NOT be conflated.

### 13.1 Freshness

A message is fresh if its `ts` is within the recipient's configured validity window. The default window is 5 minutes, configurable per federation.

**Clock skew.** The protocol does not define a canonical time source. Federations MUST define a maximum tolerated clock skew (RECOMMENDED: ±30 seconds). The validity window MUST be at least 2× the tolerated skew to prevent false stale rejections of fresh messages. Agents SHOULD use NTP or equivalent time synchronization. Without a federation-defined skew tolerance, freshness semantics are unstable: agent A's fresh message may appear stale to agent B whose clock runs ahead.

A stale message is not necessarily invalid. It may be:

- A legitimate late delivery on a slow transport
- A retransmission after transport failure
- An authentic message that was delayed by an intermediary

The recipient's response to a stale message depends on its type:

| Message class | Stale handling |
|---------------|---------------|
| `ack` | MUST accept (late ack is better than no ack) |
| `propose` | MUST reject with `stale` disposition (proposals with old timestamps may reflect outdated terms) |
| `commit` | MUST reject with `stale` disposition (critical — a stale commit may reflect outdated state) |
| `deliver` | MAY accept; if accepted, the recipient MUST validate the delivery against the current task state and commitment. If the task has already reached a terminal state, reject with `conflict` |
| `control` | MAY accept if the control action is still meaningful given current task state; MUST reject with `stale` if the target has already reached a terminal state (Section 9.6) |

### 13.2 Replay

A replay is a message whose `id` has already been processed. Replays are distinguished from retransmissions by the `idempotency_key` field.

- If a message has the same `id` as a previously processed message: it is a replay. Reject silently or with `duplicate` disposition.
- If a message has a different `id` but the same `idempotency_key` as a previously processed message AND the content is semantically equivalent: it is a retransmission. Process idempotently (return the same response as the original).
- If a message has a different `id` but the same `idempotency_key` as a previously processed message AND the content differs materially: this is a **sender defect or an integrity violation**. The recipient MUST reject the message with `conflict` disposition and SHOULD log the discrepancy. The protocol does not attempt to reconcile divergent content under the same idempotency key — that is either a bug (sender reused a key incorrectly) or an attack (key collision or tampering).
- If a message has a different `id` and no `idempotency_key`: it is a new message. Process normally.

Implementation: recipients SHOULD maintain a bounded cache of recent (`id`, `idempotency_key`, content hash) tuples. A bloom filter is acceptable for the false-positive tradeoff on `id` deduplication. The cache window MUST be at least as long as the validity window.

### 13.3 Supersession

Supersession is an explicit protocol act. A message with relation `supersedes` and a `ref` to a prior message voids the prior message.

Supersession rules:

- Only the original sender (same principal) can supersede a message.
- A superseded proposal is no longer a valid target for `commits_against`.
- A superseded commit triggers renegotiation (Section 11.3).
- Supersession of a terminal report is a protocol violation (terminal states are final).
- Supersession does not erase the superseded message from the conversation history — it marks it void.

### 13.4 Retransmission vs. semantic retry

| Situation | Same `id` | Same `idempotency_key` | Same content | Treatment |
|-----------|-----------|----------------------|--------------|-----------|
| Duplicate delivery | Yes | — | Yes | Ignore |
| Retransmission | No | Yes | Yes | Idempotent processing |
| Semantic retry | No | No | Similar | New message, new processing |
| Replay attack | Yes | — | Yes | Reject (duplicate) |

Semantic retry — sending a new message with similar content after a prior attempt was not acknowledged — is a legitimate protocol action. It creates a new message in the conversation graph. The recipient MAY recognize it as semantically similar to a prior message and handle accordingly, but the protocol treats it as a new message.

---

## 14. Provenance

### 14.1 Why provenance is core

In multi-agent systems, output without lineage is operationally dangerous. If an agent delivers a result, the consuming agent needs to know:

- Where the result came from
- Whether delegation was involved
- What bounds and policies governed its production
- Whether the result depends on artifacts from other agents

Without provenance, debugging is guesswork and accountability is impossible.

### 14.2 Provenance requirements

A terminal delivery (a `deliver` message with `fulfills` relation and `interim: false` or absent) MUST be accompanied by provenance that exposes:

- **Originating task reference**: which task this delivery fulfills
- **Commitment lineage**: which commit (and which proposal it accepted) authorized the work
- **Delegation lineage**: if delegation occurred, the chain of `delegate` messages
- **Artifact lineage**: which input artifacts were consumed, which output artifacts were produced
- **Policy context**: which policies governed execution

### 14.3 Provenance canonicalization

Provenance MUST be representable as a deterministic structure suitable for hashing and signing. The specific serialization format is not defined by the protocol core, but the provenance for a given task MUST produce identical byte output when serialized by any conformant implementation given the same inputs (canonical JSON with sorted keys and no whitespace is RECOMMENDED).

Without deterministic representation, provenance cannot be verified by third parties, compared across agents, or included in signed terminal reports. This requirement does not mandate a specific format — it mandates the property that any chosen format must exhibit.

### 14.4 Minimal provenance principle

The protocol requires preservation of the above structural provenance. It does NOT require:

- Internal reasoning traces
- Prompt content
- Model architecture details
- Intermediate computation steps

Provenance tracks responsibility and dependency. It does not mandate transparency of thought.

### 14.5 Provenance redaction and verifiability

Federations MAY allow redaction of provenance details (e.g., specific artifact content, delegation target identity in `assist` delegations). But the following MUST NOT be redacted under any circumstances:

- That delegation occurred (the signed `delegate` message exists in the conversation graph)
- The delegation form (assist/subtask/transfer)
- That policy constraints applied
- The terminal state and its cause

**Verifiability.** Because all FAMP messages are signed (Section 7.1), provenance claims are verifiable against the conversation graph. If an agent's terminal delivery claims no delegation occurred, but a signed `delegate` message from that agent exists in the task's conversation graph, the provenance is provably incomplete. The recipient SHOULD reject the delivery with `provenance_incomplete` error.

This does not prevent a malicious agent from omitting the `delegate` message entirely (performing silent subcontracting). The protocol cannot prevent an agent from lying by omission — that requires runtime monitoring, which is an enforcement concern, not a protocol concern. What the protocol provides is: if a `delegate` message was sent and signed, it cannot be later denied. The signature makes delegation non-repudiable once observed. Federation operators SHOULD deploy conversation graph auditors that verify provenance completeness against known message flows.

---

## 15. Error Semantics

Errors are structured, not opaque strings.

### 15.1 Error categories

| Category | Meaning |
|----------|---------|
| `malformed` | Message could not be parsed |
| `unsupported` | Message class or extension not supported |
| `unauthorized` | Sender lacks authority for this act |
| `stale` | Message outside validity window |
| `duplicate` | Message ID already processed |
| `orphaned` | Causal reference could not be resolved |
| `out_of_scope` | Request outside agent's capabilities |
| `capacity_exceeded` | Agent cannot accept more work |
| `policy_blocked` | Action forbidden by applicable policy |
| `commitment_missing` | Referenced commitment does not exist |
| `delegation_forbidden` | Delegation not permitted under current commitment |
| `provenance_incomplete` | Terminal report lacks required provenance |
| `conflict` | Message conflicts with existing protocol state |
| `condition_failed` | Conditional commitment's condition became false |
| `expired` | Task or message validity has elapsed |

### 15.2 Error distinctions

The protocol MUST distinguish:

- "I cannot parse this" (`malformed`) — sender's problem
- "I understand but refuse" (`unauthorized` or `policy_blocked`) — policy problem
- "I could do this but not under these terms" (`out_of_scope` + counter-proposal) — negotiation problem
- "I committed but can no longer deliver" (`deliver` with `terminal_status: failed`) — execution problem
- "I completed work but lineage is incomplete" (`provenance_incomplete`) — provenance problem

These distinctions are essential for orchestration correctness. An orchestrator that receives a generic "error" cannot distinguish a transient capacity issue (retry) from a policy refusal (find another agent) from a malformed message (fix the sender).

---

## 16. Concurrency and Conflict

### 16.1 Instance-local concurrency

Multiple instances of a principal MAY act concurrently. All commitments MUST be attributable by instance (INV-1).

### 16.2 Conflict resolution

If two valid protocol actions conflict (e.g., competing commits from different instances, cancellation crossing with completion), resolution MUST come from:

1. Explicit `supersedes` relation (if one action supersedes another)
2. Federation-defined precedence policy
3. Task owner decision, expressed as an explicit protocol message

NOT from message arrival order. This is a consequence of INV-2 (causal integrity) and the design objective that message meaning derives from typed causal relations, not sequence position.

**Task owner decisions MUST be protocol messages.** A task owner resolving a conflict MUST send a `control` message with the appropriate relation (`supersedes`, `cancels`, or `closes`) and a body explaining the resolution. Implicit or out-of-band decisions — verbal agreements, external tickets, dashboard clicks — have no protocol effect. If a conflict exists, it persists until a valid protocol message resolves it. This prevents the introduction of hidden authority channels.

### 16.3 Cancellation semantics

Cancellation is advisory until acknowledged or made authoritative by policy.

A `control` message with `cancels` relation:

- Is a request to cancel, not an accomplished fact
- Takes effect when the target agent acknowledges it (via `ack` with `accepted` disposition)
- OR when federation policy declares it authoritative (e.g., "task owner cancellations are immediate")

This prevents late-arriving cancel messages from retroactively voiding completed work.

---

## 17. Extension Model

### 17.1 Extension categories

Extensions may add:

- New artifact types
- Domain-specific policy vocabularies
- Richer negotiation terms
- Specialized report formats
- Federation-specific authorization mechanisms
- New message body fields

### 17.2 Extension limits (INV-8)

Extensions may NOT:

- Alter core commitment semantics
- Alter terminal state meanings
- Erase delegation visibility requirements
- Weaken provenance obligations
- Redefine causal relation types

### 17.3 Critical vs. non-critical

Every extension MUST declare criticality:

- `ignorable`: recipient may ignore if unsupported
- `critical`: recipient MUST reject the message if unsupported (INV-9)

### 17.4 Extension hygiene

Extension points that are never exercised tend to decay and become unsafe to depend on. Implementations SHOULD regularly exercise their extension processing paths. Federations SHOULD include at least one non-trivial extension in their standard profile to keep the extension machinery tested.

---

## 18. Transport Binding Requirements

A conformant transport binding MUST:

1. Deliver messages (no silent drops without transport-level error signaling)
2. Support messages up to 1MB in body size
3. Define how agent addressing maps to transport addressing
4. Define transport-level acknowledgment semantics (distinct from protocol-level semantic ack)

A conformant transport binding SHOULD:

5. Preserve per-conversation message order (but protocol correctness MUST NOT depend on this — see INV-2)
6. Support streaming for partial deliveries
7. Provide transport-level encryption
8. Support multiplexing multiple conversations over a single connection

The protocol explicitly does NOT require:

- Exactly-once delivery (at-least-once + idempotency keys is sufficient)
- Global ordering across conversations
- Persistent connections
- Any specific transport technology

---

## 19. Conformance Levels

### Level 1: Discoverable

Implements L1 only. Publishes a valid Agent Card. Responds to `describe` messages. Can be found by peers but does not participate in conversations or tasks.

### Level 2: Conversational

Implements L1 + L2. Supports conversation lifecycle, causal linkage, and semantic acknowledgment. Can exchange messages and negotiate. Cannot commit to or own tasks.

### Level 3: Task-capable

Implements all three layers. Supports the full task lifecycle: negotiation, commitment, delegation, delivery, terminal states, and provenance. This is the standard conformance level for production agents.

---

## 20. Protocol Review Criteria

A candidate implementation, extension, or protocol revision SHOULD be evaluated against these questions. If any cannot be answered from protocol-level information alone, the protocol surface is too weak.

1. Who committed to what, exactly?
2. Was the work delegated? If so, in what form?
3. Was delegation permitted under the governing commitment?
4. Which bounds governed the work?
5. Which policy context governed the work?
6. Is this message fresh, stale, duplicate, retransmitted, or superseded?
7. What ended the task, and why?
8. Can an auditor reconstruct the commitment and delegation lineage without access to any agent's internal reasoning?
9. If two instances of the same agent issued conflicting actions, which instance did what?
10. If a counter-proposal modified terms, can both the original and modified terms be recovered?

---

## 21. What This Protocol Deliberately Excludes

To keep the core stable, FAMP excludes:

- Economic incentives and payment settlement
- Reputation systems
- Open-internet identity and global trust
- Shared memory or shared state semantics
- Cognitive trace disclosure or chain-of-thought standardization
- Planning or workflow DAG languages
- Tool API standardization
- Agent lifecycle management (start, stop, upgrade, monitor)
- Specific serialization formats (JSON is used in examples; the protocol is format-agnostic)

These may be layered on as extensions, companion specifications, or federation-specific profiles.

---

## 22. Design Lineage

This section documents which prior protocols influenced FAMP and what was learned from each.

**FIPA-ACL (1996–2005)**: Demonstrated that shared ontologies and large performative sets (22 communicative acts) do not produce practical interoperability across heterogeneous systems. FAMP uses nine message classes tied to state machine transitions, not speech act theory. Capability matching uses natural language descriptions, not formal ontologies.

**KQML (1993)**: Showed that ambiguous performative semantics lead to incompatible dialects, defeating the purpose of standardization. FAMP defines intent via typed causal relations and state machine effects, not informal descriptions of speaker attitude.

**XMPP (1999–present)**: Federated architecture using DNS for trust boundaries is one of the most durable ideas in protocol design. The principal/instance identity split parallels XMPP's JID structure (user@domain/resource). But 340+ extension protocols show what happens when the core is too narrow: everything becomes an extension. FAMP keeps delegation, negotiation, and provenance in the core to reduce extension pressure.

**ActivityPub (2018)**: Sending activities with no capability discovery and no defined side effects works for social media but fails for coordination. FAMP requires capability posture advertisement and semantic acknowledgment.

**A2A (Google, 2025)**: Agent Cards, task lifecycle states, and agent opacity are good ideas, adopted here. A2A's JSON-RPC/HTTP binding and client-server topology are reasonable pragmatic choices but limit the protocol to web-native scenarios. FAMP is transport-agnostic with a peer-to-peer model.

**ACP (IBM/BeeAI, 2025)**: The system bus / data bus distinction influenced FAMP's L1/L2/L3 layering. ACP merged into A2A in August 2025, suggesting the community prefers consolidation. FAMP aims to be the consolidated protocol.

**NLIP (ECMA TC56, December 2025)**: The envelope protocol approach using natural language as a first-class content modality for LLM-era agents influenced FAMP's content agnosticism and the natural-language summary convention in proposals. FAMP does not adopt NLIP's specific envelope format but shares its philosophy that agents capable of natural language understanding need less rigid content schemas.

**EPP (RFC 5730)**: The extension model — small core, structured extension points, critical vs. non-critical extension marking — directly influenced FAMP's extension containment invariant and critical extension handling.

**RFC 6709 (Design Considerations for Protocol Extensions)**: The principle that extension points must be routinely exercised to remain viable influenced FAMP's extension hygiene recommendation (Section 17.4).

---

## 23. Open Questions for Peer Review

The following questions remain genuinely open. Items resolved in this revision (multi-party scope, terminal precedence, task instantiation, vocabulary consistency) are no longer listed here.

1. **Multi-party commitment profiles.** v0.4 is normatively bilateral. A future multi-party profile will need to define: commitment aggregation rules (unanimous, quorum, weighted), who may declare a task terminal, and how negotiation works when N agents must converge on terms. The likely shape is a coordination role (one agent aggregates bilateral commitments), but the interaction with the causal graph needs careful design.

2. **Streaming semantics.** Partial `deliver` messages (`interim: true`) handle checkpointing. But real-time streaming (token-by-token output) needs sequence numbers, backpressure, and completion markers. Should that be in the core or in a transport binding extension?

3. **Cross-federation delegation.** If agent A in federation X delegates to agent B in federation Y, which federation's policies govern? This requires bilateral peering agreements and policy intersection logic not defined in v0.4.

4. **Conversation archival.** Completed conversations are audit trails and potential training data. Should the protocol define archival format, retention requirements, or is that purely federation policy?

5. **Natural language in multilingual federations.** The dual representation convention (structured terms + natural language summary) assumes a shared language. Should the protocol define language negotiation, or is that a capability-level concern?

6. **Delivery body structure.** With `terminal_status` promoted to the envelope, the `deliver` message body is now fully opaque to the state machine. Should the protocol define RECOMMENDED body structure for deliveries (result payload, usage metrics, error detail for failures), or leave this entirely to capability-level schemas?

---

## 24. Summary

FAMP is built on one premise: **between autonomous agents, the protocol's primary job is not message delivery but commitment discipline under bounded delegation.**

That premise produces a small, layered core:

**L1**: Agents have dual identity (principal + instance), advertise capabilities with versioning and claim classes, and belong to a trust-establishing federation. All messages are signed (Ed25519, non-negotiable).

**L2**: Messages are causally linked via eleven typed causal relations, not sequence order. Semantic acknowledgment is a protocol act. Conversations are containers with no commitment semantics. State transitions are driven entirely by envelope-level fields. Negotiation is capped (default 20 rounds).

**L3**: Tasks carry commitment (with stable identity across supersession), bounds, delegation (three explicit forms with provenance obligations and transfer timeouts), and canonicalizable provenance. Negotiation precedes commitment. Commitment is the only binding act. Delegation is explicit and visible. Terminal states are final once acknowledged, with a deterministic default for concurrent delivery/cancellation races (delivery wins). Provenance is signed, deterministic, and non-repudiable.

Nine message classes. Eleven causal relation types. Eleven invariants. Five terminal states. Three delegation forms. Three conformance levels. Three layers. Every message signed. Every state transition from envelope fields. Every commitment stably identified. Every delegation visible and non-repudiable.

Everything else is body content, federation policy, or transport binding.

---

*This specification is released for adversarial review. Try to break the invariants with valid message sequences. Try to create ambiguous terminal states. Try to hide delegation. Try to exhaust resources through negotiation. If you succeed, the spec has a bug.*
