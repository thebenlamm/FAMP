---
phase: quick-260810-hac
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - docs/superpowers/specs/2026-08-10-native-wake-ping-design.md
  - crates/famp-bus/src/proto.rs
  - crates/famp-bus/src/broker/state.rs
  - crates/famp-bus/src/broker/handle.rs
  - crates/famp-bus/src/broker/handle/tests.rs
  - crates/famp/src/cli/mcp/tools/register.rs
  - crates/famp/src/cli/mcp/tools/send.rs
  - crates/famp/src/cli/send/mod.rs
  - crates/famp/assets/famp-await.sh
  - crates/famp/tests/hook_runner_await.rs
  - CLAUDE.md
autonomous: true
requirements: [D1, D2, D3, D4, D5]

estimate:
  tokens: 120000
  raw_tokens: 60000
  tasks: 4
  confidence: low

must_haves:
  truths:
    - "A Claude Code agent registered with listen mode gets woken by an inbound local FAMP message in seconds instead of waiting for the next Stop-hook park (D1, D2)."
    - "The wake ping the sending model relays contains zero bytes the peer controls -- only the sender's own validated identity name (D3)."
    - "The Stop hook remains installed and authoritative; Codex senders and gateway-relayed remote messages still wake their recipient exactly as today (D4)."
    - "A famp-await.sh Stop hook whose owning Claude Code session dies stops running instead of surviving as a pid-1 orphan (D5)."
    - "A live session whose owner is still alive keeps its hook parked -- the orphan fix does not silently disable listen mode."
  artifacts:
    - docs/superpowers/specs/2026-08-10-native-wake-ping-design.md
    - crates/famp-bus/src/proto.rs
    - crates/famp/src/cli/mcp/tools/send.rs
    - crates/famp/assets/famp-await.sh
  key_links:
    - "MCP famp_register (parent pid) -> SetWakeAddr frame -> broker holder slot"
    - "broker DM send path -> Delivered.wake_addr -> DeliveredRow -> famp_send tool result -> model calls SendMessage"
    - "famp-await.sh owner-liveness watcher -> fd 9 -> famp await --abort-on-fd 9 -> exit 3 -> hook exits 0"
---

<objective>
Add a second, faster wake path for FAMP listen mode on Claude Code: the MCP server records the
host session's cross-session messaging socket at register time, and `famp_send` hands the sending
model a content-free ping payload to relay via Claude Code's native `SendMessage` tool. Also stop
`famp-await.sh` Stop hooks from outliving the session that spawned them.

Purpose: cut inbound-message latency from "next Stop-hook park" to seconds, and remove the
orphaned-hook load problem that turned a 1.4s test into 302s.

Output: a design spec recording D1-D5 and the REJECTED direct-socket approach; a `SetWakeAddr`
bus frame plus holder slot; a wake-ping payload on the `famp_send` tool result; an owner-liveness
abort in the Stop hook.
</objective>

<execution_context>
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/workflows/execute-plan.md
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@CLAUDE.md

The locked design (D1-D5), the verified spike facts, and the REJECTED direct-socket approach are
reproduced in full inside Task 1's action. Task 1 is the source of truth for Tasks 2-4 -- read the
spec you wrote before starting each subsequent task.

Key source files, already surveyed during planning. Do NOT re-read these wholesale; jump to the
named symbols and line ranges.

- `crates/famp-bus/src/proto.rs` -- `BUS_PROTO_VERSION` (line 29), `BusMessage::Register` (141),
  `BusMessage::SetListen` (219), `BusReply::SetListenOk` (296), `Delivered` (337-358),
  `SessionRow` (362). `SetListen` / `SetListenOk` is the exact structural precedent for the new
  frame. `Delivered.woken` (357) is the exact precedent for the new additive field.
- `crates/famp-bus/src/broker/handle.rs` -- DM send path around lines 475-525 (`woken`, and
  `send_ok` at 1293); channel fan-out rows around 598-606, where `woken` is deliberately false.
- `crates/famp-bus/src/broker/state.rs` -- `ClientStateView.listen_mode` (247). The live
  `ClientState` holder struct lives in the same module.
- `crates/famp/src/cli/mcp/tools/register.rs` -- builds `BusMessage::Register`; `rebind_rejection`
  (222) is the precedent for a pure, unit-testable helper.
