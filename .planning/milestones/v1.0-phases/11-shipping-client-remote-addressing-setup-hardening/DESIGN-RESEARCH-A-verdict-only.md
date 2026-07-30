# **Architectural Evaluation and Resolution Strategy for the FAMP v1.0 Remote Addressing Blocker**

## **Executive Summary and Release Decision**

The release of version 1.0.0 of the Federated Agent-Messaging Protocol (FAMP) must be blocked pending the resolution of the remote principal addressing defect1. Proceeding with the v1.0.0 tag while shipping a client interface (famp send and its Model Context Protocol twin famp\_send) that silently drops egress messages as unroutable recipient errors contradicts the foundational charter of the protocol1. The v1.0 milestone promises a byte-exact, signature-verifiable implementation that enables two independent parties to interoperate out of the box1. Gate A human User Acceptance Testing (UAT) explicitly mandates that an operator can execute a signed envelope exchange across separate host environments and advance the underlying Task Finite State Machine (FSM) to a terminal state1. Shipping with a documented limitation is rejected because the shipping CLI toolchain currently renders this primary UAT recipe non-functional1.  
The architecture brief demonstrates that the underlying federation gateway (famp-gateway), cryptographic signing engine, and HTTP transport infrastructure (famp-transport-http) are verified, fully functional, and proven within integration test environments1. The defect is strictly localized to user-facing envelope construction and target address resolution within the CLI layer1. To restore end-to-end functionality without compromising architectural invariants, the project should implement Candidate C5, a refined target decoupling strategy paired with a payload class upgrade1. This fix decouples the local broker routing target from the canonical remote principal contained within the signed envelope header, and replaces the Phase-2 stub payload class with a task-driving payload structure1. This remediation requires no modifications to gateway egress logic, preserves local unsigned bus semantics, maintains complete cryptographic provenance, and keeps the codebase aligned with the v0.5.2 protocol specification authority1.

## **System Architecture and Root Cause Analysis**

The core architecture of FAMP relies on a strict operational separation between same-host local communication and cross-host federated transport1. The local plane operates over Unix-domain sockets where same-host agents exchange unsigned envelopes brokered by a local broker process1. Identity on the local path is enforced structurally by broker socket stamping rather than cryptographic signatures1. Conversely, the federation plane introduces famp-gateway processes acting as local stand-in proxies for remote principals1. When a local agent addresses a remote principal, the local gateway drains the proxy mailbox, decorates the envelope with federation headers, signs the entire structure using Ed25519 over RFC-8785 canonical JSON, and forwards the payload over TLS to the peer gateway1. The peer gateway verifies the signature against a Trust-On-First-Use (TOFU) pinned keyring before delivering the envelope onto its local bus1.

| Architectural Component | Implementation Location | Native Operational Behavior | Root Cause Failure Mechanism |
| :---- | :---- | :---- | :---- |
| **CLI Payload Builder** | crates/famp/src/cli/send/mod.rs | Hardcodes recipient authority to local.bus and payload class to audit\_log1. | Fails to capture remote peer domain qualifications and constructs fire-and-forget envelopes that cannot advance task state1. |
| **Local Bus Broker** | Unix-domain socket broker | Delivers messages based on local principal handles registered on the socket1. | Expects local proxy addresses (such as agent:local.bus/bob) to route messages into the gateway proxy mailbox1. |
| **Gateway Egress Engine** | crates/famp-gateway/src/egress.rs | Drains proxy mailbox and extracts envelope to principal for transport dispatch1. | Reads un-rewritten agent:local.bus/bob from the envelope and passes it directly to the transport lookup layer1. |
| **HTTP Transport Layer** | crates/famp-transport-http/src/transport.rs | Resolves target URLs via an internal address map populated from \--peer domains1. | Exact match lookup fails because the map contains agent:100.112.29.111/bob, yielding HttpTransportError::UnknownRecipient1. |
| **Receiver Task Engine** | Target Agent FSM Engine | Processes incoming structured requests (Request, Commit, Deliver) to advance state1. | Ignored by design when receiving audit\_log envelopes, preventing completion of UAT success criteria1. |

