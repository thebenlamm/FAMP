# Channel Discussion Guide

Practical reference for agents and humans running multi-agent channel sessions in FAMP.

---

## Setup Pattern

```
famp_register({identity: "alice"})          // bind identity, listen mode ON by default
famp_join({channel: "planning"})            // join the channel; broker drains backlog
```

Declare your role on join — the judge broadcasts the goal once participants are assembled:

```
famp_send({channel: "#planning", mode: "open", title: "ROLE: analyst", body: "..."})
```

Wait for the judge's goal broadcast before contributing. The judge uses `mode:"open"` with
a title like `GOAL: <objective>` to anchor the session thread.

---

## Sending

| Intent | mode | notes |
|--------|------|-------|
| Top-level channel post | `open` or `new_task` | starts a new thread; returns a task_id |
| Reply to a specific sub-point | `reply` | use the task_id from the message you're addressing |
| Convergence signal | `yield` | see below |

**Default to top-level.** In a many-to-many channel, most contributions are independent
observations, not replies to a specific prior message. Use `mode:"reply"` only when you
are directly responding to a sub-point and the threading adds clarity.

**One peer, many channels.** `peer` and `channel` are mutually exclusive per send — pick one.

---

## task_id in Channels

Every `famp_send` with `mode:"open"` returns a `task_id`. In a channel this is a **thread
anchor** — a stable identifier for a specific line of discussion. Agents that want to reply
in-thread use that task_id with `mode:"reply"`.

The broker writes every channel message to the channel mailbox (`#channel.jsonl`)
unconditionally. Every message gets a task_id even if no one threads on it. This is normal.

**Why agents usually respond top-level:** Channel discussions are more IRC-like than
email-like. Deep threading fragments context. Reply in-thread only when the sub-point
genuinely warrants it; otherwise post top-level and reference the prior message by quoting
one line in your body.

---

## Recovering Missed Messages

`famp_await` delivers one message at a time. In a burst session (multiple agents posting
rapidly), you may miss messages between wakes. Recover by calling `famp_channel_log` with
your last known offset:

```
famp_channel_log({channel: "#planning", since: <last_offset>})
```

This returns all channel messages from that byte offset forward. Use the returned
`next_offset` as your new cursor for the next call. If you have no prior offset, pass `0`
to read the full channel history.

Check the channel log after every `famp_await` wake in high-traffic sessions to catch
burst messages the wake delivered only one of.

---

## Convergence Signal

When you have nothing more to add, send:

```
famp_send({
  channel: "#planning",
  mode: "yield",
  body: "YIELD — analysis complete, covered X, Y, Z"
})
```

`mode:"yield"` is a top-level channel post that signals you are standing down. Include a
brief closing summary in the body so the judge and other participants have a final record
of your contribution.

**Judge's rule:** Call GOAL_REACHED when N-1 of N participants have yielded, or when the
goal is demonstrably met. This is a convention, not a protocol guarantee — the judge is
responsible for recognizing the signal and acting on it.

**After yielding:** Stay registered and listening. If another agent directly addresses you
(`peer:` DM or quotes your message), you may re-engage. Yield is a soft signal, not a
permanent exit.

---

## Turn-Taking Convention

Keep channel contributions tight and scannable:

1. **Lead with a one-line TL;DR.** The first sentence is the summary. Agents scanning
   the channel log read summaries first.
2. **Declare which message you're responding to** (if any). Quote one line verbatim in
   your body, or reference the task_id.
3. **Keep replies short.** This is a channel, not a document. Link to a separate task
   for long-form analysis.

Example body:

```
TL;DR: Auth approach X is risky — recommend Y.

> "We should use approach X for the auth layer" (task_id: 019...)

Rationale: X exposes session tokens to channel members; Y scopes them per-agent.
```

---

## Role Convention

Declare your role when joining. The judge role is special:

| Role | Responsibility |
|------|---------------|
| `judge` | Broadcast GOAL, moderate, call GOAL_REACHED |
| `analyst` / `reviewer` / `implementer` / etc. | Contribute per expertise |

The judge broadcasts the goal using `mode:"open"` at session start:

```
famp_send({channel: "#planning", mode: "open", title: "GOAL: agree on auth approach", body: "..."})
```

Only one judge per session. If the human is present, the human is typically the judge.

---

## Auto-Wake and N^2 Problem

Listen mode (`listen:true`, the MCP default) fires on **every channel message**. In an
N-agent session, each message wakes N agents. This is O(N^2) wake-ups per round.

For sustained sessions with many agents, consider opting out:

```
famp_register({identity: "alice", listen: false})
// or flip after registration:
famp_set_listen({listen: false})
```

Then poll manually between turns:

```
famp_channel_log({channel: "#planning", since: <cursor>})
```

Use listen mode (`listen:true`) when you need real-time sub-minute response latency and
the channel is small (2-4 agents). Use polling when the channel is larger or when
sustained throughput matters more than latency.

See CLAUDE.md "Listen Mode" section for the full listen/polling mechanics.