- `crates/famp/src/cli/mcp/tools/send.rs` -- `call()` (38) builds the tool result from
  `run_at_structured`; `woken_any` at 64.
- `crates/famp/src/cli/send/mod.rs` -- `SendOutcome.delivered_rows` (139), `DeliveredRow` (147),
  construction site (380).
- `crates/famp/assets/famp-await.sh` -- IN-REPO CANONICAL SOURCE for `~/.claude/hooks/famp-await.sh`,
  rendered per install. Ancestor walk at 314-337; the issue-#21 cancellation seam at 462-587 (fifo
  on fd 9, watcher subshell at 561-570, `run_await` at 581).
- `crates/famp/tests/hook_runner_await.rs` -- reads and renders the asset (lines 19-40) and has a
  mock-`famp` harness for `--abort-on-fd` at 983-1010. New hook tests go here.

Constraints that bite on this repo, all verified during planning:

- `cargo nextest` and `just ci` HANG on this machine. Use plain `cargo test --workspace` and
  `cargo test -p <crate>`, plus individual `just` recipes.
- `cargo test <filter>` exits 0 on ZERO matches. A filtered green run is NOT proof the test ran.
  Always confirm the reported test count is non-zero before believing a filtered pass.
- Five `codex` install/uninstall tests flake under `cargo test --workspace` because they probe
  `target/debug/famp` while cargo relinks it. Not a regression. Re-run them in isolation with
  `cargo test -p famp --lib codex` rather than chasing them mid-plan.
- Rust changes REQUIRE `just lint`, which is `cargo clippy --workspace --all-targets -- -D warnings`.
  A pedantic lint passes tests and still blocks the push.
- `just check-quarantine-surfaces` pins LINE NUMBERS in `.quarantine-surfaces.allow`, and
  `crates/famp-bus/src/broker/handle/tests.rs` is one of the pinned files. APPEND new tests at the
  end of that file; never interleave into pinned regions.
- `crates/famp/assets/famp-await.sh` is covered by `just check-shellcheck`.
</context>

<tasks>

<task type="tracer">
  <name>Task 1: Write the native wake-ping design spec</name>
  <files>docs/superpowers/specs/2026-08-10-native-wake-ping-design.md</files>
  <action>
Create the spec. It is the source of truth Tasks 2-4 read back, so write it first and completely.

Required sections, in this order.

**Problem.** Listen mode wakes an agent via a blocking Stop hook that parks on `famp await` for up
to 23h. Two costs: latency plus one permanently-blocked bash process per listening window; and
orphaned hooks -- when a Claude Code session dies its hook reparents to pid 1 and keeps running for
hours. Sixty orphans once turned a 1.4s test into 302s.

**Verified facts (spike, 2026-08-10).** Record all four:

1. The `famp mcp` parent pid equals the Claude Code session pid, which equals the basename of
   `/tmp/cc-socks/<pid>.sock`. Confirmed 4 of 4 on this box: 8194 to 8091, 5518 to 5393,
   21046 to 21026, 72819 to 72782.
2. `uds:/tmp/cc-socks/<pid>.sock` is a supported `to:` address for Claude Code's `SendMessage`
   tool. It is the literal `from` attribute on inbound cross-session messages.
3. `SendMessage` wakes an idle Claude Code session. A probe to an idle peer returned "ack idle";
   a busy-peer control returned "ack busy". Both arms fired, so the rig was sound and the positive
   result is not a false positive.
4. Not every claude session has a sock, and not every session runs `famp mcp`. The feature must
   degrade silently when the socket is absent.

**Design.** Record D1 through D5 as written:

- **D1.** On `famp_register`, the MCP server computes a wake address of the form
  `uds:/tmp/cc-socks/<parent-pid>.sock` and stores it on the holder -- only if that socket path
  exists on disk. If absent, nothing is stored and everything else behaves exactly as today.
- **D2.** The `famp_send` tool result includes the recipient's wake address when the recipient has
  listen mode ON and has one stored, plus a short instruction line telling the sending model to
  call `SendMessage` to that address.
