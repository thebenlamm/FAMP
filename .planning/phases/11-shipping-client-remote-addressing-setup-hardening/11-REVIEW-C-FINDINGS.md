# Phase 11 — Final Design Review (Report C) findings, source-verified

**Source:** `DESIGN-REVIEW-C-final.pdf` ("FAMP v1.0 Remote Addressing: Final Design Review"), commissioned 2026-07-28.
**Verification:** every claim below was checked against actual source with file:line evidence, then the two security-tier claims were independently re-confirmed by the orchestrator. AI reviewers false-positive; these did not.

**Status:** cataloged, NOT yet planned. Ordering below is by blast radius (security → data integrity → config → correctness), per the project audit rule.

---

## Decisions already taken from Report C

| Item | Decision | Rationale |
|------|----------|-----------|
| C5 adoption (complete federated principals, out-of-band local dispatch) | **Adopt** — already what 11-02/11-03 build | Converges with prior Reports A + B |
| C3 / §15 — don't fold task FSM into `famp send` | **Overridden — keep the mode-branch** (Ben, 2026-07-28) | Report's own stated exception applies: UAT-01 explicitly commits to a user-facing terminal FSM. Mode-branch was review HIGH #1's fix; dropping it re-opens "a request never advances past REQUESTED". Revisit the `famp request` surface post-v1.0. |
| Release ruling — `v1.0.0-rc.1` first, then two-machine test, then tag v1.0.0 against the §16 9-item checklist | **Accept** — retarget 11-06 | 11-06 currently goes straight to v1.0.0 |

---

## F-1 — Inbound destination domain is never validated (INV-H) — **CONFIRMED, worst**

**Blast radius: security.** Affects every gateway operator.

`crates/famp-gateway/src/ingress.rs:167` parses the recipient from the **URL path**; `ingress.rs:225` uses `recipient.name()` as the bus target. The envelope's own `to` field is read nowhere in `ingress.rs`. `crates/famp-gateway/src/verify.rs:58-67` does sender-pin + signature only.

Worse than the report claimed: there is no check that `envelope.to == recipient` **either**. A verified envelope addressed to `agent:X/alice`, POSTed to `/famp/v0.5.1/inbox/agent:Y/bob`, lands in mailbox `bob`. The only gates are (a) `from` is pinned, (b) `from`'s bare name is backed here.

The check cannot even be written today: `grep -rn 'local_domain|own_domain|my_domain' crates/famp-gateway/src/` returns **nothing**. The gateway holds no concept of its own domain.

Also dead: `SignedEnvelope::federation_format_ok` (`crates/famp-envelope/src/envelope.rs:531`) has **zero callers** outside its own tests — inbound `nonce`/`expiry` are never format-checked.

**Impact:** any pinned peer can plant a signed envelope into any local mailbox, or use the gateway as an open relay. A message the sender provably addressed to A can sit in B's mailbox while still displaying `to: A`.

**Fix sketch:** add a required gateway local-domain config; after verification reject unless `envelope.to.authority() == local_domain` **and** `envelope.to == recipient`, with a distinct 4xx. Wire `federation_format_ok()` into the ingress gate.

**Note:** plan 11-07 already introduces a gateway-side own-domain for the egress `from.authority()` check — F-1 should reuse that same config value, not add a second one.

---

## F-2 — Gateway signs client-supplied federation fields (INV-F) — **CONFIRMED (5 of 6 fields + 2 more)**

**Blast radius: security.** Affects every receiving federation peer. Composes with F-1.

`crates/famp-gateway/src/egress.rs:107-117` inserts every federation field with `entry(...).or_insert_with(...)` — **preserve-if-present**, not overwrite — then signs the result at `egress.rs:124`:

```rust
obj.entry("from_domain".to_string()).or_insert_with(|| ...);
obj.entry("to_domain".to_string()).or_insert_with(|| ...);
obj.entry("sender_key_id".to_string()).or_insert_with(|| ...);
obj.entry("nonce".to_string()).or_insert_with(|| ...);
obj.entry("expiry".to_string()).or_insert_with(|| ...);
```

These fields are legal on the local bus (`crates/famp-envelope/src/wire.rs:60-73` declares them as `Option` members, so `deny_unknown_fields` permits them). `BusEnvelope::decode` (`crates/famp-envelope/src/bus.rs:49-88`) rejects only `signature`. So a local agent speaking the UDS protocol sets all five and the gateway signs them verbatim with its federation identity.

**Wider than the report stated:** `capability` and `approval` are also client-settable and never touched by `sign_federation_fields` — a local agent can pre-plant capability/approval claims that arrive gateway-signed.

**`signature` sub-claim REFUTED**, but only incidentally: `egress.rs:96-102` early-returns on an existing signature, and that path is unreachable because `AnyBusEnvelope::decode` (`crates/famp-bus/src/broker/handle.rs:1141-1145`, called from `drain_walk.rs:121`) hard-rejects any line carrying `signature`. Defense is BUS-11's, not egress's — a client-planted signature wedges the mailbox line permanently undrainable rather than being cleanly rejected.

