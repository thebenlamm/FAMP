# Feature Research

**Domain:** Federated agent-to-agent messaging — open-internet trust bootstrap, peer directory, protocol-grade ingress, prompt-injection boundary, and push-notification wake-up
**Researched:** 2026-07-30
**Confidence:** HIGH on documented protocol behavior (Signal, Matrix, Nostr, RFC 9421, SigV4, DPoP, OWASP); MEDIUM on which exact scheme FAMP should adopt (judgment call, not settled by research); LOW on anything requiring FAMP-specific benchmarking (no adversarial corpus exists yet)

This file covers only the SIX new v1.1 capability areas from `.planning/PROJECT.md`: cross-person trust bootstrap, signed peer directory, protocol-grade ingress (freshness/replay/audience/DoS/revocation), the inbound-content-is-DATA boundary, and the push-notification harness adapter (SEED-002). Already-shipped v1.0 features (gateway, TOFU `peer export`/`import`, INV-10 signing, task FSM, MCP surface) are treated as fixed dependencies, not re-researched.

---

## 1. Cross-Person Trust Bootstrap

This is the milestone's most load-bearing question: the acceptance criterion is a second person completing setup **unassisted from a doc**, and v1.0's `famp peer export` → paste-over-Signal → `famp peer import` is explicitly expected to fail with a real human (PROJECT.md, v1.1 scope). Below is what each comparable system's human actually does, in concrete step counts, plus the failure mode and recovery path.

### System-by-system findings

**Magic Wormhole (SPAKE2 PAKE, low-entropy code)**
Two humans, no prior shared secret — exactly FAMP's problem. One side runs `wormhole send <file>`, which prints a code of the form `7-guitarist-revenge` (a numeric nameplate + 1-2 wordlist words, ~16 bits of entropy at the default `--code-length=2`). The second human types that code into `wormhole receive` on their own machine. That's it — **2 human actions total** (one types/reads the code, one types it in), no visual comparison step, no fingerprint eyeballing. Under the hood SPAKE2 derives a shared key from the low-entropy code such that an attacker gets exactly **one guess**: entering a wrong code causes the mailbox to be released and the connection dropped on both sides — `WrongPasswordError` — a hard fail, not a silent one, and detectable by both parties simultaneously. Recovery is trivial: re-run `send` to get a fresh code and try again. [Welcome — Magic-Wormhole docs](https://magic-wormhole.readthedocs.io/en/latest/welcome.html) · [Client-to-Client Protocol](https://magic-wormhole.readthedocs.io/en/latest/client-protocol.html) · [16 bits of entropy issue #191](https://github.com/warner/magic-wormhole/issues/191)
- **Why it matters for FAMP:** this is the closest analog to "two humans, no shared secret, need mutual trust in minutes." The low-entropy code works *because* of the one-guess-then-detected property of PAKE — a code that's merely "hard to copy correctly" (like FAMP's current 3-field pubkey blob) has no such protection; a corrupted paste silently imports a wrong key rather than failing loud.