- **D3. SECURITY INVARIANT -- the ping is CONTENT-FREE.** Peer-controlled message bytes NEVER
  appear in the SendMessage payload. This mirrors the existing rule on the Stop hook's `reason`
  field. The ping text is exactly, character for character including the em dash and the trailing
  period:

  `New FAMP message from <sender> — call famp_inbox to read it.`

  Do not re-punctuate this to match the Stop hook's `reason` text, which is worded differently.
  The sender slot is validated against the same charset regex the hook uses,
  `^[A-Za-z0-9@._:/-]{1,128}$`; on failure the literal `unknown` is substituted.
- **D4. ADDITIVE ONLY -- do not remove or weaken the Stop hook.** Codex senders and
  gateway-delivered remote messages cannot call `SendMessage`, so the hook remains the
  authoritative wake path. The ping is a latency optimization layered on top.
- **D5. Orphan fix.** The hook must exit when its owning Claude Code session dies.

**Reliability story.** State plainly: the ping is best-effort and model-mediated -- the sending
model has to actually make the call. The mailbox is durable. A missed ping means the message waits
for the next hook wake or an explicit `famp_inbox` read. The failure mode is LATENCY, NEVER LOSS.

**REJECTED -- do not re-propose.** Having the `famp-bus` broker, or any FAMP process, connect to
`/tmp/cc-socks/<pid>.sock` directly and speak that protocol. It is an unversioned private IPC
protocol owned by the `claude` binary, currently 2.1.226, which ships updates weekly. Same landmine
class as the GATEWAY-SETUP TLS double-bind. FAMP only ever stores and hands back a string; delivery
always goes through the documented `SendMessage` tool in a live session.

**Implementation notes.** Decisions taken during planning, recorded so they are not re-litigated:

- The wake address travels on a NEW `BusMessage::SetWakeAddr` frame issued right after a successful
  `Register`, mirroring the existing `SetListen` / `SetListenOk` pair -- NOT as a new field on the
  `Register` variant. Rationale: the `Register` variant has 48 construction sites across 27 files
  including the gateway's remote-principal registration, so a new field would create a slot the
  gateway path must remember never to populate; a separate frame means only the MCP register tool
  can ever send one. It also leaves the line-number pins in `.quarantine-surfaces.allow` intact.
  D1 is satisfied literally: it constrains when the value is computed and that it lands on the
  holder, not which frame carries it.
- The wake address is validated BROKER-SIDE, inside the `SetWakeAddr` handler, against
  `^uds:/tmp/cc-socks/[0-9]{1,10}\.sock$`. A non-matching value stores nothing, failing open to
  no-ping. Rationale: any bus client can send this frame; only the broker sees all of them. This
  is D3's invariant applied to the other peer-controlled string, not a new decision.
- The ping payload is composed in Rust and handed to the model whole. The model relays it; it does
  not compose it. If the model composed the text, D3 would be unenforceable.
- `BUS_PROTO_VERSION` bumps 2 to 3. A new message variant is a wire frame change per the mandate in
  that constant's own doc comment. Consequence: after `just install` the daemon must be restarted
  and every live window re-registers, because proto-2 and proto-3 peers reject each other at Hello
  by design. Mailboxes are durable per name, so nothing queued is lost.

**SUPERSEDE -- FAMP issue #21, "listen mode MUST block the turn".** That decision was settled on
the premise that no external input-injection into an idle Claude Code session existed. Verified
fact 3 falsifies that premise with live evidence: `SendMessage` demonstrably wakes an idle session.
Scope the supersede to the PREMISE only -- per D4 the blocking Stop hook remains installed and
authoritative, and the `--abort-on-fd` cancellation seam that issue #21 produced is load-bearing
for D5. Do NOT close issue #21; that is the user's call. Record this as a note addressed to the
user.

**OPEN QUESTIONS.** One item, marked untested: does `SendMessage` wake a session that is currently
PARKED IN THE STOP HOOK, as opposed to idle-at-prompt? This matters for double-wake interaction.
State that it is untested and assert nothing in either direction.
  </action>
  <verify>
    <automated>bash -c 'f=docs/superpowers/specs/2026-08-10-native-wake-ping-design.md; test -f "$f" || exit 1; for h in Problem Design REJECTED SUPERSEDE "OPEN QUESTIONS" "LATENCY, NEVER LOSS"; do grep -qi -- "$h" "$f" || { echo "MISSING: $h"; exit 1; }; done; echo SPEC_OK'</automated>
  </verify>
  <done>The spec file exists and carries Problem, the four verified facts, D1-D5, the reliability
