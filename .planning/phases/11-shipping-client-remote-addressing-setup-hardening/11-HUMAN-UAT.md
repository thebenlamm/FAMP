---
status: passed
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 06
requirement: UAT-01
gate: blocking-human
started: 2026-07-29T01:45:00Z
updated: 2026-07-29T02:01:00Z
verdict: PASS
---

# UAT-01 — Two-Machine Dogfood with the Fixed `famp send` (No Injector)

**Verdict: PASS.** `famp send --to agent:<peer-domain>/<name>` from machine A
delivered into the real remote agent's mailbox on machine B, and the task FSM
reached a **terminal state (COMPLETED) on both machines**, driven end-to-end by
the shipping client with **no hand-written injector**.

This closes the v1.0 blocker recorded in
`project_v10_no_shipping_client_addresses_remote` — "no shipping client can
address a remote principal."

Run at Ben's explicit direction (2026-07-29). The plan marks this gate
`blocking-human` / "do not auto-approve"; it was not auto-approved — Ben
directed the run after being shown the gate.

---

## 1. Topology

| Role | Host | OS | Tailnet IP | Own-domain | Local identity | Gateway backs |
|------|------|----|-----------|-----------|----------------|---------------|
| A | `bens-macbook-air` (Mac) | Darwin arm64 | 100.99.215.33 | `mac.famp` | `alice` | `bob` (REMOTE) |
| B | `home-devbox` | Linux x86_64 | 100.112.29.111 | `devbox.famp` | `bob` | `alice` (REMOTE) |

Transport: Tailscale tailnet, HTTPS on `:8443` both ways.

**Each gateway backs the REMOTE principal** — A backs `bob`, B backs `alice` —
per the corrected `docs/GATEWAY-SETUP.md` §4. This is the exact wiring the Gate A
dogfood got backwards and plan 11-05 fixed. The doc as corrected was followed
literally and worked first try.

### Isolation deviation (deliberate)

The dogfood ran under a dedicated `FAMP_HOME=~/famp-uat11` and a dedicated broker
socket (`~/famp-uat11/bus.sock`) on **both** hosts, rather than the default
`~/.famp` + production broker.

Rationale: Ben's live FAMP mesh (broker pid 743 plus three `famp mcp` sessions)
was running on the Mac. Installing fresh binaries requires a broker restart,
which would have dropped those live holders. The isolated home exercises the
identical deployed binaries over the identical real network path, so nothing
about the test's fidelity is reduced — only the blast radius. **The production
broker was never restarted and never touched.**

---

## 2. Task 1 — Binary freshness (both hosts)

Deployed via `just install-all` (Mac) and the equivalent raw cargo commands on
devbox (`just` is not installed there):
`cargo install --path crates/famp --locked --force` +
`cargo install --path crates/famp-gateway --locked --force`.

| Host | Binary | SHA-256 (full) | Built |
|------|--------|----------------|-------|
| A (Mac) | `~/.cargo/bin/famp` | `7b931b40aac1e4e6ea2b7a78a63941ce75e4571007a5ed017df69898cf87b044` | 2026-07-28 21:48 |
| A (Mac) | `~/.cargo/bin/famp-gateway` | `4544499d7acedd7c8d0307603b35ee2b20d9d4432279065e6333e846d6cf3ff1` | 2026-07-28 21:49 |
| A (Mac) | `target/release/famp-gateway` | `4544499d7acedd7c8d0307603b35ee2b20d9d4432279065e6333e846d6cf3ff1` | 2026-07-28 21:5x |
| B (devbox) | `~/.cargo/bin/famp` | `e21d8bb720fff286b7b3ee1c681b7556b6fb4b36a38c285516a2a6218ef3a79c` | 2026-07-29 01:5x |
| B (devbox) | `~/.cargo/bin/famp-gateway` | `2e53c705612147ef31ebc108524979cc4ab3152733e01990dadecd07207eb385` | 2026-07-29 01:5x |

Source commit on both hosts at build time: **`0184f01`** (devbox was synced from
the Mac over the tailnet via `git bundle`, not via `origin` — nothing was pushed
to GitHub for this dogfood). The Mac has since advanced to `f8506cb`, which is
**test-only** (the startup-deadline sweep) and does not affect either shipping
binary.

Both binaries postdate every phase-11 source change (plans 01/02/03/07/08 land in
`crates/famp/src/cli/send/mod.rs`, `own_domain.rs`, and
`crates/famp-gateway/src/{egress,ingress,main}.rs`).

### macOS firewall — finding #8, handled without sudo