The addressing failure occurs when a user initiates a cross-host message via famp send1. The CLI function build\_envelope\_value unconditionally formats the destination principal as agent:local.bus/{name}, omitting any target domain qualification1. When this envelope enters the local bus, the broker successfully routes it to the gateway proxy mailbox because the gateway registered itself under local.bus for that agent1. However, when the gateway egress loop drains the envelope in relay\_one, it extracts the envelope's internal to field (agent:local.bus/bob) and attempts to send it via the transport layer1. The HTTP transport maintains an internal lookup map (addr\_map) created during startup by binding remote peer domain flags (--peer) to backed agent names1. Consequently, the map contains fully qualified remote principals such as agent:100.112.29.111/bob1. Querying this map with agent:local.bus/bob results in an unhandled recipient error, dropping the message entirely1.  
A secondary failure mechanism exists within the payload payload structure1. The CLI generator hardcodes the envelope class as audit\_log, which carries a Phase-2 code comment indicating it was a temporary stub1. By design, audit\_log envelopes are fire-and-forget constructs that do not trigger state transitions in the receiver's Task FSM1. Consequently, even if network delivery succeeded, the setup recipe in GATEWAY-SETUP.md §5 would fail to advance the task state machine to a terminal state (COMPLETED, FAILED, or CANCELLED), making UAT verification impossible through shipping tools1.  
This failure was masked during development because the automated integration test suite (e2e\_cross\_host\_delivery.rs) passes reliably in CI1. The integration test bypasses the CLI entirely by using a low-level test client (BusClient::connect\_no\_spawn) to manually construct raw BusMessage::Send instances with fully qualified target domains (to \= agent:hostb.test/bob) and typed Request payload classes1. While this proves that the gateway, signing engine, and transport pipeline operate correctly, no shipping user-facing binary exposes this capability1.

## **Evaluation of Candidate Fixes**

Five architectural candidate solutions were evaluated to resolve the addressing and routing breakdown1. Each candidate modifies a different layer of the messaging pipeline, introducing distinct tradeoffs regarding protocol integrity, maintenance overhead, and implementation risk1.

| Solution Candidate | Primary Modification Boundary | Envelope Header Invariance | Task FSM Compatibility | Local Bus Unsigned Invariant | Implementation Complexity |
| :---- | :---- | :---- | :---- | :---- | :---- |
| **C1: Gateway Egress Rewrite** | famp-gateway Egress Engine1 | Violated (Mutates envelope to field)1 | Incompatible (Retains audit\_log)1 | Preserved1 | Low1 |
| **C2: CLI Target Qualification** | famp CLI Parameter Parser1 | Preserved1 | Incompatible (Retains audit\_log)1 | Preserved1 | Low1 |
| **C3: Full Local Signing** | famp CLI & Envelope Engine1 | Preserved1 | Compatible (Emits Request)1 | Violated (Reintroduces local crypto)1 | High1 |
| **C4: Route-by-Binding Egress** | famp-gateway Transport Mapping1 | Preserved1 | Incompatible (Retains audit\_log)1 | Preserved1 | Low1 |
| **C5: Refined Target Decoupling** | famp CLI & Bus Dispatcher1 | Preserved1 | Compatible (Emits Request)1 | Preserved1 | Medium / Focused1 |