story, the REJECTED section with its unversioned-private-IPC rationale, the implementation notes
(SetWakeAddr frame choice, broker-side validation, Rust-composed payload, proto bump 2 to 3), the
premise-scoped issue-#21 supersede with its evidence, and an OPEN QUESTIONS section that asserts
nothing about the parked-in-hook case. Committed as
`docs(quick-260810-hac): native wake-ping design spec`.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Store the wake address on the holder via a SetWakeAddr bus frame</name>
  <files>crates/famp-bus/src/proto.rs, crates/famp-bus/src/broker/state.rs, crates/famp-bus/src/broker/handle.rs, crates/famp-bus/src/broker/handle/tests.rs, crates/famp/src/cli/mcp/tools/register.rs</files>
  <behavior>
    - A SetWakeAddr frame carrying a value matching the broker regex stores that value on the
      canonical holder, and the reply echoes it back.
    - A SetWakeAddr frame carrying any value that fails the regex stores nothing, and the reply
      echoes nothing -- fail-open to no-ping, never an error that breaks registration.
    - A SetWakeAddr from a proxy (bind_as) connection is rejected with the NotRegistered error
      kind, mirroring SetListen. A proxy must not be able to set another slot's address.
    - The stored value is independent of the listen flag: a holder registered with listen off
      still stores its address, so a later famp_set_listen(true) needs no re-register.
    - BUS_PROTO_VERSION is 3, and a proto-2 client is still rejected at Hello.
    - The MCP register tool sends the frame only when the computed socket path exists on disk.
      When it does not exist, no frame is sent and the famp_register result is byte-identical to
      today's.
    - A failure on the SetWakeAddr round-trip does not fail famp_register.
  </behavior>
  <action>
Implement D1 exactly as recorded in the spec's implementation notes. Read that section first.

In `crates/famp-bus/src/proto.rs`:
- Bump `BUS_PROTO_VERSION` from 2 to 3 and extend its doc comment with a "Bumped 2 -> 3" paragraph
  naming the new frame, matching the style of the existing "Bumped 1 -> 2" paragraph. This is the
  ONLY bump in this plan -- Task 3 ships under the same version and must not bump again.
- Add `BusMessage::SetWakeAddr { wake_addr: Option<String> }` immediately after `SetListen`, and
  `BusReply::SetWakeAddrOk { wake_addr: Option<String> }` immediately after `SetListenOk`. Copy
  `SetListen`'s doc-comment shape, including its statement that proxy connections must not issue
  the frame. Add the new arms to `BusReply::variant_name`.
- Add the validation regex as a module-level LazyLock Regex next to `CHANNEL_RE`, following that
  exact pattern, matching `^uds:/tmp/cc-socks/[0-9]{1,10}\.sock$`.
- Add round-trip tests for the two new variants next to the existing SetListen round-trip tests.

In `crates/famp-bus/src/broker/state.rs`: add an optional wake-address slot to the live holder
struct that already owns `listen_mode`, and surface it on `ClientStateView` alongside `listen_mode`
so the inspector path can see it.

In `crates/famp-bus/src/broker/handle.rs`: add a `set_wake_addr` handler next to the existing
SetListen handler. Reuse that handler's canonical-holder guard verbatim so a proxy connection gets
the NotRegistered error. Run the regex; on a match store the value, on any mismatch store nothing.
Reply with the new Ok variant echoing the post-validation stored value, so the fail-open validation
is observable without inspecting broker internals.

In `crates/famp/src/cli/mcp/tools/register.rs`: after a successful RegisterOk, compute the
candidate address from the MCP process's PARENT pid -- the register tool already uses
`std::process::id()` for the `pid` field, and the parent is the Claude Code session per verified
fact 1. Extract the path-building and existence check into a pure helper in the style of
`rebind_rejection`, taking the pid and a base directory as parameters so it is unit-testable
without a live host. If the socket file does not exist, do nothing further. If it exists, issue the
SetWakeAddr frame; a transport error or an Err reply is logged and swallowed, and registration
still succeeds. Do NOT add a wake-address field to the `famp_register` tool result -- nothing in D1
requires the registering agent to see its own address.