`socketfilterfw --getglobalstate` reported the firewall **enabled**, and `sudo`
required an interactive password that was not available to the agent session.
`--listapps` showed an existing allow rule for
`/Users/benlamm/Workspace/FAMP/target/release/famp-gateway` — but **not** for
`~/.cargo/bin/famp-gateway`, a different path the rule does not cover.

Resolution: rebuilt `target/release/famp-gateway` and confirmed it is
**byte-identical** to the deployed `~/.cargo/bin/famp-gateway`
(`4544499d…` both), then ran gateway A from the already-authorized path. Running
the allow-listed path *is* running the deployed binary, so freshness is preserved
with no firewall change and no sudo.

Inbound to the Mac was then proven empirically — B's `commit` and `deliver` legs
both arrived (§4), which is only possible if inbound TCP to gateway A was
accepted.

### Task 1 automated verification deviation

The plan specifies `just ci` for Task 1. `just ci` is **unusable on this machine**
— it runs `cargo nextest`, which stalls indefinitely in the test-binary `--list`
phase (recorded in `project_nextest_list_hang`). A full `cargo test --workspace`
also exceeded a 900s timeout. Substituted, all green:

- `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) — clean
- `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery` — 2/2 pass
- `cargo test -p famp --test gateway_setup_doc_accuracy` — pass

**The full workspace suite was NOT run to completion. CI remains the real gate.**

---

## 3. Setup executed (following corrected `docs/GATEWAY-SETUP.md`)

1. **Certs** — generated per §1's CA:FALSE + `serverAuth` recipe on each host with
   that host's own tailnet IP in the SAN. Verified shape on both:
   `CA:FALSE` critical, `TLS Web Server Authentication`, `IP Address:<tailnet-ip>`.
   Certs exchanged for `--trust-cert` (self-signed leaves, no shared CA).
2. **Own-domain** — `echo mac.famp > ~/famp-uat11/own-domain` (A),
   `echo devbox.famp > ~/famp-uat11/own-domain` (B). §5's file source.
3. **Key exchange (§3)** — exported under the **sender AGENT principal**, not a
   `/gateway` label:
   - A: `famp peer export --as agent:mac.famp/alice`
     → `agent:mac.famp/alice _xGQXovBWYqerZyJTFZH4lA43qECerIFLm5QVlXb57E RGOYGVXBdHB5fOrC`
   - B: `famp peer export --as agent:devbox.famp/bob`
     → `agent:devbox.famp/bob HYdHRs9ieJv2Hcm1xcaGJSBvDQ1f5PGXTBHq10VTiOE Gexao6MToG0HOuP0`

   Fingerprints eyeball-checked across the transfer; both matched. Pinned in both
   directions **before** either gateway started (§3's no-hot-reload rule).
   Final keyrings — exactly one entry each, no duplicate-pubkey brick:
   - A: `agent:devbox.famp/bob  HYdHRs9ie…`
   - B: `agent:mac.famp/alice  _xGQXovBW…`
4. **Gateways (§4)** — each backing the REMOTE principal. Both printed
   `famp-gateway: ready, backing 1 principal(s): …` **after** the keyring load
   (plan 11-07's ready-line move), and both stayed up.

---

## 4. Task 2 — The dogfood

Task under test: **`019fab97-d3e0-7d63-92ba-39f1ce171b83`**

### Leg 1 — A opens the task (request)

```
famp send --as alice --to agent:devbox.famp/bob \
  --new-task "UAT-01 phase 11 two-machine dogfood" --body "hello from mac.famp/alice"
→ {"delivered":"[Delivered { to: Agent { name: \"bob\" }, ok: true, woken: true }]",
   "task_id":"019fab97-d3e0-7d63-92ba-39f1ce171b83"}
```

Landed in **bob's real mailbox on devbox** (not a proxy stand-in):

```
FROM                  TO                     CLASS    STATE      TIMESTAMP
agent:mac.famp/alice  agent:devbox.famp/bob  request  REQUESTED  2026-07-29T01:58:01Z
```

`famp inspect identities` on B: `bob  unread=1 total=1 last_sender=agent:mac.famp/alice`.

Raw delivered envelope on B confirms the shape plan 11-03 ships — a typed
`request` (not `audit_log`), domain-qualified both ways, and with the
federation-owned signature fields correctly **stripped at ingress** per plan
11-08's F-2 (gateway is sole writer of those fields):

```json
{"authority":"advisory","body":{"bounds":{"budget":{"amount":"0","unit":"usd"},"hop_limit":8},
"natural_language_summary":"UAT-01 phase 11 two-machine dogfood","scope":{}},
"class":"request","famp":"0.5.2","from":"agent:mac.famp/alice",
"id":"019fab97-d3e0-7d63-92ba-39f1ce171b83","scope":"standalone",
"to":"agent:devbox.famp/bob","ts":"2026-07-29T01:58:01Z"}
```

### Legs 2 & 3 — B drives the FSM to terminal

```
famp send --as bob --to agent:mac.famp/alice --task 019fab97-… --body "bob commits to the task"
→ task_id 019fab99-52bf-…, thread_task_id 019fab97-…      (class commit → COMMITTED)

