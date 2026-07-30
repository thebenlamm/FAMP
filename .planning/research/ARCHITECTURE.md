# Architecture Research: FAMP v1.1 Open-Internet Federation

**Domain:** Protocol reference implementation — adding open-internet federation to a shipped Layer 0/1/2 Rust workspace
**Researched:** 2026-07-30
**Confidence:** HIGH (all findings grounded in the actual v1.0.0 tree, file:line cited; no speculative library research needed — this is a brownfield integration question)

## Standard Architecture (current, as shipped at v1.0.0)

```
┌──────────────────────────────────────────────────────────────────────┐
│ Layer 2 — Federation (per-milestone, cross-host)                     │
│  famp-gateway (binary): registry.rs, principal.rs, ingress.rs,       │
│  egress.rs, verify.rs, error.rs, main.rs                             │
│  Depends on: famp-transport-http, famp-keyring, famp-transport,      │
│  famp-crypto, famp-envelope, famp-bus (client), famp (bus_client)    │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 1 — Local bus (same-host, same-UID)                            │
│  famp-bus (broker actor, tokio-free — `just check-no-tokio-in-bus`)  │
│  famp-inbox (durable JSONL), famp-taskdir                             │
│  famp-inspect-{proto,server,client} (read-only side-channel)          │
├──────────────────────────────────────────────────────────────────────┤
│ Layer 0 — Protocol primitives (FROZEN this milestone)                 │
│  famp-canonical, famp-crypto, famp-core, famp-envelope, famp-fsm      │
├──────────────────────────────────────────────────────────────────────┤
│ CLI/MCP surface — crates/famp (binary): cli/mcp/server.rs (12 tools), │
│  cli/peer/{export,import,identity}.rs, cli/send, cli/inbox, cli/await │
└──────────────────────────────────────────────────────────────────────┘
```

Only Layer 1 and Layer 2 (and any brand-new crate) are legal build targets this milestone. Every answer below names which existing file is touched or which new module/crate is introduced, and confirms it never lands in `famp-canonical` / `famp-crypto` / `famp-core` / `famp-envelope` / `famp-fsm`.

---

## Q1 — Reachability layer placement (relay / NAT traversal)

**Answer: (b) — a sibling of `run_ingress`/`run_egress` inside `famp-gateway`, composed under the same `tokio::select!` in `main.rs`. Not (a), not (c).**

Why not (a) a new `Transport` trait impl: `famp-transport-http::HttpTransport` (used by egress, `crates/famp-gateway/src/egress.rs:30,319`) does direct HTTPS POST to a peer's `--peer <domain>=<url>` base URL (`crates/famp-gateway/src/main.rs:269-320` `build_route_map`). A relay/NAT-traversal path changes *how the TCP/TLS connection to the peer is established or proxied*, not *what gets POSTed once connected* — a relay is a routing/dialing concern, not a serialization concern, so bolting it onto `Transport` would smuggle infra-plumbing into a trait whose only job today is "send these signed bytes to this URL." `HttpTransport` can stay the wire format; only its *dial* step needs a relay-aware path (e.g. dial via a relay-assigned rendezvous URL instead of a direct peer URL) — that's a `--peer` URL-resolution concern already local to `main.rs`'s route map (`main.rs:269`), or a thin wrapper the egress task's `transport.send(...)` call goes through.

Why not (c) an entirely separate process: `famp-gateway` already owns exactly one `Arc<Mutex<GatewayRegistry>>` shared between `run_ingress` (`ingress.rs:370`) and every per-principal `run_egress` task (`egress.rs:315`), composed in one `tokio::select!` (`main.rs:441-455`). A relay client is not a new trust decision — it just gets bytes to/from the same ingress/egress pair. Running it as a separate process means a second thing to keep alive, a second socket/registry to keep in sync, and (critically) a second place someone could accidentally re-verify or re-trust inbound bytes, undermining the "one gateway process, one registry, one verification site" invariant the v1.0 design leans on (documented at `ingress.rs:1-18`, "NOT `famp_transport_http::build_router`... a second, forbidden trust source").