Tests: extend bus-side behavior by APPENDING at the end of `crates/famp-bus/src/broker/handle/tests.rs`.
Do not interleave into existing regions -- that file's line numbers are pinned by
`.quarantine-surfaces.allow`. Cover every bullet in the behavior block above.
  </action>
  <verify>
    <automated>bash -c 'set -o pipefail; cargo test -p famp-bus 2>&1 | tail -25 && cargo test -p famp --lib mcp::tools::register 2>&1 | tail -10 && just lint && just check-quarantine-surfaces'</automated>
  </verify>
  <done>`cargo test -p famp-bus` is green with a NON-ZERO reported test count that includes the new
SetWakeAddr cases -- state the count in the SUMMARY, since a filtered run exits 0 on zero matches.
`just lint` is clean. `just check-quarantine-surfaces` passes; if line numbers shifted despite
appending, regenerate the record block per the documented procedure at the top of
`.quarantine-surfaces.allow` and say so in the SUMMARY. `BUS_PROTO_VERSION` is 3. Committed as
`feat(quick-260810-hac): store host wake address on the bus holder`.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 3: Return a content-free wake ping on the famp_send result</name>
  <files>crates/famp-bus/src/proto.rs, crates/famp-bus/src/broker/handle.rs, crates/famp-bus/src/broker/handle/tests.rs, crates/famp/src/cli/send/mod.rs, crates/famp/src/cli/mcp/tools/send.rs, CLAUDE.md</files>
  <behavior>
    - A DM to a recipient that has listen mode ON and a stored wake address, sent by a client whose
      declared origin is Local, returns that address on the delivery row.
    - The same DM sent by a client whose origin is not Local returns no address -- a
      gateway-relayed remote sender cannot call SendMessage and must not learn a local sock path.
    - A recipient with listen mode OFF returns no address even though one is stored.
    - Channel fan-out rows always return no address, matching the existing woken-is-false
      precedent for channel rows.
    - A Delivered frame produced by a peer that omits the new field deserializes with it absent,
      and a Delivered with all-default fields serializes byte-identically to the pre-change form.
    - The ping-text builder is a pure function: given a sender name it returns exactly the D3
      sentence with that name interpolated.
    - The builder substitutes the literal `unknown` for any sender name failing the D3 charset
      regex, including names carrying newlines, backticks, or an embedded instruction-shaped
      phrase.
    - No field of the outbound envelope can reach the ping text: a hostile message body passed
      through famp_send produces a ping payload whose text is a function of the sender name alone.
  </behavior>
  <action>
Implement D2 and D3. Read the spec's D2, D3, and reliability sections first. Do NOT bump
`BUS_PROTO_VERSION` again -- Task 2 already moved it to 3 and this change ships under that version.

In `crates/famp-bus/src/proto.rs`: add an optional wake-address field to `Delivered` as an additive
field with serde default plus skip-serializing-if-none, following the documented wire-compat
comment style already present on `woken`. Add a round-trip test proving an all-default `Delivered`
still serializes to the exact pre-change JSON, next to the existing back-compat test for `woken`.

In `crates/famp-bus/src/broker/handle.rs`: in the DM delivery path that already computes `woken`,
populate the new field from the RECIPIENT holder's stored value, gated on BOTH the recipient's
listen flag being true AND the SENDING client's declared origin being Local. Channel fan-out rows
keep it absent -- add a short comment pointing at the adjacent rationale for why `woken` is false
on channel rows. Append the new tests at the end of `broker/handle/tests.rs` as in Task 2.

In `crates/famp/src/cli/send/mod.rs`: carry the field through `DeliveredRow` and its construction
site so the CLI structured output surfaces it unchanged.

In `crates/famp/src/cli/mcp/tools/send.rs`: add a pure ping-payload builder in the style of
`rebind_rejection`. It takes the sender identity and the recipient address and returns the payload
object. Its text output is a function of the sender name alone, and it takes no envelope, title, or
body parameter at all -- so the D3 invariant is enforced by the function signature rather than by
discipline. Validate the sender name with the D3 charset regex and fall back to the literal
`unknown` on failure. In `call()`, after a successful send, if exactly one delivery row carries an
address, add a wake-ping object to the tool result holding the target address, the ping text, and a
one-line instruction telling the model to call the SendMessage tool with that address and that
exact text. Keep every existing result field unchanged. When no row carries an address, the result
is byte-identical to today's.

