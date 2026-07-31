# The Inbound-Content-Is-DATA Quarantine (Phase 14, QUAR-01..11)

## What this delivers

**FAMP delivers machine-checkable provenance and untrusted-marking at every rendering surface that can show received content. Steering-resistance remains the harness's job.**

This claim is worded to stay true whether or not a future harness ships tool-gating on top of it: this phase's job is to make "was this content received from outside this host" a fact any consumer can check mechanically, not to itself stop a local agent from acting on untrusted content.

## What this does NOT do

**This does not prevent a remote agent from steering a local agent by sending it text.**

Say it in exactly those terms — no qualifier softens this into a partial claim. A remote agent can still cause a local agent to take unwanted actions purely through the content of a message it sends, exactly as it could before this phase.

## Why not — the corrected threat model (D-12/D-13)

An earlier justification for scoping this phase to provenance-only argued that FAMP v1.1 is "conversation-only," so the tool leg of the lethal trifecta (untrusted content + private data + tools + automatic ingestion) is removed on the receiving side. **That justification is false, and it is retracted here rather than quietly dropped.**

Conversation-only removes a remote sender's ability to trigger *FAMP* tools directly. It does not touch the receiving end at all: the receiving end is a listen-mode Claude Code (or Codex) session that auto-wakes on inbound messages, is instructed to call `famp_inbox`, and holds its **own full local toolset** — Bash, file access, `famp_send` for exfiltration, and whatever else the harness grants it. Untrusted content, private data, tools, and automatic ingestion are all present **on the recipient's side**. Removing the attacker's direct tool-invocation path did not remove the steered-local-agent path — the trifecta reassembles around the local agent's own toolset regardless of what the remote sender can invoke.

This does not change the implementation this phase ships (provenance stamping, fail-closed defaults, mechanical rendering-surface coverage, the adversarial corpus — none of that moved). It changes what this document is allowed to claim about the result.

## Known-insufficient mitigations

These are all real defenses against a naive attack, and all are explicitly insufficient against a capable, persistent attacker. Each is present in this implementation as defense in depth, not as the boundary itself.

- **Delimiter-emission.** An attacker who can print the exact quarantine delimiter (or a close guess) can attempt to forge a fake block close and smuggle content past a naive parser that just scans for the closing marker. The per-render UUIDv7-tail nonce raises the cost of this — a guessed or replayed nonce is rejected — but it does not bound a capable attacker who gets unlimited retries against a long-lived listener; it only rules out the cheapest version of the attack.
- **Prompt-level "treat the following as data" mitigation.** Instructing the model in prose that enclosed text is data, not instructions, is named insufficient by OWASP LLM01:2025 and the broader spotlighting literature: mitigation that lives entirely inside the model's own context is not a boundary the model itself can be forced to respect, because the same channel that carries the instruction also carries the attack.
- **Structural tagging generally** (what this phase actually implements — the quarantine-wrap marker plus the machine-readable `origin` field). This is spotlighting / datamarking. Published consensus is that spotlighting raises attack cost and does not bound a capable attacker — it is the reason this phase is honest about delivering provenance rather than claiming a steering boundary.

## The one-hop laundering limitation (QUAR-11)

The origin stamp tracks how content **entered this local bus**, not where it originally came from once a local agent re-authors it. Quoting `crates/famp/tests/quarantine/laundering.rs`'s module doc comment verbatim, so the documented limitation and the passing test that proves it cannot drift apart:

> FAMP's origin stamp tracks how content ENTERED this local bus — the broker stamps `origin: gateway` on the mailbox append that receives a federation-relayed message (D-02), and every rendering surface marks that content as untrusted DATA (D-07/D-09). It does NOT — and cannot, without content provenance far beyond this phase's scope — track where content originally came from once a LOCAL agent re-authors it.
>
> Concretely: if remote-tagged content reaches local agent A, and A then quotes or paraphrases that content into a NEW message A sends to local agent B, B's copy is a message from A — a local sender — so it arrives at B stamped `origin: local` and renders verbatim, unmarked. The content survived the hop; the provenance did not. An agent that re-emits remote text launders its provenance.

Executable proof: `crates/famp/tests/quarantine/laundering.rs::laundered_remote_content_arrives_at_second_hop_untagged`. This test **passes** — it pins the limitation, it does not assert the limitation's absence.

## What IS mechanically guaranteed

Each guarantee below names the test that proves it, so a claim here cannot silently drift out of sync with the code:

- **Fail-closed stamping at every mailbox append.** The broker stamps every `Out::AppendMailbox` explicitly with the sending connection's declared origin; there is no code path that upgrades absence of a stamp into trust. Proven by `crates/famp-bus/src/broker/handle/tests.rs::register_without_origin_field_resolves_unknown` and `crates/famp-bus/src/origin.rs::origin_default_is_unknown`.
- **Unknown-not-local on every absence path.** A missing `Register.origin` field, a legacy pre-Phase-14 mailbox line, an unrecognized on-disk shape, or an unrecognized origin string all resolve to `Origin::Unknown`, never `Origin::Local`. Proven by `crates/famp-bus/src/origin.rs::split_stamped_legacy_bare_envelope_resolves_unknown`, `::split_stamped_unknown_origin_string_resolves_unknown`, and `::split_stamped_extra_key_resolves_unknown`, and re-proven over a real socket (not just in-process serde) by `crates/famp/tests/quarantine_skew.rs::skew_unstamped_mailbox_record_renders_untrusted` and `::skew_register_without_origin_produces_marked_records`.
- **All seven rendering surfaces marked through one helper.** `famp_inbox`, `famp_await`, `famp_channel_log`, CLI `inbox list`, CLI `await`, CLI `register --tail`, and CLI `wait-reply` all reach `famp::cli::render::render_envelope_body`/`render_body_text` — the one shared implementation (D-07), not N ad-hoc copies. Proven by the 13 tests in `crates/famp/tests/quarantine_surfaces.rs`, including `await_marks_gateway_origin`, `wait_reply_marks_gateway_origin`, `register_tail_marks_gateway_origin`, and `channel_log_marks_gateway_origin`.
- **A regression gate that goes red on an unregistered eighth surface.** The rendering-surface list is generated by a mechanical query (`scripts/quarantine-surfaces.sh`), not hand-curated, and `just check-quarantine-surfaces` fails CI if a new call site reaching received content is added without being accounted for. Proven non-vacuous by `crates/famp/tests/quarantine_gate.rs::gate_goes_red_on_an_unregistered_surface`, which fabricates an unregistered call site, shows the gate go red with the filename in its output, then green again.
- **The wake payload carries no attacker body text.** `famp-await.sh`'s wake-up notification never carries the message body, across the full 24-case adversarial corpus. Proven by `crates/famp/tests/quarantine_surfaces.rs::wake_payload_emits_no_body_text` and `crates/famp/tests/quarantine_corpus.rs::corpus_wake_payload_carries_no_case_body`.
- **The corpus is non-vacuous.** A falsification control was actually run, not just written: reverting `render_envelope_body`'s marking behavior makes `corpus_delimiter_emission_cannot_forge_a_block_close` genuinely fail while `corpus_local_origin_renders_verbatim` stays genuinely green, in both directions. Full captured output: `.planning/phases/14-inbound-content-is-data-quarantine/14-FALSIFICATION.md`.

## Unforgeability: the correct mechanism

The origin stamp cannot ride inside the envelope's own JSON `Value` — an attacker cannot pre-embed an `origin` key inside the envelope body and have it survive to be read as authoritative. The reason is `WireEnvelope`'s `#[serde(deny_unknown_fields)]` at decode time (`crates/famp-envelope/src/wire.rs`): any envelope carrying a field the schema does not recognize is rejected outright, so a forged `origin` key inside the envelope body simply fails to decode.

**`strip_relay_fields` is explicitly NOT the mechanism, and an earlier explanation attributing unforgeability to it was wrong — corrected here rather than repeated.** `strip_relay_fields` (`crates/famp-gateway/src/ingress.rs`) removes exactly eight named federation-wrapper fields by exact key match before the local bus write; it does nothing about a field name it doesn't already know, and a field named `origin` is not on that list. If the stamp had been designed to live inside the envelope `Value`, `strip_relay_fields` would not have stopped an attacker from planting it there. The real mechanism is `WireEnvelope`'s decode-time strictness — which is exactly why the stamp instead rides an additive field on `famp-bus`'s `Register` frame (Layer 1, not the frozen `famp-envelope`), a location an attacker cannot reach by crafting envelope content at all.

## What would make this a real boundary

None of the following are implemented in this phase. Provenance is the prerequisite that makes each of them possible — before this phase, none of them had a reliable signal to act on, because `strip_relay_fields` erased every trace of remote origin before the local bus ever saw it.

- **A `PreToolUse` hook that blocks tool calls once remote-tagged content has entered the turn.** Consistent with FAMP's existing hook surface (`famp-await.sh`, `famp hook`), but Claude-Code-specific rather than harness-agnostic.
- **Rendering remote bodies only through a quarantined summarizer** that strips actionable structure before the content ever reaches the agent's main context.
- **A listener profile with tools disabled** — a mode a recipient can opt into where inbound content is readable but no local tool execution is available in the same turn.

The choice among these (and whether to build any of them at all) was explicitly deferred as a scope decision — see the "Open flag for Ben" note in `.planning/phases/14-inbound-content-is-data-quarantine/14-05-SUMMARY.md`.