**Why (b) preserves `verify_inbound_any` as the sole trust decision:** `verify_inbound_any` (`crates/famp-gateway/src/verify.rs:58-67`) is called exactly once, at `ingress.rs:234`, inside `inbox_handler` — *after* the HTTP body has been received, regardless of how that connection was dialed or relayed. A relay (whether a dumb TCP/TLS-passthrough rendezvous, or a NAT-hole-punch handshake) sits entirely below this call: it changes how bytes arrive at the axum router in `build_gateway_router` (`ingress.rs:72-86`), never what happens to them once they do. As long as the relay path terminates back into the same `inbox_handler`/`verify_inbound_any` call (e.g. the relay forwards to the gateway's existing HTTPS listener, or the gateway dials out to the relay and the relay proxies inbound HTTPS to it), there is exactly one verification site, unchanged. The risk of a second trust path only appears if a relay-specific handler is added that bypasses `inbox_handler` (e.g. a raw TCP proxy that speaks a different wire dialect and hand-rolls its own signature check) — that must not happen; **any relay-terminated connection MUST still resolve to axum's existing `POST /famp/v0.5.1/inbox/{principal}` route** (`ingress.rs:83`, reusing `famp_transport_http::INBOX_ROUTE`).

**What must NOT change in `famp-bus`:** nothing. The relay operates strictly above `famp-gateway`'s registry (`registry.rs`) and below its HTTP client/server boundary (`egress.rs`/`ingress.rs`). `famp-bus` (`crates/famp-bus/src/*`) has zero awareness of cross-host topology today — `ProxiedPrincipal::register` (`crates/famp-gateway/src/principal.rs:37-57`) talks to the broker over a plain UDS `Register` exactly like any local session. A relay changes nothing about that UDS conversation. This also keeps `just check-no-tokio-in-bus` trivially satisfied — the relay client is itself tokio-based HTTP/TCP code living entirely inside `famp-gateway`, which already depends on tokio.

**Concrete integration point (new code):** a new `famp-gateway/src/relay.rs` (or `dial.rs`) module, consumed by `egress.rs`'s `relay_one` (`egress.rs:248`) when resolving the peer URL, and/or a relay-aware bind step ahead of `run_ingress`'s `std::net::TcpListener::bind` (`ingress.rs:385`) if the chosen model is "gateway dials out to a public relay and receives a reverse-proxied inbound stream" rather than "gateway binds a directly-reachable public port." Which of these two shapes is correct is exactly what the milestone's Phase-13 zero-code spike (PROJECT.md's target-features list) must decide before any of this is written — this research answers *where the code goes*, not *which relay model to buy*.

---

## Q2 — Trust bootstrap (`famp peer export`/`import`, keyring extension)

**Current location:** `crates/famp/src/cli/peer/{mod.rs, export.rs, import.rs, identity.rs}`. `export.rs:124-126` formats a 3-field line (`<principal> <pubkey-b64url> <key_id>`); `import.rs:44-86` parses it and calls `Keyring::pin_tofu` (`crates/famp-keyring/src/lib.rs:135-148`), then `Keyring::save_to_file` (`lib.rs:90-98`) to `~/.famp/gateway/peers.keyring` (`identity.rs:28-30`). This is the exact file `verify_inbound_any` reads at gateway startup (`main.rs:374-383`).

**Minimum-blast-radius place for a new bootstrap mechanism (short code / QR / PAKE / directory fetch):** add new CLI subcommands as siblings under `crates/famp/src/cli/peer/mod.rs:35-46` (`PeerSubcommand::Export`/`Import` today) — e.g. `PeerSubcommand::Bootstrap` or a new `peer/pake.rs`/`peer/directory_fetch.rs` module — that all terminate at the SAME two calls: `Keyring::pin_tofu` + `Keyring::save_to_file`. This is the smallest possible surface because the keyring itself is agnostic to *how* a `(Principal, TrustedVerifyingKey)` pair was obtained; `export.rs`/`import.rs` are just one on-ramp. A short-code or QR mechanism only needs to produce the same `(Principal, TrustedVerifyingKey)` tuple that `parse_export_line` (`import.rs:97-128`) already produces, then feed it through the identical `pin_tofu` call. A PAKE flow additionally needs a live two-way handshake (unlike the current one-shot copy/paste blob) — that handshake logic is new code (likely a new `famp-keyring`-adjacent module or a small new crate, see below), but its *output* still lands through the same `pin_tofu`/`save_to_file` pair, so the on-disk format and the gateway-side consumer (`ingress.rs`/`verify.rs`) do not change at all.

**Does the on-disk keyring format survive multiple keys per peer and key rotation? No — it needs extending, and this is the sharpest finding in this section.**

`Keyring` (`crates/famp-keyring/src/lib.rs:41-44`) is `HashMap<Principal, TrustedVerifyingKey>` — **one key per principal, full stop**. Three separate places encode this as a hard invariant, not an oversight:
- `pin_tofu` (`lib.rs:135-148`): a second *different* key for an already-pinned principal is `Err(KeyringError::KeyConflict)` — there is no rotation path, "any conflict is fatal" (module doc, `lib.rs:5`).
- `load_from_file` (`lib.rs:55-85`) additionally rejects a **duplicate pubkey across two different principals** (`KeyringError::DuplicatePubkey`, `lib.rs:75-80`) — so key *sharing* across names is also structurally impossible.
- `famp peer export`'s own module doc (`export.rs` is documented at `mod.rs:14-19`) states this explicitly as a v1.1-deferred scope note: *"the trust model here is one signing key per remote principal name... generalizing this is deferred to v1.1."*