Then update the Listen Mode section of `CLAUDE.md` with two or three sentences documenting the
second wake path: it is best-effort and model-mediated, the Stop hook remains the authoritative
path per D4, and the failure mode is latency rather than loss. Link the spec.

Deployment, required before this task closes because both the MCP tool surface and the bus wire
changed: run `just install`, restart the broker daemon, then re-register this window. Proto-2 and
proto-3 peers reject each other at Hello by design, so every live listening window on the box must
re-register; mailboxes are durable per name, so nothing queued is lost. Record the exact restart
commands you ran in the SUMMARY so the user can repeat them.
  </action>
  <verify>
    <automated>bash -c 'set -o pipefail; cargo test -p famp-bus 2>&1 | tail -25 && cargo test -p famp --lib mcp::tools::send 2>&1 | tail -10 && cargo test --workspace 2>&1 | tail -40 && just lint && just check-quarantine-surfaces'</automated>
  </verify>
  <done>All behavior bullets have a passing test, with the reported counts stated in the SUMMARY
rather than inferred from exit status. `cargo test --workspace` is green apart from the five known
codex install/uninstall relink flakes, which are re-run in isolation with
`cargo test -p famp --lib codex` and confirmed passing there. `just lint` is clean.
`just check-quarantine-surfaces` passes. `just install` has run and `~/.cargo/bin/famp` is newer
than the edited sources. The broker has been restarted and this window re-registered successfully
against the proto-3 daemon. After the restart, one REAL `famp_send` has been issued between two
registered listen-mode windows and the resulting wake-ping object is pasted verbatim into the
SUMMARY -- the generator must not be the only thing that has seen its own output. Committed as
`feat(quick-260810-hac): hand the sending model a content-free wake ping`.</done>
</task>

<task type="auto">
  <name>Task 4: Make the Stop hook exit when its owning session dies</name>
  <files>crates/famp/assets/famp-await.sh, crates/famp/tests/hook_runner_await.rs</files>
  <behavior>
    - Owner dies: the hook aborts its park within roughly two poll intervals and exits 0.
    - CONTROL, owner stays alive: the hook does NOT abort and stays parked. This is the arm that
      matters -- a false abort silently disables listen mode for the rest of the session, which is
      a worse outcome than the orphan it fixes.
    - Owner pid unreadable at capture time, or already the init pid, arms nothing and the hook
      behaves exactly as it does today.
  </behavior>
  <action>
Implement D5 in the IN-REPO canonical asset `crates/famp/assets/famp-await.sh`. Do NOT edit
`~/.claude/hooks/famp-await.sh` directly -- it is outside this repo and is rendered from this asset
at install time.

Reuse the existing issue-#21 cancellation seam rather than adding a second mechanism. The hook
already creates a fifo on fd 9, runs a polling watcher subshell, and passes `--abort-on-fd 9` to
the pinned await; a byte on fd 9 makes `famp await` return exit 3, which the hook already maps to a
clean exit 0.

Capture the owner pid once at hook start, before the watcher is armed, by reading the hook's own
parent pid. Note that bash does not reset `$$` inside a subshell, so the watcher subshell can read
the hook's current parent pid the same way. Add an owner-liveness predicate to the SAME polling
loop that already evaluates the queue predicate: if the hook's current parent pid differs from the
captured value, the owner is gone -- the hook has been reparented -- so write the abort byte and
break. Prefer reparent detection over a bare liveness probe as the primary signal, because it
cannot be fooled by pid reuse; a liveness probe on the captured pid may be added as a secondary
signal only if it does not weaken the control arm.

Fail-open in the same style as the surrounding code: if the parent pid cannot be read at capture
time, or is already the init pid, do not arm the liveness predicate at all and leave every other
behavior untouched. Never abort on uncertainty.

Tests go in `crates/famp/tests/hook_runner_await.rs`, using the existing mock-`famp` harness for
`--abort-on-fd` near line 983 -- that mock reads one byte from the abort fd, which is exactly the
signal under test. Write BOTH arms as a matched pair: a kill-the-parent arm that must abort, and a
parent-stays-alive control arm that must NOT abort. A single-arm test here is worthless, because a
hook that aborts unconditionally passes the kill arm.