Candidate C1 proposes modifying the egress loop within famp-gateway so that any envelope carrying a local.bus recipient is dynamically rewritten to match the registered peer domain prior to signature generation1. This candidate must be rejected because it introduces architectural ambiguity when a gateway backs multiple agent names or connects to multiple remote peers1. Rewriting envelope headers inside the gateway also causes the egress proxy to synthesize destination principals that the original sender never specified, compromising cryptographic provenance1. Furthermore, C1 fails to update the stubbed audit\_log payload class, leaving the Task FSM state machine stalled1.  
Candidate C2 modifies the CLI to parse fully qualified domain targets or accept explicit \--domain arguments1. While this correctly places addressing responsibility on the sender, executing C2 in isolation causes a local bus routing failure1. If the CLI emits an envelope addressed directly to agent:100.112.29.111/bob, the local broker attempts to match that full principal string on the local socket1. Because the gateway proxy registers locally as agent:local.bus/bob, the local broker cannot deliver the message to the gateway proxy mailbox1. Additionally, C2 does not resolve the stubbed audit\_log class issue1.  
Candidate C3 re-implements local cryptographic envelope construction within the CLI, fulfilling the original Phase-4 design intent by constructing full signed Request and Deliver structures prior to local bus submission1. This candidate must be rejected because it reopens a settled architectural constraint1. FAMP v0.9 explicitly stripped cryptography from the local bus path to eliminate local key management complexity and performance overhead1. Requiring the CLI to sign envelopes locally compromises this boundary and introduces significant risk immediately prior to release1.  
Candidate C4 alters gateway egress logic to route messages based on proxy mailbox bindings rather than envelope headers1. Under C4, any message drained from a local proxy mailbox associated with agent bob is blindly dispatched to the peer gateway bound to bob, regardless of the envelope's internal to field1. This approach violates standard protocol semantics by divorcing transport delivery from message envelope headers1. It creates dangerous edge cases if an agent attempts local multi-hop messaging while attached to a proxy, and it fails to resolve the payload class defect1.  
Candidate C5 is the recommended solution1. It combines CLI domain qualification with local target decoupling and a payload class upgrade1. Under C5, the CLI accepts remote target definitions and builds an envelope whose canonical header contains the fully qualified destination principal (agent:{peer-domain}/{name}) and whose payload class is set to Request1. When submitting this envelope to the local broker, the client explicitly decouples the local routing handle (agent:local.bus/{name}) from the internal envelope destination1. The broker delivers the message into the local gateway proxy mailbox based on the local routing handle1. Gateway egress drains the mailbox, reads the fully qualified destination from the signed envelope header, resolves the peer transport URL, applies the Ed25519 signature across canonical JSON, and forwards the payload across the network1.

## **Invariant Analysis against System Constraints**

The recommended remediation (Candidate C5) was evaluated against the mandatory operational, cryptographic, and specification constraints governing the FAMP architecture1.  
The transaction pipeline under Candidate C5 begins at the sender CLI, where target inputs are parsed into a local dispatch handle and a remote canonical principal1. The local client generates an unsigned envelope containing a typed Request payload and submits it to the local broker1. The broker routes the envelope over the local Unix socket to the gateway proxy mailbox using the local dispatch handle1. The gateway egress loop drains the proxy mailbox, reads the fully qualified remote principal directly from the envelope header, attaches federation metadata, formats the structure as RFC-8785 canonical JSON, and applies an Ed25519 signature under the FAMP-sig-v1\\0 prefix1. The transport layer resolves the destination principal in its address map and posts the payload over TLS to the peer gateway1. Upon receipt, the peer gateway verifies the signature against its TOFU keyring, validates domain separation constraints, and delivers the payload to the receiving agent's local bus, successfully driving the Task FSM to completion1.  
This operational sequence satisfies invariant INV-10 regarding cryptographic signing and canonicalization1. No field within the envelope header or payload is mutated after generation or during gateway relay1. Because the CLI writes the final destination principal directly into the envelope during initial construction, the signature generated at the gateway egress boundary covers the exact byte payload received by the peer node1. Inbound verification (verify\_strict) on the remote gateway succeeds without canonicalization discrepancies or hash mismatches1.  
The local bus unsigned invariant established in v0.9 is fully preserved under Candidate C51. The CLI does not load private keys, generate digital signatures, or manage local key stores1. Messages traversing the local Unix socket remain plain, unsigned protocol envelopes1. Cryptographic boundaries remain localized strictly to the gateway edge, preserving the performance and simplicity of same-host agent communication1.  
Preservation of existing integration test suites is guaranteed under Candidate C51. The passing end-to-end integration test (e2e\_cross\_host\_delivery.rs) relies on constructing domain-qualified Request envelopes that mirror the structure produced by the updated CLI1. Because Candidate C5 updates production tooling to match the structural patterns already expected by the gateway transport layer, zero changes are required within famp-gateway or the test suite, keeping existing CI gates green1.  
Protocol specification fidelity to authority v0.5.2 is restored1. Specification v0.5.2 mandates that global principals maintain domain-qualified naming conventions and that inter-agent transactions utilize state-bearing payload classes1. Transitioning the CLI away from hardcoded local.bus addresses and audit\_log stubs brings client behavior into full compliance with the specification authority1.  
Continuous Integration enforcement and identity provenance requirements are completely maintained1. No compiler warnings, linting suppressions (clippy \-D warnings), or code formatting overrides are introduced1. Sender and receiver identity provenance remains immutable across the entire transmission path1. The remote gateway extracts the sender principal (from) directly from the verified envelope header, allowing public key lookups against the TOFU keyring (keyring.get(peek\_sender(bytes))) to operate without resolution failure1.