So key rotation and multi-key-per-peer are both **out of scope for the current format** and must be added. The minimum viable extension, without forking the file grammar (`file_format.rs:1-19`, two-whitespace-separated-field grammar) or the save format (alphabetical, two-space separator, `file_format.rs:70-74`):
1. Change `Keyring`'s map value from `TrustedVerifyingKey` to `Vec<TrustedVerifyingKey>` (or a small `PinnedKeys { active: TrustedVerifyingKey, retired: Vec<TrustedVerifyingKey> }` struct) — a Layer-2 (`famp-keyring`) change, not Layer 0, so it is in-scope this milestone.
2. `pin_tofu` gains an explicit `rotate_to(principal, new_key)` sibling that appends rather than conflicts, and marks the previous key retired-but-still-verifiable for some grace window (so in-flight envelopes signed under the old key still verify — see Q4).
3. `file_format.rs`'s one-line-per-entry grammar needs either multiple lines per principal (requires relaxing `load_from_file`'s `DuplicatePrincipal` reject at `lib.rs:68-73`, since a second line for the same principal is now a **second key**, not a conflict) or a new per-entry field (`key_id` + `status: active|retired`) appended after the existing two whitespace-separated fields. The latter is lower blast radius: it is purely additive to `parse_line`/`serialize_entry` (`file_format.rs:32-74`) and does not touch the `DuplicatePrincipal` uniqueness gate.
4. `verify_inbound`/`verify_inbound_any` (`crates/famp-gateway/src/verify.rs:37-67`) call `keyring.get(&from)` expecting exactly one key — this becomes "try each pinned key for `from` in turn," a small, local, non-breaking change to `verify.rs` only.

This is genuinely new work, not a reinterpretation of existing code — flag it as its own roadmap phase (see Build Order).

---

## Q3 — Protocol-grade ingress: freshness, replay cache, audience binding, DoS ordering

**Current call chain inside `inbox_handler` (`crates/famp-gateway/src/ingress.rs:225-351`), in the exact order it runs today:**

1. `Path::<String>` extraction + `Principal::from_str` (`ingress.rs:230-232`) — cheap, no crypto, no I/O.
2. `verify_inbound_any(&body, &state.keyring)` (`ingress.rs:234`) — this is `peek_sender` (unverified, cheap `from`-field extraction, `verify.rs:62`) → keyring lookup (`verify.rs:63-65`, hard-reject on unpinned, zero mutation) → `AnySignedEnvelope::decode` which runs Ed25519 `verify_strict` (`verify.rs:66`, and `famp-envelope/src/envelope.rs:308-321,346-349`). **This is the ONLY signature check on the path** (module doc, `ingress.rs:12`).
3. `envelope_to != recipient` (misaddressed-recipient) check (`ingress.rs:253-264`).
4. `own_domain` authority check (`ingress.rs:265-283`).
5. `envelope_federation_format_ok(&envelope)` (`ingress.rs:284-287`) — calls `SignedEnvelope::federation_format_ok` (`famp-envelope/src/envelope.rs:519-557`), a **format-only** well-formedness check on `nonce`/`expiry` (non-empty nonce; expiry strictly after `ts`; canonical UTC form) — explicitly NOT an active replay/expiry enforcement (doc comment at `envelope.rs:519-524`: "This does NOT reject an expired (past) `expiry` and does NOT consult any replay cache — active anti-replay + expiry rejection are v1.1 concerns").
6. Re-parse to `Value`, `strip_relay_fields`, registry-lock-scoped `BusMessage::Send` (`ingress.rs:296-338`) — the only state mutation on the whole path, happening only after every check above has passed.

**Where new checks slot in, and why the order matters for DoS:**

The project's own stated principle — "cheap checks precede signature verification; signature verification precedes anything that mutates state" — is *already* the shape of this handler, with one caveat: currently NOTHING precedes step 2 except the free `Principal::from_str` parse. A malicious sender can force step 2's Ed25519 `verify_strict` to run against a garbage-but-well-pinned-looking sender name, or (worse) against an unpinned name — the keyring lookup at `verify.rs:63-65` happens BEFORE `decode`/`verify_strict`, so an unpinned-sender flood is already cheap-rejected before any signature math runs. That ordering is correct and must be preserved.

New v1.1 checks, ordered by cost, all inserted as **new gates inside `inbox_handler`, or as a new pre-check module `famp-gateway/src/ingress_guard.rs`** (never inside `famp-envelope`, which is frozen):