famp send --as bob --to agent:mac.famp/alice --task 019fab97-… --terminal --body "bob completes the task"
→ task_id 019fab99-6271-…, thread_task_id 019fab97-…      (class deliver + terminal_status=completed → COMPLETED)
```

Both landed back on A, correctly threaded via `causality.ref`:

```
FROM                   TO                    TASK_ID    CLASS    STATE
agent:devbox.famp/bob  agent:mac.famp/alice  019fab97-… commit   COMMITTED
agent:devbox.famp/bob  agent:mac.famp/alice  019fab97-… deliver  COMPLETED
```

### Terminal FSM on BOTH machines — the gate

**Machine A:**
```
TASK_ID                               STATE      PEER                  ENVELOPES
019fab97-d3e0-7d63-92ba-39f1ce171b83  COMPLETED  agent:mac.famp/alice  2
```

**Machine B:**
```
TASK_ID                               STATE      PEER                  ENVELOPES
019fab97-d3e0-7d63-92ba-39f1ce171b83  COMPLETED  agent:mac.famp/alice  2
```

B's `--id … --json` detail additionally reports **`"sig_verified": true`** on both
envelopes — Ed25519 verification confirmed on the federated path, not assumed.

REQUESTED → COMMITTED → COMPLETED, converging to the same terminal state on both
hosts. **Gate satisfied.**

### Reverse direction

`famp send --as bob --to agent:mac.famp/alice --new-task "reverse-direction UAT probe"`
delivered into alice's real mailbox on A (`class request`, `REQUESTED`,
`2026-07-29T02:00:29Z`), exercising B's egress and A's ingress for an *opening*
request — a path the first three legs did not cover. Bidirectional delivery
confirmed.

---

## 5. Findings

### F-A (minor, non-blocking) — an opening `request` is not task-indexed until a threaded reply arrives

Immediately after leg 1, `famp inspect tasks` was **empty on both hosts** and
`inspect messages` showed `task_id: ""` for the delivered `request`, even though
the envelope carried the task id as its `id` field.

Falsified with a control, both poles named:
- **Must-pass:** a purely local send (`--to carol`, bare name) → `inspect tasks`
  shows the task with `task_id` populated. **Passed.**
- **Must-fail-if-real:** the remote send → `inspect tasks` empty, `task_id` `""`.
  **Failed as predicted.**

Root cause is *not* remote-vs-local and not the class change: the task index keys
on **`causality.ref`**. An opening `request` has no `causality.ref` (it *is* the
task root), so it is not indexed until a `commit`/`deliver` referencing it
arrives. Once legs 2–3 landed, both hosts indexed the task correctly and showed
COMPLETED.

Impact: an *open, unanswered* remote task is invisible to `famp inspect tasks`.
Delivery, verification, and the FSM are all unaffected. Does not block UAT-01 —
the gate's requirement (terminal state observable on both sides) is met.
Worth a follow-up: index the request root on receipt so a pending task is
observable before it is answered.

### F-B (informational) — firewall allow-rules are path-bound

The existing macOS allow rule covered `target/release/famp-gateway` but not
`~/.cargo/bin/famp-gateway`. Anyone following GATEWAY-SETUP.md §1 who previously
approved a `target/` build will silently *not* be covered after `just install-all`
moves them to the `~/.cargo/bin` path. Worth one sentence in §1.

### F-C (informational) — `just ci` cannot serve as a UAT gate on this machine

See §2. The plan's Task 1 `<automated>just ci</automated>` is not runnable
locally. Any future plan specifying `just ci` as its verification should name the
targeted substitutes instead.

---

## 6. Verdict

**PASS** — UAT-01 satisfied.

- ✅ Real shipping `famp send` addressed a remote principal; no injector
- ✅ Delivered into the real remote agent's mailbox on a physically separate machine
- ✅ Task FSM reached COMPLETED on **both** machines
- ✅ `sig_verified: true` on the federated path
- ✅ Bidirectional (A→B and B→A opening requests both delivered)
- ✅ Corrected GATEWAY-SETUP.md followed literally and worked first try
- ⚠️ One minor observability gap (F-A), non-blocking

This unblocks tagging **v1.0.0-rc.1**. Per the phase decision record, `v1.0.0`
itself still requires design review C's §16 nine-item checklist.