## **Implementation Plan and Remediation Roadmap**

Restoring end-to-end remote principal addressing requires targeted modifications isolated entirely to crates/famp1. No structural alterations are made to crates/famp-gateway, crates/famp-transport-http, or crates/famp-envelope1.  
The implementation modifies argument parsing and envelope generation in crates/famp/src/cli/send/mod.rs1. The command structure is expanded to parse domain-qualified targets provided via explicit flags or inline address strings1. The helper function build\_envelope\_value is refactored to return both the local broker dispatch target and the constructed envelope1. The stubbed audit\_log payload is replaced with a structured Request class containing a valid task identifier and initial state1.

Rust  
// Refactored envelope construction within crates/famp/src/cli/send/mod.rs

pub struct SendArgs {  
    pub target: String,  
    pub domain: Option\<String\>,  
    pub message: String,  
}

pub fn build\_envelope\_value(args: \&SendArgs, identity: \&str) \-\> Result\<(Principal, Envelope), Error\> {  
    let (target\_name, target\_domain) \= parse\_target(\&args.target, args.domain.as\_deref())?;  
      
    // Canonical remote principal placed into the signed envelope header  
    let envelope\_to \= format\!("agent:{target\_domain}/{target\_name}");  
    let envelope\_from \= format\!("agent:local.bus/{identity}");  
      
    // Local broker routing handle pointing to the gateway stand-in proxy  
    let local\_routing\_target \= format\!("agent:local.bus/{target\_name}").parse::\<Principal\>()?;

    let envelope \= EnvelopeBuilder::new()  
        .from(envelope\_from)  
        .to(envelope\_to)  
        .class("Request") // Replaces Phase-2 audit\_log stub to fire Task FSM  
        .payload\_json(serde\_json::json\!({  
            "task\_id": uuid::Uuid::new\_v4().to\_string(),  
            "state": "INITIATED",  
            "body": args.message  
        }))?  
        .build()?;

    Ok((local\_routing\_target, envelope))  
}

The bus dispatch call within the CLI execution path is updated to pass local\_routing\_target as the destination header for the local broker client, while embedding the constructed envelope within the message body1. This ensures the local broker places the envelope directly into the proxy mailbox (agent:local.bus/bob), while gateway egress reads the remote principal (agent:100.112.29.111/bob) from the envelope header1.  
Verification follows a three-stage testing sequence prior to tagging release v1.0.01. First, full workspace compilation and unit test execution are performed via cargo test \--all to verify that no regressions were introduced1. Second, the automated end-to-end test suite (e2e\_cross\_host\_delivery.rs) is executed to confirm gateway relay continuity1. Third, the manual Gate A dogfood deployment is re-executed across nodes opus and zed over Tailscale1. An operator executes famp send \--to bob \--domain 100.112.29.111 "test payload" from opus1. Verification confirms that the message routes across the gateway wire, verifies signatures against pinned keys on zed, posts to zed local bus, and successfully advances the receiving agent's Task FSM to state COMPLETED1.

## **Strategic Conclusion**

The release tag for v1.0.0 must wait until Candidate C5 is implemented and verified1. Attempting to ship v1.0.0 with a documented limitation undermines the core promise of the protocol milestone and delivers a broken user experience during human UAT1. Because the gateway, signing engine, and transport layers are fully verified and ground-truth proven, the required work is strictly limited to decoupling address targets and upgrading payload classes inside the CLI toolchain1. Executing Candidate C5 restores complete alignment between client toolchains, gateway routing, and protocol specifications while upholding all cryptographic and system invariants1. Upon completion of the Gate A dogfood verification sequence across the production environments, FAMP v1.0.0 can be tagged with full confidence in its inter-host interoperation capabilities1.

#### **Works cited**

> 1. DESIGN-BRIEF-v1-remote-addressing.md