1. **Body-size cap** — already exists (`ONE_MIB` limit layer, `ingress.rs:44-47,85`) and already runs before axum even calls the handler. No change needed, just confirm it stays first.
2. **Bounded replay/nonce cache lookup** — this MUST run **before** `verify_inbound_any`'s expensive `verify_strict` call if the goal is pure DoS-cost minimization (a replayed-envelope flood shouldn't pay Ed25519 verify cost twice), BUT the nonce is only trustworthy once its authenticity is established — a nonce lookup keyed on an *unverified* `(from, nonce)` pair is itself a cheap hash-map probe (not a crypto operation), so it is safe and correct to place it **between step 1 (principal parse) and step 2 (verify_inbound_any)**, keyed on the peeked-but-unverified `from` (via `famp_envelope::peek_sender`, already used internally by `verify.rs:41,62`) plus a peeked `nonce` field. A hit on the replay cache is a same-cost-as-parse reject, strictly cheaper than a signature check — genuinely helps DoS ordering. However, an attacker who does NOT know a valid nonce cannot force cache evictions this way (a bounded LRU/TTL cache with a fixed slot budget), so this is safe to place pre-signature. **Concretely:** new pre-check between `ingress.rs:232` and `ingress.rs:234`, backed by a new small in-process cache (bounded, e.g. `lru`/`moka` crate or a hand-rolled ring buffer — new dependency, Layer 2 only) owned by `famp-gateway`'s `GatewayIngressState` (`ingress.rs:51-61`) or a sibling `Arc<Mutex<ReplayCache>>` alongside the existing `registry`/`keyring` fields.
3. **Expiry/freshness ENFORCEMENT (not just format-check)** — `federation_format_ok`'s job stays format-only (frozen crate); the *enforcement* ("reject if `expiry` is in the past relative to wall-clock now, or `ts` is too far in the past/future") is new logic that must run **after** `verify_inbound_any` succeeds (you must trust `ts`/`expiry` came from the claimed sender before making a decision based on them) but **before** the registry lock / `BusMessage::Send` (state mutation). Concretely: a new check inserted between `ingress.rs:287` (existing `federation_format_ok` reject) and `ingress.rs:296` (re-parse + Send), living in `famp-gateway` (e.g. `verify.rs` gains a new `freshness_ok(&envelope) -> bool` sibling function, or a new `ingress_guard.rs`).
4. **Audience binding** — this is mechanically what `MisaddressedRecipient` (`ingress.rs:253-264`) and `ForeignDomain` (`ingress.rs:265-283`) already do for the *envelope-level* `to`/domain. If v1.1's "audience binding" means something stronger (e.g. binding to a specific gateway instance ID, or a capability audience once FAMP-Sec lands post-v1.1), it slots in the same place, right after those two existing checks (`ingress.rs:283`), still before `federation_format_ok`.
5. **Registry lock + `Send`** — stays last, unchanged (`ingress.rs:317-338`).

**Where `federation_format_ok` lives vs. where new validation goes:** `federation_format_ok` (`famp-envelope/src/envelope.rs:537-557`) is a method on `SignedEnvelope<B>` inside the frozen `famp-envelope` crate — it cannot gain new enforcement logic (e.g. "compare `expiry` against `SystemTime::now()`") without touching a frozen crate, and it deliberately does not (its own doc says so). **All new v1.1 validation — replay cache, active expiry enforcement, audience binding beyond what already exists — lives in `famp-gateway`**, either as new standalone functions in `verify.rs` (parallel to `verify_inbound`/`verify_inbound_any`) or a new `famp-gateway/src/ingress_guard.rs` module that `inbox_handler` calls after `verify_inbound_any` succeeds. This keeps `famp-envelope`'s job as "is this envelope syntactically/cryptographically well-formed" and `famp-gateway`'s job as "is this envelope trustworthy *right now*, in *this* trust-boundary process" — which matches the existing division of labor (crypto verification in `famp-envelope`, everything policy-shaped in `famp-gateway`, per the module docs at `verify.rs:1-19`).

---

## Q4 — Key revocation

**Minimum architecture, given the current keyring + TOFU design:**

Today `Keyring` has no revocation concept at all — a pinned key is trusted forever, or until a human manually edits/deletes the `peers.keyring` file (`identity.rs:28-30`) and re-imports. There is also no expiry on trust itself (distinct from envelope `expiry`, which is per-message, not per-key).

The minimum v1.1 shape, building on the multi-key extension from Q2:
1. Extend the keyring entry (once it supports multiple keys per principal, Q2) with a `status: active | revoked` tag per key, or a separate small revocation-list file (`~/.famp/gateway/revoked.keyring`, same grammar as `peers.keyring`) that `verify_inbound`/`verify_inbound_any` consult.
2. **Where the check is:** immediately inside the keyring lookup step of `verify_inbound`/`verify_inbound_any` (`verify.rs:42-44` and `63-65`) — `keyring.get(&from)` becomes "get the active key for `from`, and separately check it isn't on the revoked list," rejecting with a NEW `RejectReason` variant (e.g. `RejectReason::RevokedKey { principal, key_id }`) distinct from `UnpinnedKey` (operators need to tell "never trusted" apart from "was trusted, now isn't" — same D-08 two-reason-split philosophy already used for `InvalidSignature` vs `UnpinnedKey`, `error.rs:56-71`).
3. **Revocation propagation is the hard part, not the check.** Since trust is bilateral, hand-copied, and local-file-based (no directory yet — see Q7), a revocation has to be either (a) manually re-imported by both sides out-of-band (mirrors the existing `peer export`/`import` bootstrap — lowest new-code cost, but requires the revoking party to notice and act), or (b) distributed via the new signed peer directory (Q7) once that exists, which is the more robust long-term answer but creates a real ordering dependency: **revocation-via-directory cannot ship before the directory itself.**
4. **In-flight envelopes:** an envelope signed by a key BEFORE revocation and delivered AFTER revocation should be rejected — this is a normal "revoked key means immediately untrusted for all future verification," no grace period, matching TOFU's fail-closed philosophy (`Keyring` module doc, `lib.rs:5`: "any conflict is fatal"). If a grace window is wanted (e.g. to let in-flight messages signed just before rotation complete), that's the `retired` bucket from Q2's `PinnedKeys` shape — retired keys still verify but log a warning, revoked keys never verify. This distinction (retired vs. revoked) should be made explicit in the data model rather than conflated, since they have opposite security postures (retired = "trust it a little longer," revoked = "never trust it again").

