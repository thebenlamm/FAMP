# Adversarial Behavior-Divergence Review — 3 refactor targets

Date: 2026-06-06. Method: read-only, reasoning from source. Scope: behavior
divergence only (not style/naming/pure moves).

Reference — EnvelopeView accessors (`crates/famp-envelope/src/view.rs`):
- `from_str()` = `value.get("from").and_then(Value::as_str)` — byte-identical to raw.
- `to_str()`   = `value.get("to").and_then(Value::as_str)` — byte-identical to raw.
- `class()`    = `value.get("class").and_then(Value::as_str)` — byte-identical to raw.
- `body()`     = `value.get("body")` → `Option<&Value>` — byte-identical to raw `.get("body")`.
- `task_id()`  = causality.ref → body.details.task → (id iff body.event==new_task) → None — verbatim copy of deleted `envelope_task_id`, with an in-crate equivalence property test.

---

## TARGET 1 — §2b envelope-read migration (c1917fc + 74964a6)

### VERDICT: CLEAN

Every migrated site is byte-equivalent. The accessors are literal copies of the
raw expressions they replace, so absent-vs-present-but-non-string-vs-string all
resolve identically (all collapse to `None`/default).

Per-site check:

| File:func | field | before | after | equiv? |
|---|---|---|---|---|
| messages.rs `message_row` | body | `get("body").cloned().unwrap_or(Null)` | `view.body().cloned().unwrap_or(Null)` | ✓ |
| messages.rs `message_row` | from/to/class | `get(x).and_then(as_str).unwrap_or("")` | `view.{from_str,to_str,class}().unwrap_or("")` | ✓ |
| messages.rs `message_row` | task_id | `envelope_task_id(env).unwrap_or_default()` | `view.task_id().unwrap_or_default()` | ✓ |
| parse.rs `derive_fsm_state` | class | `get("class").and_then(as_str).unwrap_or("")` | `view.class().unwrap_or("")` | ✓ |
| parse.rs `derive_fsm_state` | details | `get("body").and_then(\|b\|b.get("details"))` | `view.body().and_then(\|b\|b.get("details"))` | ✓ |
| parse.rs | envelope_task_id deleted | inline fn | `view.task_id()` (verbatim copy) | ✓ |
| tasks.rs `inspect_tasks_by_id` | filter | `envelope_task_id(env)` | `EnvelopeView::new(env).task_id()` | ✓ |
| tasks.rs `inspect_tasks_by_id` | from/to | `get(x).and_then(as_str).unwrap_or("")` | `view.{from_str,to_str}().unwrap_or("")` | ✓ |
| tasks.rs `inspect_tasks` | by_task key | `envelope_task_id(env)` | `EnvelopeView::new(env).task_id()` | ✓ |
| poll.rs `find_match` | class/from/body | raw get-chains | `view.{class,from_str,body}()` | ✓ |
| broker/mod.rs `read_mailbox_meta_for` | last_sender | `get("from").and_then(\|f\|f.as_str().map(String::from))` | `EnvelopeView::new(v).from_str().map(String::from)` | ✓ |
| inbox.rs thread_state | body.event | `env.get("body").and_then(...event)` | `EnvelopeView::new(env).body().and_then(...event)` | ✓ |
| register.rs `emit_tail_line` | from | `get("from").and_then(as_str).unwrap_or("?")` | `view.from_str().unwrap_or("?")` | ✓ |
| register.rs `emit_tail_line` | body_raw | `get("body").map_or_else(...)` | `view.body().map_or_else(...)` | ✓ |

Kept-raw sites — decision VERIFIED CORRECT (not half-migrated):

1. **tasks.rs `inspect_tasks` peer (~line 174)**: `first.get("to").or_else(||first.get("from")).and_then(as_str).unwrap_or("")`.
   This is a **Value-level** or_else: when `to` is present-but-non-string, the
   chain keeps the `to` Value (a non-string), `and_then(as_str)` yields `None`,
   and the `from` fallback is **never tried**. The str-level equivalent
   `to_str().or_else(||from_str())` WOULD fall through to `from` in that case
   (because `to_str()` already returned `None`). The two genuinely differ for
   input `{"to": 42, "from": "agent:x"}` → raw yields `""`, str-level would
   yield `"agent:x"`. Keeping it raw preserves the original `""`. Correct, and
   the explanatory comment is accurate.

2. **register.rs `emit_tail_line` `to` field**: needs the present-but-non-string
   Value to render `t.to_string()` (debug-quote) when `to` is a structured
   target. `to_str()` would drop that to `"?"`. Raw is correct; comment accurate.

Edge cases I specifically checked and why each is equivalent:
- field absent vs present-but-non-string: raw `.get().and_then(as_str)` and the
  accessor both return `None` for both — identical defaults applied.
- `body()` returns the raw `Option<&Value>`, so the polymorphic body (string vs
  object) projects identically downstream.
- `task_id()` resolution order and the `new_task`-only `id` fallback match the
  deleted fn line-for-line (and a property test pins it over a corpus including
  `causality.ref` being a non-string).

---

## TARGET 2 — §1 hello bind_as via proxy_holder_alive (1055cdf)

### VERDICT: CLEAN (equivalent under a genuinely-enforced invariant)