Because the hook is rendered into the user's home directory outside this repo, the SUMMARY must
carry the exact commands the user runs to pick up the fix -- `just install` followed by whatever
hook-wiring step the installer performs -- plus a one-line note that already-orphaned hooks from
before this fix must be killed by hand, and the command to count them.
  </action>
  <verify>
    <automated>bash -c 'set -o pipefail; cargo test -p famp --test hook_runner_await 2>&1 | tail -25 && just check-shellcheck'</automated>
  </verify>
  <done>Both arms pass: the kill-parent arm aborts within roughly two poll intervals, and the
control arm stays parked. The reported test count is non-zero and stated in the SUMMARY.
`just check-shellcheck` passes on the edited asset. The SUMMARY carries the user-side commands to
adopt the fix and to count pre-existing orphans. Committed as
`fix(quick-260810-hac): exit the listen-mode Stop hook when its session dies`.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| peer agent -> local broker | Any bus client can send frames, including a wake address string and a message body it fully controls. |
| broker -> sending model | The famp_send tool result is read by an LLM as instructions; anything in it is an injection surface. |
| dead session -> host machine | An orphaned hook keeps consuming CPU and process slots after its owner is gone. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-hac-01 | Tampering | famp_send tool result | high | mitigate | D3: ping text is built by a pure Rust function whose signature accepts only the sender name -- no envelope, title, or body parameter exists, so peer bytes cannot reach it (Task 3). |
| T-hac-02 | Spoofing | SetWakeAddr handler | high | mitigate | Proxy (bind_as) connections are rejected with NotRegistered, reusing SetListen's canonical-holder guard, so no client can set another slot's address (Task 2). |
| T-hac-03 | Elevation of Privilege | SetWakeAddr handler | high | mitigate | Broker-side regex pins the address to the cc-socks socket shape; any other value stores nothing. Enforced at the broker, the layer that sees every client (Task 2). |
| T-hac-04 | Information Disclosure | broker DM delivery path | medium | mitigate | The address is returned only when the sending client's declared origin is Local, so a gateway-relayed remote sender never learns a local sock path (Task 3). |
| T-hac-05 | Denial of Service | famp-await.sh | high | mitigate | Owner-liveness predicate on the existing fd-9 abort seam terminates the hook when its session dies; a paired control arm proves it does not fire while the owner lives (Task 4). |
| T-hac-06 | Denial of Service | listen mode overall | medium | accept | A false-positive abort would disable listen mode for a session. Accepted because the control-arm test gates it and the fail-open design never aborts on uncertainty; the residual failure mode is latency, not loss. |
| T-hac-SC | Tampering | dependency installs | low | accept | No new package-manager installs in this plan -- every change uses crates already in the workspace, so the legitimacy gate does not fire. |
</threat_model>

<verification>
- The spec exists and records D1-D5, the REJECTED approach, the premise-scoped issue-#21 supersede,
  and an OPEN QUESTIONS section that asserts nothing about the parked-in-hook case.
- `BUS_PROTO_VERSION` is 3, bumped exactly once across the whole plan.
- `cargo test --workspace` is green apart from the five known codex relink flakes, which pass in
  isolation. Reported test counts are stated, never inferred from exit status.
- `just lint`, `just check-quarantine-surfaces`, and `just check-shellcheck` all pass.
- `just install` has run and the broker has been restarted; this window re-registers cleanly.
- Every commit is atomic and scoped to one task.
</verification>

<success_criteria>
- A local DM between two listen-mode Claude Code windows on this box produces a wake-ping payload
  on the sender's tool result, and relaying it via SendMessage wakes the recipient.
- A hostile sender name or message body cannot alter the ping text, proven by a test.
- A gateway-relayed remote sender receives no wake address, proven by a test.
- Killing a session's owner process terminates its parked hook; leaving the owner alive does not,
  both proven by a matched test pair.
- The Stop hook remains installed and functional as the authoritative wake path.
</success_criteria>

<output>
Create `.planning/quick/260810-hac-add-native-claude-code-wake-ping-to-famp/260810-hac-SUMMARY.md` when done.
</output>