---

## Q5 — Inbound-content-is-DATA boundary (exhaustive surface enumeration)

This is the milestone's blocking gate, so every surface is listed with its exact rendering call site.

**The structural problem underlying all of this:** the raw envelope `Value` — including its `body` — is what gets stored in the recipient's durable mailbox and is what every read surface hands back verbatim. Trace:

- `famp-gateway`'s ingress handler builds `BusMessage::Send { to, envelope: value }` (`ingress.rs:328-334`) where `value` is the re-parsed, relay-field-stripped, but otherwise UNTOUCHED envelope `Value` (content-transparency, `ingress.rs:289-296,303-315`).
- The broker's `send`/`send_agent` handler appends that exact envelope `Value` (serialized to a JSONL line) directly into the recipient's mailbox (`crates/famp-bus/src/broker/handle.rs:427,438-460`, `Out::AppendMailbox`).
- **Critically, `strip_relay_fields` (`ingress.rs:154-160`) removes `from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry`/`capability`/`approval`/`signature` — the ONLY fields that could have signaled "this came from a remote host" are deliberately erased before the local `Send`, because `BusEnvelope::decode` hard-rejects a `signature` key (BUS-11, `famp-envelope/src/bus.rs:53-56,49-88`) and the bus is meant to look identical regardless of origin.** The practical consequence: **once an envelope lands in a local mailbox, there is currently NO signal anywhere distinguishing a locally-authored message from a gateway-relayed (cross-host, untrusted-origin) one.** This is not a bug in v1.0 — v1.0 never needed the distinction, since federation was Ben-to-Ben. It is a real gap for v1.1's blocking gate: **provenance tagging cannot be done by reading the stored envelope bytes, because the only fields that would carry it are exactly the ones already stripped.** See "Design implication" below.

**Every surface that renders remote message content, enumerated:**

1. **`famp_inbox` MCP tool** — `crates/famp/src/cli/mcp/tools/inbox.rs:161-190`. Builds `{"task_id":..., "thread_state":..., "envelope": env}` where `env` is the raw stored `Value` (line 187, `"envelope": env`) — full `body` included, verbatim, straight into the MCP tool-result JSON the agent reads. **This is the single highest-exposure surface**: it is the intended, everyday path an agent uses to read a peer's message.
2. **`famp_await` MCP tool** — `crates/famp/src/cli/mcp/tools/await_.rs:90-94`. Returns `{"envelopes": out.envelopes, ...}` where `out.envelopes` is the raw `Vec<Value>` from `BusReply::AwaitOk` (`famp-bus/src/proto.rs:233-237`), unmodified.
3. **`famp_channel_log` MCP tool** — `crates/famp/src/cli/mcp/tools/channel_log.rs:21-70` (`read_channel_mailbox`, called at line ~30-40) reads the raw `.jsonl` channel mailbox file and returns entries containing full envelope content.
4. **`famp inbox` CLI (stdout)** — `crates/famp/src/cli/inbox/list.rs:57-78`, specifically `serde_json::to_string(env)` at line 64 followed by `writeln!` at line 68: one JSONL line per envelope, printed directly to stdout — read by any harness (Codex, Grok, a human) that captures Bash-tool stdout.
5. **`famp await` CLI (stdout)** — `crates/famp/src/cli/await_cmd/mod.rs:191` (`serde_json::to_string(&wrapper)`), same pattern: raw envelope JSON to stdout.
6. **Stop-hook wake notification text** (`crates/famp/assets/famp-await.sh:787-812` bash path, and the Rust-native equivalent `crates/famp/src/cli/hook/emit.rs:138-160` `build_reason`) — **this one is ALREADY a structural quarantine, and it is worth naming explicitly as the pattern to replicate elsewhere.** The `REASON`/`reason` string that becomes the actual `{"decision":"block","reason":...}` payload the agent sees carries ONLY: a fixed instructional sentence, the message count, and the `sender` name (regex-validated to `^[A-Za-z0-9@._:/-]{1,128}$` at `famp-await.sh:782` / `validate_sender` at `emit.rs:85-88`). **Peer-controlled envelope BODY bytes never reach this string** — the shell script's own comment says so explicitly (`famp-await.sh:788-789`: "SECURITY: peer-controlled envelope bytes are NOT included in reason"). This is the one surface that already does what Q5 demands; v1.1's job is to extend the same discipline to surfaces 1-5.
7. **Channel logs on disk** (`~/.famp/mailboxes/<channel>.jsonl`, written via `famp-inbox::append`, read back by `channel_log.rs` above and by `famp inspect messages`) — same raw-content storage as agent mailboxes; not a distinct code path, same finding as #3.
8. **`famp inspect messages` CLI/MCP** (`crates/famp/src/cli/inspect/messages.rs`) — **already safe by design**, not a hole: per the v0.10 design (`ARCHITECTURE.md`'s deferred non-goals, and the crate's own stated contract), this surface deliberately surfaces `byte_len` + `sha256_prefix` instead of body content (metadata-only, privacy-by-construction). Confirmed no body rendering here — listed for completeness of the audit, not as a gap.
9. **`famp_verify` MCP tool** (`crates/famp/src/cli/mcp/tools/verify.rs`) — returns delivery-confirmation booleans/metadata (`envelope_matches`, `scan_files`), not body content. Not a rendering surface.
10. **`famp_peers`/`famp_whoami`/`famp_register`/`famp_join`/`famp_leave`/`famp_set_listen`/`famp_inspect_waiters` MCP tools** — none of these read or render mailbox/envelope body content (confirmed by grep across `crates/famp/src/cli/mcp/tools/*.rs`); out of scope for this gate.
11. **`famp send` output (CLI/MCP)** — renders only the LOCAL agent's own outgoing content and delivery confirmation, never remote content; not a gate surface, but worth noting since it's adjacent code.