Before (inline, handle.rs hello):
```rust
let holder_pid = clients.values().find_map(|s|
    if s.connected && s.name==Some(name) { s.pid } else { None });
let alive = holder_pid.is_some_and(|pid| is_alive(pid));
if !alive { reject }
```
After: `if !proxy_holder_alive(broker, &name) { reject }` where
```rust
fn proxy_holder_alive = clients.values().any(|h|
    h.connected && h.name==Some(bound) && h.pid.is_some_and(|pid| is_alive(pid)))
```

The ONE semantic difference: `find_map` returns the **first** matching client's
pid and tests liveness on *that* pid; `any` tests liveness across **all**
matches. These diverge ONLY when ≥2 connected clients share the same `name` and
the first (by ClientId order — `clients` is a `BTreeMap`, deterministic) has a
`Some`-but-DEAD pid while a later one is alive:
- input: clients {A: connected,name=X,pid=dead}, {B: connected,name=X,pid=alive}
- before: find_map stops at A's Some(dead pid) → alive=false → **reject**
- after:  any finds B alive → **accept**

Is that input reachable? **No.** `register` (handle.rs:291-295) rejects
`NameTaken` if ANY *connected* client already holds `name` — with **no liveness
check**. A dead-pid holder stays `connected=true` in the map until `tick`'s
liveness sweep removes it (handle.rs:747), and during that window a second
register for the same name is refused. Proxies carry `bind_as`, not `name`, so
they never create same-`name` holders. **Sole-assignment verified:** the only
`state.name = Some(..)` in the entire broker is register's handle.rs:326 (grep-
confirmed); both hello insert paths set `name: None` (the proxy path sets only
`bind_as: Some`). So register's NameTaken guard is the complete enforcement.
Therefore at most one connected client per name exists at any instant → exactly
one match → find_map-first-pid and any-alive coincide for all reachable states
(verified across pid=Some-alive, pid=Some-dead, pid=None, and zero-match cases —
all identical).

Note: `proxy_holder_alive` already existed and was used for the per-op liveness
re-check (`resolve_op_identity`), which always used the any-alive form. The Hello
gate was the lone find_map holdout; unifying them removes a latent
Hello-vs-per-op disagreement that could only have manifested under the
(unreachable) multi-holder case. No behavior change for any reachable input.

---

## TARGET 3 — §4 CliError→ToolError table (6b3297c + def7854)

### VERDICT: CLEAN (after def7854; 6b3297c alone had a regression, now fixed)

Shared `impl From<CliError> for ToolError` (tools/mod.rs) final state:
- `BusError{kind,message}` → `(kind, message)`
- `NotRegisteredHint{..}` → `ToolError::not_registered()` (drops name — matches all tools)
- `BrokerUnreachable` → `(BrokerUnreachable, "broker unreachable")`
- `other` → `(Internal, Display)`
- `SendArgsInvalid` → NOT in shared impl; falls to `other`→Internal.

Original per-tool arms (pre-6b3297c): await_/inbox/join/leave each had EXACTLY
`BusError | NotRegisteredHint | BrokerUnreachable | _→Internal`; send had those
four PLUS `SendArgsInvalid→EnvelopeInvalid`. The shared impl reproduces the four
common arms identically, so for every variant the four delegating tools'
behavior is unchanged.

The one real hazard (already caught by def7854): join/leave call
`normalize_channel` (util.rs:44) which returns `CliError::SendArgsInvalid` on a
bad channel, and that error propagates through `run_at_structured` → `Err(e) =>
Err(e.into())`. 6b3297c had folded `SendArgsInvalid→EnvelopeInvalid` into the
shared impl, which **silently flipped join/leave bad-channel errors from
Internal (historical) to EnvelopeInvalid**. def7854 removed that arm (join/leave
fall through to Internal = historical) and restored send.rs's explicit
`SendArgsInvalid→EnvelopeInvalid` arm placed BEFORE its `e.into()`. Verified:
join.rs/leave.rs `run_at_structured` (lines 65/46) call normalize_channel at
67/48; send.rs has the explicit arm. Net result is behavior-preserving for all
five tools. **The def7854 fix is correct and correctly placed.**

Any OTHER variant with the same bug class? **No.** The bug class is "a variant a
tool maps specially in its match but the shared wildcard now maps differently."
The shared impl only special-cases BusError/NotRegisteredHint/BrokerUnreachable —
all three were ALREADY special-cased identically in EVERY tool. No variant that
was previously `Internal` in any tool is now non-`Internal` (SendArgsInvalid is
the sole exception and is handled). Every other CliError variant
(TaskNotFound, FsmTransition, AwaitTimeout, NoIdentityBound, Disconnected,
BusClient, etc.) went to the wildcard→Internal before and after — unchanged.

---

## SUMMARY

| Target | Verdict |
|---|---|
| 1 — EnvelopeView migration (c1917fc, 74964a6) | CLEAN |
| 2 — hello via proxy_holder_alive (1055cdf) | CLEAN (equiv under enforced name-uniqueness) |
| 3 — CliError→ToolError table (6b3297c+def7854) | CLEAN (def7854 fixes 6b3297c's join/leave regression) |

No actionable divergence found. All three refactors are behavior-preserving for
every reachable input. The two kept-raw sites in Target 1 and the def7854 fix in
Target 3 are the load-bearing decisions, and all three are correct.
