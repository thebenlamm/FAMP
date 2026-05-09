# Claude Code FAMP Context Guide

**Scope:** Practical reference for Claude Code agents using FAMP tools. Covers context-cost
patterns, tool-call sequencing, and known failure modes discovered in production sessions.

---

## The Two Retrieval Flows

FAMP delivers messages through two distinct paths with different context implications.
Mixing them up is the most common source of unnecessary context consumption.

### Flow A — Listen Mode (register with `listen: true`)

```
Hook blocks on famp_await (background)
  → message arrives
  → hook sends wake signal: "New FAMP message from <sender>. Call famp_inbox to read it."
  → Claude calls famp_inbox → full body printed ONCE
  → Claude replies with famp_send
```

**Correct.** One full body retrieval. Use this for dedicated agent windows (e.g., a Sofer mesh
participant, a standing reviewer node).

### Flow B — Manual famp_await (general-purpose windows)

```
Claude calls famp_await directly
  → full envelope body returned in tool result (body printed ONCE here)
  → Claude uses envelope id from result as task_id
  → Claude replies with famp_send
```

**Correct.** One full body retrieval. The body is in the famp_await result — no follow-up
famp_inbox call needed.

### Flow B (broken variant) — the double-print

```
Claude calls famp_await
  → full envelope body returned (body printed ONCE)
Claude calls famp_inbox to "look up" the task_id
  → full body returned AGAIN (body printed TWICE)
Claude replies with famp_send
```

**Wrong.** This doubles the context cost of every received message. The famp_inbox call here
is wasted — it doesn't surface anything famp_await didn't already return, and it reprints the
entire body.

**Why it happens:** The CLAUDE.md Listen Mode section correctly instructs "Call famp_inbox to
read it" — but that instruction is for the wake signal in Flow A, where famp_await ran
silently in the background and did NOT return the body to Claude. An agent that calls
famp_await manually (Flow B) already has the body and must NOT call famp_inbox afterward.

---

## task_id Resolution

The famp_inbox entry for an externally-initiated task may show `task_id: null`. This is not
an error. It means: use the envelope id from the received message as the task_id when
replying.

**From famp_await result:**

```json
{
  "envelope": {
    "id": "019e0d1c-03a4-7b72-9a1f-603c2e1563c0",
    ...
  }
}
```

Use `"019e0d1c-03a4-7b72-9a1f-603c2e1563c0"` as `task_id` in `famp_send` mode `deliver` or
`terminal`. Do not call famp_inbox to look this up — you already have it.

**Correct reply pattern:**

```
famp_send({
  mode: "terminal",
  task_id: "<envelope.id from famp_await result>",
  peer: "<sender>",
  body: "..."
})
```

---

## Context Cost Model

Every FAMP tool call that returns message content prints the full body. There is no
pointer or reference mechanism — long briefings repeat at full length every retrieval.

**Per-session estimate:**

| Component | Context cost |
|---|---|
| Each received message | ≈ body length |
| Each sent message | ≈ body length (stays in context as tool call param) |
| Each unnecessary famp_inbox call | ≈ body length (again) |
| famp_await result | ≈ full envelope (body + headers) |

A three-round discussion with 800-word messages, using the broken flow, costs approximately:

```
3 rounds × 800 words × 3 (await + inbox + send) = 7,200 words ≈ 9,000 tokens
```

Using the correct flow:

```
3 rounds × 800 words × 2 (await + send) = 4,800 words ≈ 6,000 tokens
```

The difference compounds in sessions with heavy session-start overhead (long CLAUDE.md,
large MEMORY.md, project context). FAMP-heavy sessions should budget 20–30% of context
for the protocol layer alone before any work happens.

---

## Design Guidelines for Briefing Authors

The context-cost problem is partially on the sender side. Long briefings are expensive for
every recipient for the lifetime of the task.

**Keep briefing bodies under 500 words where possible.** If a task requires extensive
context, pass it by reference:

- File path: `"Full context: see /path/to/analysis.md"` — recipient reads directly
- Memory key: `"Context stored at brain key: <key>"` — recipient queries
- Summary + offer: state the brief, offer to provide full context if needed

**Structure round-based discussions to narrow per-round.** Round 1 needs full context. Round
2 can reference Round 1 decisions by label rather than restating them. Round 3 should be
tightly scoped to remaining open questions.

---

## Checklist: Before Each famp_send Reply

1. **Do I have the message body?** If yes (from famp_await result), do not call famp_inbox.
2. **Do I have the task_id?** Use `envelope.id` from famp_await result. If null, check
   famp_inbox, but be aware this reprints the body.
3. **Is this the final reply?** Use `mode: "terminal"`. Use `mode: "deliver"` only for
   interim replies where more turns are expected.
4. **Is my reply body proportionate?** If over 600 words, consider whether the body can
   reference an external artifact instead.

---

## Failure Mode Summary

| Symptom | Cause | Fix |
|---|---|---|
| Context at 40%+ after one message | Double-print from famp_await + famp_inbox | Use envelope id from famp_await; skip famp_inbox |
| task_id not found | Looking for it in famp_inbox when it's null | Use envelope.id from famp_await result |
| Long sessions hit context limit mid-discussion | Briefing bodies too long, all retained in context | Pass large context by file reference; shorten per-round bodies |
| famp_inbox returns nothing after famp_await | famp_await already drained the message | Expected; don't call famp_inbox after manual famp_await |

---

## When to Use Listen Mode vs. Manual famp_await

| Situation | Recommended |
|---|---|
| Dedicated agent window, sub-minute response required | `famp_register({ listen: true })` |
| General-purpose dev window, check messages on demand | `famp_register({ listen: false })`, call `famp_await` when ready |
| Multi-round structured discussion (this session) | Manual `famp_await` per round — predictable, no background wake |
| Long-running autonomous agent mesh | Listen mode — don't burn context polling |

---

*Last updated: 2026-05-09. Source: production session — baalshem × matt × torah-graph
integration discussion.*