**Design implication for the quarantine (the actual architectural recommendation):**

Because federation-origin fields are deliberately stripped before the local bus write (BUS-11, `ingress.rs:303-315`), and because `famp-envelope` is frozen, **the provenance tag cannot be added to the envelope `Value` itself** without either (a) reopening the frozen envelope crate, or (b) breaking BUS-11 decodability for older clients. The correct place is **one layer up, in `famp-bus`'s wire protocol (Layer 1, not frozen):**

- `BusMessage::Register` (`famp-bus/src/proto.rs:131-148`) already has precedent for additive, backward-compatible optional fields (`cwd`, `listen`, both `#[serde(default, skip_serializing_if=...)]`) — a new optional `origin: Option<&str>` (e.g. `"gateway-relay"`) on `Register` would let `ProxiedPrincipal::register` (`famp-gateway/src/principal.rs:42-47`) tag its own connection as gateway-backed, using the identical additive-field pattern already proven twice on this struct.
- The broker (`famp-bus/src/broker/handle.rs`), which already resolves `effective_identity` per-connection at Send time (`handle.rs:392-425`, the T-11-18 forgery-prevention check), can look up whether the SENDING connection was registered with `origin: gateway-relay` and stamp a **broker-internal, non-envelope wrapper** around the stored mailbox record — NOT inside the signed/content-transparent `Value`, but as a sibling field in whatever wraps it for storage (a new small struct, not `WireEnvelope<B>`).
- Every read surface (1-5, 7 above) then surfaces that wrapper's `origin`/`provenance` tag alongside — never instead of — the envelope, so the MCP tool JSON becomes `{"task_id":..., "provenance": "remote-untrusted", "envelope": env}` and the CLI JSONL gains a sibling `provenance` key on the same line. **This is additive to every one of surfaces 1-5 and does not require touching `famp-envelope`, `famp-canonical`, or `famp-crypto`.**
- This still only tags provenance — it does not by itself constitute "quarantine." The actual quarantine (wrapping body content so a harness cannot mistake it for instructions, regardless of which of surfaces 1-5 renders it) is a presentation-layer concern that should be implemented ONCE, as a shared helper function used by all of surfaces 1-5, rather than five separate ad-hoc implementations — e.g. a single `famp::provenance::render_untrusted(value: &Value) -> Value` (or `-> String`) call inserted at each of the five sites, so a missed surface is structurally harder (one helper, five call sites, each individually testable/CI-gated) rather than "five independent judgment calls."

---

## Q6 — Push adapter (SEED-002, `famp watch --notify`)

**Does the broker have a per-identity event stream, or is `await` the only mechanism?**