Note the write path does not validate: `encode_envelope` (`handle.rs:1016-1035`) only canonicalizes and size-checks. The gate is drain-side only.

**Fix sketch:** replace every `entry().or_insert_with()` with unconditional `insert()`, and `remove()` `capability`/`approval` — or better, reject the envelope outright if any of the 7 keys is present on a locally-originated drain, so the sending agent learns it did something illegal.

---

## F-3 — Route map is a peer × name cross-product (§6) — **CONFIRMED**

**Blast radius: security, bordering config.** Affects any deployment with ≥2 peers or ≥2 backed names.

`crates/famp-gateway/src/main.rs:255-261` nests peers × backed names:

```rust
for (domain, url) in &args.peers {
    for name in &backed_names {
        if let Ok(principal) = format!("agent:{domain}/{name}").parse::<Principal>() {
            transport.add_peer(principal, url.clone()).await;
        }
    }
}
```

`--peer alpha=… --peer beta=… bob carol` makes all four of `agent:alpha/bob`, `agent:alpha/carol`, `agent:beta/bob`, `agent:beta/carol` routable. Lookup is exact-match on the full principal (`crates/famp-transport-http/src/transport.rs:128`), so every fabricated entry is live. Because `to` is client-controlled (the broker never stamps it — `handle.rs:384-392`), these fabricated bindings are reachable by a local agent, not only by operator misconfiguration.

No per-principal binding syntax exists in `parse_args` (`main.rs:101-180`) or `docs/GATEWAY-SETUP.md:112-120`.

**Fix sketch:** add an explicit binding form (`--route agent:<domain>/<name>=<url>`, or require fully-qualified positional names) and populate `addr_map` only from declared bindings.

---

## F-4 — Duplicate route config silently last-write-wins (INV-J) — **CONFIRMED**

**Blast radius: config → security.** Explicitly named in the report's §16 v1.0.0 tag checklist ("ambiguous route configuration fails closed").

`crates/famp-gateway/src/main.rs:148` pushes into a `Vec<(String, Url)>` with no dedup; validation covers only missing `=`, empty domain, unparseable URL (`main.rs:138-147`). Downstream `crates/famp-transport-http/src/transport.rs:80-82` uses `HashMap::insert` — last write silently wins. `--peer alpha=https://A --peer alpha=https://B` resolves to B with no warning, and reversing flag order silently redirects every destination for that domain.

Test suite locks in only that repetition parses (`main.rs:352-368 peer_flag_is_repeatable`); no duplicate-domain test exists.

**In-repo precedent for the fix:** the trust plane already fails closed — `crates/famp-keyring/src/lib.rs:140-145` returns `KeyringError::KeyConflict` on a conflicting re-pin. The route plane simply lacks the equivalent.

**Fix sketch:** error in `parse_args` on a repeated domain; optionally make `add_peer` return a conflict error mirroring `pin_tofu`'s shape.

---

## F-5 — Bare-name proxy mailboxes (§8.1) — **PARTIAL (the "undetected" part is refuted)**

**Blast radius: correctness / availability, with a security edge.**

**Refuted:** the collision is NOT silent. `crates/famp-bus/src/broker/handle.rs:305-315` rejects a name held by a different live client with `BusErrorKind::NameTaken`; the gateway's proxy registration goes through that same path (`crates/famp-gateway/src/principal.rs:42-56`) and a failure aborts startup (`main.rs:193-198`). `GatewayRegistry::back` also rejects intra-process duplicates (`registry.rs:29-31`). It fails closed.

**What is true:** the gateway backs remote principals by **bare name with the domain discarded** (positional args are bare names, `main.rs:200`; registry keyed by bare `String`, `registry.rs:19`; confirmed by `tests/e2e_cross_host_delivery.rs:667`). So local-vs-remote collision is order-dependent mutual exclusion: local `bob` first ⇒ gateway `exit(1)`; gateway first ⇒ the human's local `bob` can never register. The error message names neither the gateway nor the remote domain.

**Second, genuinely ambiguous case:** `ingress.rs:218` selects the stand-in with `guard.get_mut(sender.name())`, discarding `sender.authority()`. Two pinned remote principals sharing a bare name (`agent:alpha/bob`, `agent:beta/bob`) are conflated onto one stand-in connection, and one gateway can never back both.

**Fix sketch:** namespace proxy mailbox names so local `bob` and remote `agent:hostB/bob` coexist; key `GatewayRegistry` by full `Principal`; look up ingress stand-ins by the full verified sender. At minimum, make the `NameTaken` abort message explain the gateway/remote-domain collision.

---

## Sequencing note

F-2, F-3, F-4 touch `crates/famp-gateway/src/egress.rs` and `crates/famp-gateway/src/main.rs` — **the same files plan 11-07 modifies**, and F-1 needs the very gateway own-domain config 11-07 introduces. Landing these as a sibling plan while those files are hot is materially cheaper than a second pass after the UAT.

F-5's residual (ingress `sender.name()` conflation, error-message quality) is the only one that is not release-gating on the report's own §14 list and could defer.