**Signal safety numbers (post-hoc verification, not bootstrap)**
Not actually a bootstrap mechanism — trust is established automatically on first message (TOFU) and safety-number *verification* is an optional, later, out-of-band check. The human flow: open the conversation → tap contact name → "View Safety Number" → see a 60-digit code (12 groups of 5 digits) plus a QR code → scan the other party's QR code in person, or read/compare the 60 digits over a voice call, or share via another channel → tap "Mark as Verified". That's **4-5 steps**, and it happens *after* messages already flowed under an unverified key. If the numbers don't match, Signal gives no explicit "wrong" signal beyond the numbers visibly differing — the human must notice and stop trusting; there's no cryptographic hard-fail like Wormhole's. If a contact's key changes later (new phone, reinstall), Signal shows a "safety number changed" banner and previously-verified status is revoked automatically, requiring re-verification before further "verified" messaging. [How to Verify a Signal Contact's Identity](https://www.howtogeek.com/709733/how-to-verify-a-signal-contacts-identity-using-the-safety-number/) · [Signal Support: safety number changed](https://support.signal.org/hc/en-us/articles/360007060632-What-is-a-safety-number-and-why-do-I-see-that-it-changed) · [Signal blog: safety number updates](https://signal.org/blog/safety-number-updates/)
- **Why it matters:** Signal's model is TOFU-then-optionally-verify, i.e. structurally identical to what v1.0 already does (TOFU pin on `peer import`). It does NOT solve the "verify BEFORE trusting" problem FAMP needs for a stranger — it's an anti-pattern to copy directly, because it optimizes for casual messaging with security as an opt-in upgrade, not for "no shared VPN, no hand-copied keys, first contact must be right."

**Matrix emoji SAS (cross-signing + interactive verification)**
Requires both parties already have some communication channel (a Matrix room) open. Flow: one party initiates a verification request → the other accepts → both clients display the SAME sequence of ~7 emoji (or a numeric fallback) derived from a Diffie-Hellman commit-then-reveal exchange (an actual short-authentication-string / SAS protocol, not just a display of a static fingerprint) → both humans read them aloud or compare visually → both tap "they match" → clients cross-sign each other's keys. **~5-6 human steps** (initiate, accept, compare, confirm x2). If the emoji don't match, either side taps "they don't match," which aborts and the devices are NOT trusted — a hard-refusal UI path, closer to Wormhole's fail-loud model than Signal's. Recovery: retry from scratch. [Matrix.org e2ee cross-signing docs](https://matrix.org/docs/older/e2ee-cross-signing/) · [Cross-Signing — Matrix JS SDK](https://matrix-org-matrix-js-sdk.mintlify.app/guides/cross-signing)
- **Why it matters:** SAS is cryptographically stronger than "eyeball a pasted fingerprint" (it's an interactive commit-reveal, immune to the corrupted-paste failure mode) but needs a live bidirectional session at verification time and a UI affordance for "match / no match" — non-trivial for a CLI-first tool like FAMP unless the two peers are mid-conversation over an existing channel (e.g., Signal chat) while running the ceremony.

**SSH TOFU + `ssh-keygen -lv` randomart**
The honest baseline for "what NOT to expect a stranger to get right." First connection prints a fingerprint (SHA256 hash, or `-v` ASCII-art "randomart" image) and a yes/no prompt; the human is *supposed* to compare it against an out-of-band-obtained authoritative value, but in practice almost nobody does — accepting on faith is the overwhelmingly common real-world behavior, which is exactly the failure mode "the person doesn't verify, just accepts" that a real (non-Ben) user threatens to reproduce with FAMP's current flow. [Trust on first use — Wikipedia](https://en.wikipedia.org/wiki/Trust_on_first_use) · [How to verify SSH host key fingerprints](https://www.simplified.guide/ssh/verify-host-key)
- **Why it matters:** this is the negative case. FAMP's v1.0 mechanism (paste a 3-field blob, eyeball the fingerprint, `import` even on mismatch with only a `warning:`) is architecturally the SSH TOFU pattern, and SSH TOFU's real-world failure mode (nobody actually checks) is precisely what the v1.1 acceptance gate is testing for. Don't design a v1.1 mechanism that's "SSH TOFU with slightly nicer formatting" — it will reproduce the same failure with a real, non-Ben, non-technical-on-purpose user.

**age/rage recipients**
Not a two-party bootstrap protocol at all — it's a file-encryption recipient list (`-r <pubkey>`), with the pubkey obtained by whatever out-of-band means the user likes (paste, file, etc.) and zero built-in verification ceremony. Notably, age has **no revocation mechanism at all** by design — if a key is compromised, the only remedy is generating a new keypair and re-encrypting everything, manually, forever. [age and Authenticated Encryption](https://words.filippo.io/age-authentication/) · [rage — a simple, secure file encryption tool](https://github.com/str4d/rage)
- **Why it matters:** relevant mainly to the **revocation** section below, not bootstrap — it's the "what happens if we do nothing" baseline.

**Syncthing device IDs + Introducer**
Device ID is a long alphanumeric string (or QR code) shown under Settings; adding a remote device means pasting its ID (or scanning its QR in person) into the Add Device dialog. TLS handshake occurs, then the device ID is derived from the cert and checked against what was configured — mismatch drops the connection outright (fail-closed, not warn-and-continue). "Introducer" is a convenience feature: a device flagged as an introducer will automatically vouch other devices it already trusts to a newly-added peer, extending trust transitively without a fresh pairwise ceremony. [Understanding Device IDs — Syncthing docs](https://docs.syncthing.net/dev/device-ids.html)
- **Why it matters:** the QR-code-for-in-person-pairing pattern (also used by Signal, WhatsApp) is a strong option if the two humans in the v1.1 acceptance gate are ever in the same room; the Introducer pattern is a differentiator idea for future multi-peer FAMP federations (not needed for the 2-person v1.1 gate) but worth flagging as a natural v1.2+ extension of the signed peer directory.

**Tailscale auth keys**
Solves a *different* problem (device-to-network join, not stranger-to-stranger trust) via a centrally-issued, admin-generated key (`tskey-...`) pasted into `tailscale up --auth-key=...` — one command, zero verification ceremony, because trust is delegated entirely to the Tailscale control-plane account, which both devices already share. [Auth keys — Tailscale Docs](https://tailscale.com/docs/features/access-control/auth-keys)
- **Why it matters:** this is an anti-pattern for FAMP's stated problem — it requires a shared central authority (an account both people are members of), which directly contradicts "no shared VPN, no hand-copied keys" and the "no central authority" federation model. Cite it as the thing FAMP deliberately is NOT building.

**WireGuard config exchange**
Structurally identical to FAMP's own `peer export`/`import`: each side generates a keypair, the public keys are exchanged over "any out-of-band method... similar to how one might send an SSH public key to a friend," and each side's config file lists the other's pubkey + allowed IPs. No verification ceremony beyond "did you get the right string." [WireGuard Quick Start](https://www.wireguard.com/quickstart/)
- **Why it matters:** WireGuard's own docs describe the pattern FAMP shipped in v1.0 and treat it as sufficient — but WireGuard's target user is a sysadmin, not an unassisted stranger. This is further evidence the v1.0 mechanism is a known-good pattern for a *technical* peer, and the v1.1 gap is specifically about non-technical unassisted usability, not cryptographic soundness.

### Categorization

| Feature | Category | Complexity | Notes / dependency on shipped features |
|---|---|---|---|
| PAKE-backed short-code pairing (Wormhole-style) replacing pasted-pubkey TOFU | **Differentiator** (arguably table stakes given the explicit acceptance gate) | HIGH — needs a new PAKE implementation (SPAKE2 or similar) + a rendezvous/mailbox service for the two sides to find each other, since FAMP has no existing signaling channel between strangers | Depends on: `famp-keyring`/TOFU pinning (v1.0) as the storage backend once the code exchange derives a key; the gateway's HTTPS listener (v1.0) as the transport the derived trust then rides on. Does NOT reuse v1.0's `peer export`/`import` blob format — this is a new command, not a UX polish of the old one. |
| Fail-loud on mismatch (reject import on any fingerprint/derivation mismatch, no "warning: still imports") | **Table stakes** | LOW — v1.0 already computes the fingerprint check; the change is refusing rather than warning | Direct fix to `famp peer import`'s existing warn-but-import behavior (GATEWAY-SETUP.md §3) |
| In-person QR-code pairing option (Syncthing/Signal-style) | **Differentiator**, optional secondary path | LOW-MEDIUM — needs a QR renderer/scanner, only useful if the two people can be co-located even briefly | Independent of the PAKE path; nice-to-have fallback, not required for the acceptance gate (the gate explicitly requires "no shared VPN... doc unassisted," implying possibly-remote, so QR-in-person can't be the *only* path) |
| SAS-style interactive emoji/number comparison (Matrix-style) | **Anti-feature for v1.1** (revisit for v1.2+) | HIGH — needs a live bidirectional session at verification time, awkward for a CLI-only tool with no shared always-on channel between strangers yet | Requires the peer directory / rendezvous infra to exist first — chicken-and-egg with the very problem it's meant to solve |
| Central-authority auth keys (Tailscale-style) | **Anti-feature** | N/A — deliberately rejected | Contradicts "no shared VPN," "no central authority" constraints in PROJECT.md |
| Silent TOFU-then-optional-verify (Signal/SSH-style, i.e. ship as-is) | **Anti-feature** | N/A — this is what v1.0 already does and is flagged as expected to fail | Already shipped; the milestone exists specifically to replace this |

**Concrete step-count comparison (fewer human steps = lower failure surface):**

| Scheme | Human steps | Fail-loud on wrong input? | Recovery path |
|---|---|---|---|
| Magic Wormhole PAKE | 2 (read code, type code) | YES — hard abort, both sides notified | Re-run, get new code |
| Matrix emoji SAS | 5-6 (initiate, accept, compare x2, confirm x2) | YES — explicit "no match" button | Retry from scratch |
| Signal safety number | 4-5, but AFTER trust already granted | NO — silent visual mismatch only | Re-verify after banner |
| SSH TOFU | 1 (accept prompt), verification optional and rarely done | NO — accept-by-default | None; damage is done at accept time |
| FAMP v1.0 (`peer export`/`import`) | ~4 (export, copy, paste, import) per direction, x2 directions = ~8 total | NO — "warning:" only, still imports | Manual re-pin, and only if the human happens to notice the warning |

---

## 2. Signed Peer Directory

**Matrix federation:** discovery via `/.well-known/matrix/server` (a JSON file naming the actual homeserver address behind a domain, similar in spirit to DNS SRV) plus a signing-keys endpoint `/_matrix/key/v2/server` that publishes the server's current and recently-retired (`old_verify_keys`) Ed25519 keys. Every federation request is authenticated by a signature in the HTTP `Authorization` header AND at the TLS layer. Staleness is handled by an explicit cache-lifetime contract: intermediate "notary" servers cache a key response for half its stated lifetime, and origins are told not to advertise expiries under an hour, to bound how stale a cached key can be. [Server-Server API — Matrix spec](https://spec.matrix.org/v1.11/server-server-api/)
- **What "signed directory" means concretely here:** a per-domain, self-published, versioned key list with an explicit TTL — not a third-party-curated registry. Any relying party fetches it live (or via a caching notary) rather than trusting a bundled snapshot.

**ActivityPub/Mastodon:** discovery is two-hop — WebFinger (`GET /.well-known/webfinger?resource=acct:user@domain`) maps a human-readable handle to the actor's canonical URL, then a `GET` on that actor URL (content-negotiated `application/activity+json`) returns the actor object, which embeds a `publicKey` field. Every POST to an inbox must carry an HTTP Signature keyed to that publicKey; GETs may optionally require one too. [WebFinger — Mastodon docs](https://docs.joinmastodon.org/spec/webfinger/) · [ActivityPub — Mastodon docs](https://docs.joinmastodon.org/spec/activitypub/)
- **What "signed directory" means here:** no central directory at all — each domain is its own authority, key distribution piggybacks on the same actor document used for profile lookup, and HTTP Signatures (a precursor to RFC 9421) bind every write to a specific actor key.

**Nostr NIP-05 + relays:** the closest philosophical match to FAMP (no server, no domain-owned identity beyond a courtesy DNS mapping). A client resolves `user@domain` by fetching `https://domain/.well-known/nostr.json?name=user`, which returns the user's real identity — the Ed25519-family public key — plus (optionally) a `relays` map naming which relay servers to query for that pubkey's events. The actual trust anchor is the pubkey itself (self-sovereign, not domain-issued); NIP-05 is a convenience label, not an authority — losing the domain doesn't invalidate the key. [NIP-05 — nostr-protocol/nips](https://github.com/nostr-protocol/nips/blob/master/05.md) · [Trust in Nostr NIP-05 Identifiers](https://engineering.block.xyz/blog/trust-in-nostr-nip-05-identifiers)
- **What "signed directory" means here:** it's explicitly NOT signed in the cryptographic sense — it's a DNS-hosted courtesy pointer, and the actual signed artifact is every individual event (Schnorr-signed, content-addressed by SHA-256 id), not a directory entry.

**Email (MX/DKIM/DANE):** MX records point at mail servers (unsigned, DNS-cache-TTL-bounded); DKIM signs individual messages with a domain-published public key in a DNS TXT record (works without DNSSEC — the DNS lookup itself is the weak link, not the signature); DANE (TLSA records) additionally pins the expected TLS certificate for a domain's mail server but is only meaningful when DNSSEC is enabled end-to-end — without DNSSEC, TLSA records themselves are spoofable and add zero security. [How SMTP DANE works — Microsoft Learn](https://learn.microsoft.com/en-us/exchange/security-and-compliance/how-dane-secures-email) · [DANE explained — SEAL](https://frameworks.securityalliance.org/infrastructure/domain-and-dns-security/dnssec-and-email/)
- **What "signed directory" means here:** three separate layers, each solving one narrow problem (routing, message integrity, transport-cert pinning), stacked and NONE of them sufficient alone — a cautionary tale against a single "signed directory" doing everything.

**Certificate Transparency:** not a directory at all — an append-only, publicly auditable log that every CA-issued cert must be submitted to before browsers trust it. Critically, CT is **detective, not preventive**: it doesn't stop a malicious/compromised CA from issuing a bad cert, it just guarantees that issuance becomes publicly visible so a domain owner (or an automated monitor watching on their behalf) can find out. [How CT Works](https://certificate.transparency.dev/howctworks/) · CAA vs CT distinction, [ivision research blog](https://research.ivision.com/how-does-certificate-transparency-work.html)
- **What "signed directory" means here:** immutable append-only audit log + Merkle-tree tamper-evidence, decoupled entirely from trust decisions (CT logs don't decide who to trust — they let you notice if someone else made a decision on your behalf).

### Categorization

| Feature | Category | Complexity | Dependency |
|---|---|---|---|
| Self-published, domain-scoped signed key list with explicit TTL (Matrix-style `/_matrix/key/v2/server` analog) | **Table stakes** for v1.1's "signed peer directory" requirement | MEDIUM — needs an endpoint format + staleness/TTL semantics, but FAMP already has `key_id` and a keyring on disk (v1.0) | Builds directly on `famp-keyring` + the gateway's existing `--peer <domain>=<url>` map; extend, don't replace |
| Self-sovereign pubkey-as-identity, domain label as courtesy pointer only (Nostr NIP-05 model) | **Differentiator** — philosophically the better fit for a no-central-authority federation | LOW — FAMP's `agent:<domain>/<name>` principal scheme already treats the domain as a label, and Layer 0 primitives never assumed a CA | Aligns with the "Layer 0 primitives stay untouched" constraint — the directory is additive metadata, not a new trust root |
| Third-party curated central registry (a hypothetical "FAMP directory service") | **Anti-feature** | N/A | Directly contradicts "no central authority," "bilateral only" (PROJECT.md Out-of-Scope: cross-federation delegation, multi-party commitment) |
| Public append-only audit log of directory changes (CT-style) | **Differentiator, defer to v1.2+** | HIGH — a whole separate service; valuable at ecosystem scale (Gate B / multi-implementer), not for a 2-person v1.1 gate | Only matters once Gate B (second implementer) fires — explicitly out of scope per PROJECT.md |
| Stacking routing + message-integrity + transport-pin into ONE artifact (email's 3-layer mistake, inverted) | **Anti-feature warning** — don't conflate "where do I send this" with "is this signature valid" | N/A | FAMP's existing `--peer <domain>=<url>` (routing) and `peers.keyring` (trust) are already correctly separated — keep them separated in the v1.1 directory design too |

---

## 3. Freshness / Replay Defense

FAMP's envelope already reserves `nonce` and `expiry` fields (v1.0, unused pending v1.1 — PROJECT.md). The question is what window and cache-bound to use.

| System | Mechanism | Window / bound | Source |
|---|---|---|---|
| AWS SigV4 | `X-Amz-Date` timestamp in the signed scope | **±15 minutes**, hard-rejected outside with `RequestTimeTooSkewed` | [Troubleshoot SigV4 — AWS docs](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv-troubleshooting.html) |
| OAuth DPoP (RFC 9449) | `iat` timestamp + optional server-issued nonce | Implementation-defined, "on the order of seconds to minutes" (FusionAuth's own implementation uses 10s lifetime ± 15s skew); server-nonce mode exists specifically because client-clock skew can be large and unreliable | [RFC 9449](https://datatracker.ietf.org/doc/html/rfc9449) · [DPoP — FusionAuth docs](https://fusionauth.io/docs/lifecycle/authenticate-users/oauth/dpop) |
| HTTP Message Signatures (RFC 9421) | `created` + optional `expires` params, plus an app-supplied `nonce` with a verifier-side dedup cache | Convention: `created` within **±5 minutes**; replay-cache window should track real HTTP timeout norms (30-60s) with a safety margin (community guidance cites ~8x, i.e. bound the cache to a few minutes, not hours, to avoid unbounded memory growth) | [RFC 9421](https://datatracker.ietf.org/doc/rfc9421/) · [Understanding HTTP message signatures](https://victoronsoftware.com/posts/http-message-signatures/) |
| Nostr | `created_at` on every signed event; no protocol-mandated replay cache — relays are free to accept/reject/dedupe by event id as they see fit; deletion (NIP-09) is advisory-only, "relays SHOULD... but there is no requirement" | No fixed window — deliberately loose, reflecting Nostr's relay-trust-optional design | [NIP-09](https://github.com/nostr-protocol/nips/blob/master/09.md) |
| Matrix | Federation requests are signed per-request (Authorization header, not a standing session); staleness bounded via the key-TTL/notary-cache-half-life rule above, not a nonce cache per se | Key TTL ≥1 hour by convention; per-request freshness relies on TLS + signature over the exact request | [Server-Server API](https://spec.matrix.org/v1.11/server-server-api/) |

**Synthesis for FAMP:** the converging convention across SigV4/DPoP/RFC 9421 is a **single-digit-minutes clock-skew window** (5 min is the most commonly cited figure, 15 min is the outer/conservative bound used by AWS at massive scale) paired with a **bounded, short-lived nonce/jti cache** (minutes, not hours) to avoid unbounded memory growth — exactly the shape PROJECT.md already gestures at ("freshness / bounded replay cache"). Nostr's laissez-faire model is the cautionary counter-example: without a mandatory window, replay defense becomes "whatever the relay feels like," which is unacceptable for a protocol whose core value proposition is byte-exact, signature-verifiable behavior.

### Categorization

| Feature | Category | Complexity | Dependency |
|---|---|---|---|
| `expiry` field enforcement with a minutes-scale window (5-15 min) | **Table stakes** | LOW-MEDIUM — field already reserved in the envelope (v1.0); needs validation logic at gateway ingress | Depends on the `nonce`/`expiry` fields already reserved on `famp-envelope` (v1.0) — "use them, don't add a second signature" (PROJECT.md constraint) |
| Bounded nonce-dedup cache (sized to the expiry window, not unbounded) | **Table stakes** | MEDIUM — needs an in-memory (or lightly persisted) cache with eviction tied to `expiry`, scoped per sender or per (sender, serial) pair | Lives at gateway ingress, alongside the existing `verify_inbound_any` trust check (v1.0) |
| Server-issued nonce mode (DPoP-style, defends against large/adversarial client clock skew) | **Differentiator**, defer unless clock skew proves to be a real problem | MEDIUM-HIGH — requires a round-trip (server hands out a nonce before the real request), which changes the protocol shape | Not required for a bilateral 2-machine gate; revisit if fielded skew turns out large |
| Unbounded/no replay window (Nostr laissez-faire model) | **Anti-feature** | N/A | Contradicts FAMP's byte-exact, signature-verifiable core value; a protocol that can't say precisely what "fresh" means undermines conformance |

---

## 4. Key Revocation (No Central Authority)

| System | Mechanism | What actually works without a CA |
|---|---|---|
| Signal | No formal revocation primitive — a device's identity key simply changes (new phone/reinstall) and Signal auto-invalidates prior "verified" status, forcing re-verification; there is no way to declare "this old key is now untrusted" independent of a key *change* actually happening | Implicit revocation via mandatory re-verification on key-change events, not an explicit revoke message [Signal Support](https://support.signal.org/hc/en-us/articles/360007060632-What-is-a-safety-number-and-why-do-I-see-that-it-changed) |
| age/rage | **None whatsoever, by explicit design decision.** Compromise recovery = generate a new keypair and manually re-encrypt everything, forever, with no way to signal "the old key is dead" to anyone | [age and Authenticated Encryption](https://words.filippo.io/age-authentication/) |
| SSH CA + KRL | A binary Key Revocation List (`ssh-keygen -k`, referenced via `RevokedKeys` in `sshd_config`) can revoke either full CA-issued certs (by serial number — 1 bit each, extremely compact) or bare TOFU-pinned plain keys (by key hash — less space-efficient but works without a CA at all) | The KRL format's bare-key-hash-revocation mode is the piece that works with NO central CA — a party can distribute a "these specific key hashes are dead" list out-of-band, same channel as the original pin [PROTOCOL.krl](https://raw.githubusercontent.com/openssh/openssh-portable/master/PROTOCOL.krl) |
| PGP/GPG | A pre-generated, signed "revocation certificate" is created and (ideally) stored safely BEFORE it's ever needed, then published to keyservers when the key is compromised. Explicitly documented as PGP's weakest link: no guarantee revocation info actually reaches everyone, keyserver propagation is slow, and a leaked revocation cert lets a third party invalidate someone's key | [Revocation in OpenPGP](https://dkg.gitlab.io/openpgp-revocation/) — describes this as "the weakest link of OpenPGP PKI" |

**Synthesis for FAMP:** the two-party, no-CA case has exactly one pattern that generalizes without inventing new infrastructure: **an explicitly-signed revocation statement, generated proactively (or on demand), distributed over the same out-of-band channel used for the original trust pin, and checked by the peer's keyring at import/verify time** — this is the SSH-KRL-bare-key-hash idea and the PGP-revocation-cert idea converged. What does NOT work without a CA: anything that assumes a shared registry to check against (that's the FAMP anti-pattern — see peer-directory anti-features above) and anything that relies purely on "the key just silently changed" (Signal's implicit model), because in a bilateral federation there's no guarantee the peer will ever attempt a new connection to trigger that check.

### Categorization

| Feature | Category | Complexity | Dependency |
|---|---|---|---|
| Signed revocation statement, distributed via the same out-of-band channel as the original pin, checked at keyring load/import | **Table stakes** | MEDIUM — new message type + keyring format change (a "revoked" marker, not just delete-on-sight, since the peer must be able to explain WHY a previously-trusted key is now rejected) | Builds on `famp-keyring`'s existing "importing a different key for an already-pinned principal fails closed" behavior (v1.0) — extend that same fail-closed posture to revoked entries |
| Reject silently on missing revocation info (age's "just re-encrypt and hope" model) | **Anti-feature** | N/A | Leaves the peer with no way to learn a key is dead if the out-of-band channel isn't re-used |
| Centralized revocation registry / keyserver | **Anti-feature** | N/A | Contradicts no-central-authority constraint; also reproduces PGP keyserver propagation-lag and poisoning risks |
| Full CA-issued cert + serial-based instant revocation (SSH-CA style) | **Anti-feature for v1.1**, differentiator if Gate B ever needs ecosystem-scale trust | HIGH — requires standing up an actual CA, which no one in a 2-party bilateral federation wants to run or trust | Explicitly the wrong shape for "two independent parties, no central authority" |

---

## 5. Inbound-Content-Is-DATA Boundary (Prompt-Injection)

This is flagged in PROJECT.md as a **BLOCKING SECURITY GATE**, and the 2025-2026 research consensus is unambiguous on the core architectural fact and on what does/doesn't work.

**The core architectural fact (why this can't be solved "in the prompt"):** current LLMs have no structural way to distinguish trusted instructions from untrusted data — both arrive as the same token stream, so **control/data separation cannot be enforced inside the model** and must be enforced by code outside it. [genai.owasp.org LLM01:2025](https://genai.owasp.org/llmrisk/llm01-prompt-injection/) — this directly validates PROJECT.md's own framing ("harness-agnostic... a harness-layer boundary is untestable in FAMP's CI and silently fails to protect Codex/Grok/other clients").

**Simon Willison's "lethal trifecta"** (June 2025): an agent is exploitable for data exfiltration the moment it simultaneously has (1) access to private data, (2) exposure to untrusted content, and (3) a way to communicate externally. [The lethal trifecta for AI agents](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/) — a FAMP agent that reads inbound messages (untrusted content, #2), has local tool access (#1, private data/capability), and can itself send FAMP messages (#3, external communication) is a textbook trifecta instance. This is precisely why PROJECT.md scopes v1.1 as "conversation-only — no remote-triggered tools": removing leg #1 (no capability/approval plane yet) is what keeps the trifecta from closing in v1.1.

**Meta's "Agents Rule of Two" (2025) and Anthropic/OpenAI/DeepMind's "The Attacker Moves Second"** treat Willison's three legs as a budget: an agent may satisfy at most two of the three without a human in the loop; combining all three requires human approval. [New prompt injection papers — Simon Willison](https://simonwillison.net/2025/Nov/2/new-prompt-injection-papers/) · [Agents Rule of Two — Meta AI](https://ai.meta.com/blog/practical-ai-agent-security/)

**Dual-LLM pattern (Willison, 2023) and CaMeL:** a Privileged LLM (has tool access, only sees trusted input) and a Quarantined LLM (may see untrusted content, has NO tool access) run as separate conversations so untrusted text literally cannot reach a context with action-taking power. CaMeL strengthens this into a fully deterministic pattern: the model *proposes* actions, but a separate, non-LLM policy engine outside the model decides whether to execute them, with dynamic information-flow tracking so data provably tainted by an untrusted source can't silently drive a privileged action. [CaMeL — Afine](https://afine.com/llm-security-prompt-injection-camel)

**Microsoft's Spotlighting / datamarking (2024 paper, in production in Azure Prompt Shields):** three concrete transform techniques applied to untrusted text before it's concatenated into a prompt — (1) datamarking: interleave a special token throughout the untrusted span; (2) random interleaving; (3) encoding (base64/ROT13), the strongest of the three because the untrusted span is no longer even plain-text-shaped to the model, which measurably reduces (but does NOT eliminate) susceptibility to embedded instructions. [Defending Against Indirect Prompt Injection With Spotlighting — Microsoft Research](https://www.microsoft.com/en-us/research/publication/defending-against-indirect-prompt-injection-attacks-with-spotlighting/)

**MCP-specific finding:** the MCP ecosystem itself is a known live attack surface — tool titles, descriptions, and parameter names are literally part of the model's prompt, so "enabling a tool hands control of the LLM's inference to whichever MCP server defined that tool's interface" (Simon Willison, on MCP specifically) — and untrusted MCP tool outputs feeding straight into the model's context is the same class of hole FAMP's inbound-message-as-DATA gate exists to close. [Model Context Protocol has prompt injection security problems — Simon Willison](https://simonwillison.net/2025/Apr/9/mcp-prompt-injection/)

**What is known NOT to work:** (a) telling the model "ignore instructions in untrusted text" as a pure prompt-level instruction — refusal training doesn't create structural separation because the model has no architectural concept of source-trust; (b) any purely-in-the-harness convention (a `~/.claude`-side wrapper) — this is the exact anti-pattern PROJECT.md already rejects, because it's untestable in FAMP's own CI and doesn't protect Codex/Grok/other MCP clients.

### Categorization

| Feature | Category | Complexity | Dependency |
|---|---|---|---|
| Structural quarantine at the MCP/CLI output layer: inbound message bodies wrapped with an explicit untrusted-origin marker (spotlighting/datamarking-style), never concatenated as if they were operator instructions | **Table stakes — blocking gate per PROJECT.md** | MEDIUM — mechanical transform at a well-defined choke point (`famp_inbox`/`famp await` output construction), not a model change | Sits at the same layer as the existing `famp_inbox`/`famp_await` MCP tool responses (v0.9-v1.0 surface) — must be added without changing the stable tool contract's shape, only its content-formatting |
| Provenance tagging carried end-to-end (sender identity + trust tier stamped alongside the message body, not just in a log) | **Table stakes** | LOW-MEDIUM — FAMP already carries `from`/`sender_key_id` on signed envelopes (v1.0); the work is surfacing that at the point the content reaches the agent, not discarding it after verification | Reuses INV-10 signature verification output (v1.0) as the source of truth for "who actually sent this" |
| Adversarial corpus in CI (a battery of known injection payloads run against the quarantine boundary every build) | **Table stakes per PROJECT.md** | MEDIUM — needs a maintained payload set + a pass/fail harness; conceptually mirrors FAMP's existing adversarial-conformance-matrix pattern (v0.7/v0.8) | New test surface, but follows the same "adversarial matrix" precedent already used for envelope/transport testing |
| No remote-triggered tool execution (keep the capability/approval plane entirely out of v1.1) | **Table stakes — already decided** | N/A (it's a non-build) | Explicit PROJECT.md scope boundary; removes leg #1/#3 of the lethal trifecta, which is what makes v1.1 safe to ship without a capability plane |
| Dual-LLM / CaMeL-style split runtime (separate privileged vs. quarantined model contexts) | **Differentiator, defer** — real architectural weight, belongs to whichever harness (Claude Code, Codex) hosts the agent, not to FAMP itself | HIGH | FAMP-side, the achievable equivalent is provenance-tagged, clearly-quarantined DATA at the tool-output boundary — the harness-side dual-context split is out of FAMP's control and explicitly named as the wrong place to put the enforcement (PROJECT.md: "not... in `~/.claude` wiring") |
| Prompt-level "please ignore instructions in the following" convention only | **Anti-feature — known NOT to work** | N/A | Directly contradicted by OWASP/genai.owasp.org 2025 guidance: refusal training ≠ structural separation |
| Harness-side-only enforcement (e.g., a Claude-Code-specific hook) | **Anti-feature** | N/A | Explicitly rejected in PROJECT.md: untestable in FAMP CI, doesn't protect Codex/Grok/other clients |

---

## 6. Push-Notification Wake-Up (SEED-002, `famp watch --notify`)

| Mechanism | How it wakes an idle consumer | Fit for replacing `famp await` + Stop-hook |
|---|---|---|
| Long-poll (what FAMP does today) | Client holds an open request; server responds when data arrives or a timeout elapses; client immediately re-issues. Low latency, feels like ordinary HTTP/RPC. | This IS the current mechanism (`famp await` blocking on a parked UDS waiter). Its brittleness in FAMP isn't long-polling per se — it's that the **Stop hook** is what turns the long-poll into a blocking convention the harness has to cooperate with, which is fragile onboarding surface for a new user/harness. |
| Server-Sent Events (SSE) | Single long-lived HTTP connection, server pushes a stream of events over it; simplest to reason about for continuous feeds/dashboards. | Adds a persistent-connection requirement the CLI-first, session-per-window FAMP model doesn't currently need elsewhere; better fit for a future dashboard/inspector UI than for waking an interactive coding-agent session. |
| Webhooks | Source system POSTs to a pre-registered URL the moment an event occurs — no persistent connection, delivery is at-least-once not exactly-once, and the receiver must already be running an HTTP listener to receive it. | This is the natural harness-adapter shape: `famp watch --notify` could be, concretely, "run a small process/handler that the harness's own notification/wake mechanism (not a browser) invokes" — but it inverts the current model (FAMP-broker-initiated push into the harness) rather than the harness blocking on FAMP. |
| MCP server-initiated notifications | MCP's spec explicitly supports bidirectional channels — servers can push tool-initiated events/notifications to the client, not just respond to client-initiated calls. | This is the most idiomatic fit for `famp watch --notify`, since FAMP already ships an MCP server (`famp mcp`) as its primary harness integration surface — a server-initiated notification is a smaller conceptual leap than adding a webhook listener or SSE client to Claude Code / Codex, and doesn't require the harness to expose a new inbound port. |

[Polling vs Long Polling vs SSE vs Webhooks — AlgoMaster](https://blog.algomaster.io/p/polling-vs-long-polling-vs-sse-vs-websockets-webhooks) · [When to use Webhooks vs WebSocket vs Pub/Sub vs Polling — Hookdeck](https://hookdeck.com/webhooks/guides/when-to-use-webhooks) · [Model Context Protocol has prompt injection security problems (context on MCP bidirectionality) — Simon Willison](https://simonwillison.net/2025/Apr/9/mcp-prompt-injection/)

### Categorization

| Feature | Category | Complexity | Dependency |
|---|---|---|---|
| MCP server-initiated push notification replacing the blocking `famp await` + Stop-hook + `.famp-listen` sentinel convention | **Table stakes per PROJECT.md** ("a stranger's agent waking reliably... is part of the unassisted-follower experience") | MEDIUM — the broker already has a waiter/notification mechanism (v0.9's parked-`Await` waiter table); the work is exposing that as an MCP-native push rather than a held connection the harness must specifically know to block on | Directly replaces the existing `famp_await` + `.famp-listen` + global Stop-hook convention (README/CLAUDE.md-documented today); must keep the underlying broker wake-signal plumbing (v0.9) intact — this is a delivery-mechanism change, not a new broker feature |
| Retain long-poll `famp await` as a fallback / for harnesses without notification support | **Differentiator** (graceful degradation) | LOW — it already exists; just don't delete it | Zero new dependency — literally "don't remove the old path" |
| Full SSE/webhook listener added to FAMP itself | **Anti-feature for v1.1** | HIGH — adds a persistent-connection or inbound-port requirement to a CLI tool whose whole value proposition is zero-daemon-babysitting simplicity (v0.11's daemon work was hard-won for exactly the opposite reason: keeping presence simple) | Would duplicate/compete with the daemon's existing UDS-based wake mechanism for no clear benefit at the 2-person v1.1 scale |

---

## Feature Dependencies

```
Signed revocation statement
    └──requires──> famp-keyring fail-closed pin behavior (v1.0, SHIPPED)
                       └──requires──> Cross-person trust bootstrap (new pairing mechanism)
                                          └──requires──> Public reachability decision (Phase 13 spike, separate track)

Signed peer directory (Matrix-key-list-style)
    └──requires──> famp-keyring + gateway --peer map (v1.0, SHIPPED)
    └──enhances──> Cross-person trust bootstrap (directory can host post-pairing key updates)

Protocol-grade ingress (freshness/replay/audience/revocation)
    └──requires──> nonce + expiry fields already reserved on famp-envelope (v1.0, SHIPPED)
    └──requires──> Signed peer directory OR at minimum the existing keyring (for audience binding / revocation lookups)

Inbound-content-is-DATA boundary
    └──requires──> INV-10 signature verification + sender/from binding (v1.0, SHIPPED — SEC-01..04)
    └──BLOCKS──> allowing an outside (non-Ben) person to connect at all (explicit PROJECT.md gate ordering)

SEED-002 push-notification adapter
    └──requires──> broker waiter/notification mechanism (v0.9, SHIPPED)
    └──enhances──> Cross-person trust bootstrap acceptance UAT (a stranger's agent must wake reliably to complete the human gate)

Cross-person trust bootstrap ──conflicts──> Central-authority schemes (Tailscale auth-key model, PGP keyserver model)
```

### Dependency Notes

- **Everything in this milestone rides the existing signature substrate, never a new one.** Every table-stakes recommendation above (revocation statements, directory entries, freshness fields, provenance tags) is designed to extend the ONE existing INV-10 signature and the reserved `nonce`/`expiry` fields — matching PROJECT.md's hard constraint ("do not add a second signature or a parallel envelope type").
- **The trust-bootstrap mechanism gates almost everything else.** A peer directory, revocation, and audience binding all need *some* notion of "which peer is this" established first — bootstrap is correctly sequenced first among the six requirement areas in PROJECT.md.
- **The inbound-content boundary has no upstream dependency on the bootstrap work** — it can and should ship independent of, and before, opening the connection to an outside person, exactly as PROJECT.md states ("Blocking before any outside person connects").
- **SEED-002 (push notification) is technically independent of trust bootstrap**, but PROJECT.md correctly frames it as *part of* the unassisted-follower UX — a stranger who has to also intuit the Stop-hook/sentinel convention adds failure surface on top of the pairing ceremony itself.

---

## MVP Definition

### Launch With (v1.1)

- [ ] **PAKE-backed or equivalent fail-loud pairing** replacing the pasted-blob/warn-and-import TOFU flow — the acceptance gate is literally "unassisted human," and every fail-loud mechanism studied (Wormhole, Matrix SAS) beats every warn-only one (Signal, SSH, v1.0's current behavior) on that exact criterion.
- [ ] **Signed, TTL-bounded peer directory** extending the existing keyring/`--peer` map (Matrix-style key-list pattern, not a central registry).
- [ ] **`expiry`-bounded freshness check + bounded nonce-dedup cache** at gateway ingress, using the fields already reserved in the envelope.
- [ ] **Signed revocation statement**, checked at keyring load, fail-closed like the existing duplicate-pubkey-rejection behavior.
- [ ] **Structural inbound-DATA quarantine + provenance tagging** at the MCP/CLI output layer, plus an adversarial payload corpus wired into CI — blocking, must land before any outside person connects.
- [ ] **MCP server-initiated push notification (`famp watch --notify`)**, with the existing `famp await` long-poll kept as fallback.

### Add After Validation (v1.1.x / v1.2)

- [ ] QR-code-based in-person pairing fallback — only if the two humans in a future federation are ever co-located.
- [ ] Server-issued nonce mode (DPoP-style) — only if fielded clock skew between real peers proves to be a measurable problem.
- [ ] Introducer-style transitive trust (Syncthing-style) — only relevant once FAMP has more than 2 federated parties.

### Future Consideration (v2+)

- [ ] Public append-only audit log of directory/key changes (CT-style) — matters at ecosystem scale, gated on Gate B (second implementer), not this milestone.
- [ ] Full SAS-style live interactive verification ceremony — needs a standing bidirectional channel between strangers that doesn't exist yet; revisit once the peer directory + push notification are mature enough to host it.
- [ ] Any capability/approval/tool-admission plane (FAMP-Sec) — explicitly v2.0+, demand-gated, out of scope here.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---|---|---|---|
| Fail-loud pairing (PAKE or equivalent) | HIGH | HIGH | P1 |
| Signed peer directory (Matrix-key-list pattern) | HIGH | MEDIUM | P1 |
| Freshness window + bounded nonce cache | HIGH | MEDIUM | P1 |
| Signed revocation statement | HIGH | MEDIUM | P1 |
| Inbound-DATA quarantine + provenance tagging + adversarial CI corpus | HIGH (blocking gate) | MEDIUM | P1 |
| MCP push-notification adapter (SEED-002) | HIGH (UX gate) | MEDIUM | P1 |
| QR in-person pairing fallback | LOW | LOW | P3 |
| Server-issued nonce (DPoP-style) | LOW (unless skew proves real) | MEDIUM | P3 |
| Introducer / transitive trust | LOW (2-party scope) | MEDIUM | P3 |
| CT-style public audit log | MEDIUM (ecosystem-scale only) | HIGH | P3, gated on Gate B |
| Dual-LLM / CaMeL harness split | MEDIUM | HIGH, out of FAMP's control | P3 / not-FAMP's-to-build |

## Competitor / Analogous-System Feature Analysis

| Concern | Signal (messaging) | Matrix (federated chat) | Nostr (relay-based) | FAMP's v1.1 approach |
|---|---|---|---|---|
| Trust bootstrap | TOFU + optional post-hoc safety-number check | Live SAS ceremony (emoji/number), needs existing room | None — pubkey IS the identity, no ceremony | PAKE-style fail-loud short exchange (Wormhole-inspired), stronger than Signal/SSH's silent-accept, lighter than Matrix's live-session requirement |
| Directory | N/A (centralized account system) | Self-published signed key list per domain, TTL-bounded | DNS courtesy pointer (NIP-05), pubkey is the real anchor | Matrix-style signed, TTL-bounded list layered on the existing keyring — no central registry |
| Freshness | N/A (Signal Protocol ratchets per-message) | Per-request signature + key-TTL bound | `created_at` only, no mandated cache | Minutes-scale `expiry` window + bounded nonce cache (RFC 9421/SigV4/DPoP convergence) |
| Revocation | Implicit, on key-change only | N/A documented here (out of scope for this research pass) | N/A (no revocation concept) | Explicit signed revocation statement over the same out-of-band channel, fail-closed at keyring load |

## Sources

- [Magic-Wormhole documentation](https://magic-wormhole.readthedocs.io/en/latest/welcome.html) · [Client-to-Client Protocol](https://magic-wormhole.readthedocs.io/en/latest/client-protocol.html) · [16-bit entropy issue](https://github.com/warner/magic-wormhole/issues/191)
- [Signal: verify a contact's identity](https://www.howtogeek.com/709733/how-to-verify-a-signal-contacts-identity-using-the-safety-number/) · [Signal Support: safety number changed](https://support.signal.org/hc/en-us/articles/360007060632-What-is-a-safety-number-and-why-do-I-see-that-it-changed) · [Signal blog: safety number updates](https://signal.org/blog/safety-number-updates/)
- [Matrix e2ee cross-signing](https://matrix.org/docs/older/e2ee-cross-signing/) · [Matrix JS SDK cross-signing guide](https://matrix-org-matrix-js-sdk.mintlify.app/guides/cross-signing) · [Matrix Server-Server API spec](https://spec.matrix.org/v1.11/server-server-api/)
- [SSH Trust on first use — Wikipedia](https://en.wikipedia.org/wiki/Trust_on_first_use) · [Verify SSH host key fingerprints](https://www.simplified.guide/ssh/verify-host-key) · [OpenSSH PROTOCOL.krl](https://raw.githubusercontent.com/openssh/openssh-portable/master/PROTOCOL.krl)
- [age and Authenticated Encryption — Filippo Valsorda](https://words.filippo.io/age-authentication/) · [rage (age impl)](https://github.com/str4d/rage)
- [Syncthing Device IDs](https://docs.syncthing.net/dev/device-ids.html)
- [Tailscale Auth Keys](https://tailscale.com/docs/features/access-control/auth-keys)
- [WireGuard Quick Start](https://www.wireguard.com/quickstart/)
- [Keybase proofs / sigchain](https://keybase.io/blog/keybase-proofs-for-mastodon-and-everyone) · [Keybase Book: sigchain](https://book.keybase.io/docs/server#meet-your-sigchain-and-everyone-elses)
- [Mastodon WebFinger spec](https://docs.joinmastodon.org/spec/webfinger/) · [Mastodon ActivityPub spec](https://docs.joinmastodon.org/spec/activitypub/)
- [NIP-05 — Nostr Implementation Possibilities](https://github.com/nostr-protocol/nips/blob/master/05.md) · [Trust in Nostr NIP-05 Identifiers](https://engineering.block.xyz/blog/trust-in-nostr-nip-05-identifiers) · [NIP-09 deletion](https://github.com/nostr-protocol/nips/blob/master/09.md)
- [DANE and email security — SEAL](https://frameworks.securityalliance.org/infrastructure/domain-and-dns-security/dnssec-and-email/) · [How SMTP DANE works — Microsoft Learn](https://learn.microsoft.com/en-us/exchange/security-and-compliance/how-dane-secures-email)
- [How Certificate Transparency works](https://certificate.transparency.dev/howctworks/) · [CT practical guide](https://research.ivision.com/how-does-certificate-transparency-work.html)
- [RFC 9421 — HTTP Message Signatures](https://datatracker.ietf.org/doc/rfc9421/) · [Understanding HTTP message signatures](https://victoronsoftware.com/posts/http-message-signatures/)
- [RFC 9449 — OAuth 2.0 DPoP](https://datatracker.ietf.org/doc/html/rfc9449) · [DPoP explained — FusionAuth docs](https://fusionauth.io/docs/lifecycle/authenticate-users/oauth/dpop)
- [AWS SigV4 troubleshooting — AWS docs](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv-troubleshooting.html)
- [Revocation in OpenPGP](https://dkg.gitlab.io/openpgp-revocation/)
- [The lethal trifecta for AI agents — Simon Willison](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/) · [New prompt injection papers: Agents Rule of Two / The Attacker Moves Second — Simon Willison](https://simonwillison.net/2025/Nov/2/new-prompt-injection-papers/) · [Agents Rule of Two — Meta AI](https://ai.meta.com/blog/practical-ai-agent-security/)
- [CaMeL prompt injection defense — Afine](https://afine.com/llm-security-prompt-injection-camel)
- [Defending Against Indirect Prompt Injection With Spotlighting — Microsoft Research](https://www.microsoft.com/en-us/research/publication/defending-against-indirect-prompt-injection-attacks-with-spotlighting/)
- [OWASP GenAI LLM01:2025 Prompt Injection](https://genai.owasp.org/llmrisk/llm01-prompt-injection/)
- [Model Context Protocol has prompt injection security problems — Simon Willison](https://simonwillison.net/2025/Apr/9/mcp-prompt-injection/)
- [Polling vs Long Polling vs SSE vs Webhooks — AlgoMaster](https://blog.algomaster.io/p/polling-vs-long-polling-vs-sse-vs-websockets-webhooks) · [When to use Webhooks vs WebSocket vs Pub/Sub vs Polling — Hookdeck](https://hookdeck.com/webhooks/guides/when-to-use-webhooks)
- Project context: `/Users/benlamm/Workspace/FAMP/.planning/PROJECT.md`, `/Users/benlamm/Workspace/FAMP/ARCHITECTURE.md`, `/Users/benlamm/Workspace/FAMP/docs/GATEWAY-SETUP.md`

---
*Feature research for: FAMP v1.1 Open-Internet Federation*
*Researched: 2026-07-30*