`await` is the only mechanism today. There is no separate pub/sub event stream — `BusMessage::Await { timeout_ms, task }` (`famp-bus/src/proto.rs:163-168`) parks the connection in the broker's `pending_awaits` table (referenced at `handle.rs:465-477`, `broker/awaiting.rs`) until a matching `Send`/`Join` wakes it via `Out::Reply` + `Out::UnparkAwait` (`handle.rs:469-477`). It is a long-poll, not a stream: one parked connection, one wake, then the connection either re-issues `Await` or disconnects. The Stop-hook shim (`famp-await.sh`) is built entirely on top of this long-poll (`await --as <identity> --timeout 23h`, `famp-await.sh:582-585`).

**Would `watch` be additive, or does it require broker changes?**

**Additive in spirit, but it does require a small, well-scoped `famp-bus` change** — not a violation of the tokio-free gate, since the gate constrains *how* the broker is implemented (no tokio inside `famp-bus`'s pure actor core), not *whether* new message types/behaviors can be added to it. Two viable designs:

1. **Thin wrapper, zero broker change:** `famp watch --notify <cmd>` is purely a CLI-side loop that calls `famp await` repeatedly (exactly what the Stop-hook shell script already does) and, on each wake, execs `<cmd>` instead of (or in addition to) emitting the block-decision JSON. This requires ZERO `famp-bus` change — it is a new CLI subcommand in `crates/famp/src/cli/` that reuses `cli::await_cmd::run_at_structured` (`await_cmd/mod.rs`) in a loop. This is the lowest-risk, fastest-to-ship option and should be the default recommendation.
2. **True broker-side push (if #1 proves insufficient — e.g. needs to survive the CLI process dying and restarting without missing a beat, or wants multiple concurrent watchers per identity):** would need the broker to support a *second* parked-waiter shape that isn't a one-shot long-poll but a registered persistent subscription, re-armed automatically by the broker itself rather than the client re-issuing `Await`. That is new logic in `famp-bus/src/broker/awaiting.rs` and `handle.rs`'s wake path — still tokio-free (the re-arm is pure state-machine bookkeeping, same as today's one-shot wake), so it does not collide with `just check-no-tokio-in-bus`; the tokio side lives in whatever process consumes the subscription (a new CLI daemon loop), exactly as `famp-gateway`'s tokio-based egress task consumes the tokio-free broker's UDS protocol today.

**Recommendation:** ship #1 first (genuinely additive, zero `famp-bus` risk, matches SEED-002's stated goal of "replacing the `famp await` long-poll + `.famp-listen` sentinel + global Stop-hook trick" with a first-class subcommand that does the same long-poll under the hood). Only reach for #2 if usage reveals #1's process-lifetime coupling is a real problem.

---

## Q7 — Signed peer directory

**New crate, not a module in an existing one.** Rationale: a signed directory is conceptually distinct from both `famp-keyring` (local TOFU pin store, no signing, no network fetch) and `famp-gateway` (the relay/verification runtime) — it is closer to a small, standalone "fetch + verify a signed document, then feed the result into the keyring" library, with its own format and its own signature-verification logic over a *directory document* (a different signing context than per-envelope INV-10 signing). Bolting it into `famp-keyring` would pull directory-fetch/HTTP concerns into what is currently a pure, dependency-light, file-I/O-only crate (`famp-keyring`'s only deps today are `famp-core` + `famp-crypto`, per `Cargo.toml`); bolting it into `famp-gateway` would conflate "the runtime that relays live traffic" with "the offline/periodic job that refreshes trust data."

**Proposed name:** `famp-directory` (or `famp-peer-directory`), new Layer-2 crate.

**What it depends on:** `famp-core` (for `Principal`), `famp-crypto` (for `TrustedVerifyingKey`, and to verify the directory document's own signature — the directory itself should be signed by *something*, likely a well-known directory-operator key each participant pins once, same TOFU pattern as peer keys), and `famp-canonical` (to canonicalize the directory document before verifying its signature, exactly as every other signed artifact in this codebase does). It should NOT depend on `famp-gateway` or `famp-bus` — it produces `(Principal, TrustedVerifyingKey)` tuples (or a batch thereof) that get fed into `famp-keyring::Keyring::pin_tofu`/`with_peer`, the same integration point as Q2's bootstrap mechanisms.

**Does it create a dependency cycle with `famp-keyring`?** No, as long as the dependency direction is `famp-directory → famp-keyring` (directory crate calls into the keyring to pin fetched entries) rather than the reverse. `famp-keyring` should stay ignorant of directories entirely — it only ever sees `pin_tofu(principal, key)` calls, regardless of whether they originated from `peer import`, a QR scan, or a directory fetch. This mirrors exactly how `famp-gateway` depends on `famp-keyring` today (`Cargo.toml`'s workspace member list plus `famp-gateway`'s `Keyring::load_from_file` call at `main.rs:374`) without any reverse dependency.

**Consumer wiring:** a new CLI subcommand (`famp peer directory-sync`? — naming TBD) in `crates/famp/src/cli/peer/` that calls `famp-directory`, gets back verified `(Principal, TrustedVerifyingKey)` entries, and pins each via the existing `Keyring::pin_tofu` + `save_to_file` pair (`import.rs:65-83` is the template to follow).

---

## Suggested Build Order

**Hard dependency chain (must land in this relative order):**

```
Phase 13 (spike, zero code)
  Public-reachability model decision (Q1)
        │
        ▼
Phase — Trust bootstrap v2: multi-key keyring + revocation (Q2 + Q4)
  (famp-keyring format extension: Vec<key>/rotation/revoked-tag)
  MUST land before any new bootstrap UX, because export/import/directory
  all write through pin_tofu — changing its shape once, early, avoids a
  second migration later.
        │
        ├──────────────────────────────┐
        ▼                              ▼
  New bootstrap UX (Q2: short code /   Signed peer directory (Q7, new crate)
  QR / PAKE) — additive CLI, depends   — depends on the extended keyring
  only on the extended keyring          shape from the prior phase
        │                              │
        └──────────────┬───────────────┘
                        ▼
        Protocol-grade ingress (Q3: replay cache, freshness
        enforcement, audience binding, DoS ordering)
        — depends on the reachability model (Q1) being real
          (no point hardening ingress before the gateway is
          reachable from the open internet), and benefits from
          revocation (Q4) already existing so a compromised-key
          scenario has a real remediation path once discovered
                        │
                        ▼
        Reachability implementation (Q1: relay/NAT-traversal code,
        per the Phase-13 spike's decision) — the ingress hardening
        above should exist BEFORE this goes live on the open
        internet, not after
                        │
                        ▼
        Inbound-content-is-DATA quarantine (Q5)
        — BLOCKING GATE. Must land before any outside person
          connects, per PROJECT.md. Technically independent of
          Q1-Q4/Q7 (it's a presentation-layer change touching
          famp-bus's Register frame + 5 CLI/MCP read sites), so it
          CAN be built in parallel with everything above — but it
          must be VERIFIED complete (adversarial corpus in CI)
          before the human acceptance gate (early, Phase 2-3 per
          PROJECT.md) lets a second person's traffic reach this host.
                        │
                        ▼
        Human acceptance gate (second person, real network, no
        shared VPN, no hand-copied keys)
```

**Independently verifiable (can be built, tested, and CI-gated on their own, with no cross-phase coupling to prove correctness):**
- Q6 (push adapter / `famp watch --notify`) — genuinely orthogonal to everything else; ship whenever convenient, no ordering constraint. (PROJECT.md already notes this was "orthogonal to the federation transport" when scoping v1.0.)
- Q4 (revocation) mechanism itself, once Q2's multi-key format lands — the revoked-key reject path in `verify_inbound`/`verify_inbound_any` is unit-testable with zero network/relay dependency.
- Q5 (quarantine) — testable via an adversarial corpus fed directly into the 5 rendering call sites, no live gateway/relay/directory needed.
- Q7 (directory crate) — testable as a pure fetch-and-verify library against fixture documents, independent of the live gateway.

**Must land together (cannot be verified independently, only as a pair/group):**
- Q1 (reachability code) + Q3 (protocol-grade ingress) — a reachability path that is live on the open internet WITHOUT the replay/freshness/audience checks is an unacceptable intermediate state; these two should ship in the same phase or with Q3 strictly gating Q1's go-live (feature-flag the public listener until Q3's checks are in place).
- Q2 (keyring extension) + Q4 (revocation data model) — revocation only makes sense once there's a "retired vs. revoked" multi-key shape to hang it on; designing them separately risks a second keyring-format migration.
- The blocking gate (Q5) must be verified complete BEFORE the human acceptance gate, but its own construction is independent of Q1/Q2/Q3/Q4/Q7 — it can and should be built and CI-gated first or in parallel, precisely because PROJECT.md schedules the human gate early (Phase 2-3) and Q5 is a hard precondition for it.

## Sources

All findings sourced directly from the repository at HEAD (2026-07-30), no external documentation lookups were needed — this is a pure code-archaeology research task per the milestone brief. Files read in full: `ARCHITECTURE.md`, `.planning/PROJECT.md`, `Cargo.toml`, `crates/famp-gateway/src/{lib,ingress,egress,verify,registry,principal,error,main}.rs`, `crates/famp-envelope/src/{envelope,peek,timestamp,bus}.rs`, `crates/famp-keyring/src/{lib,file_format,peer_flag}.rs`, `crates/famp/src/cli/peer/{mod,export,import,identity}.rs`, `crates/famp/src/cli/mcp/tools/{inbox,await_,channel_log}.rs`, `crates/famp/src/cli/mcp/server.rs`, `crates/famp/src/cli/inbox/list.rs`, `crates/famp/assets/famp-await.sh`, `crates/famp/src/cli/hook/emit.rs`, `crates/famp-bus/src/{proto,mailbox}.rs`, `crates/famp-bus/src/broker/handle.rs` (partial).

---
*Architecture research for: FAMP v1.1 Open-Internet Federation*
*Researched: 2026-07-30*
