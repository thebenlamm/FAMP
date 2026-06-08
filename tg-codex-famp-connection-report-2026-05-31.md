# tg-codex FAMP Connection Report

Date: 2026-05-31
Workspace: `/Users/benlamm/Workspace/torah-graph`
Requested identity: `tg-codex`
Target peer: `tg-claude`

## Summary

I completed the read-only refactoring review requested for `torah-graph`, then attempted to register with FAMP as `tg-codex` and contact `tg-claude` so we could compare findings and prepare a unified report.

The review portion succeeded. The FAMP collaboration portion did not: `tg-claude` was visible to FAMP, but every attempt to hold the `tg-codex` identity with `famp register tg-codex` failed with `broker unreachable`. I stopped the retry loop and did not send any message to `tg-claude`.

No files in `/Users/benlamm/Workspace/torah-graph` were modified.

## What Worked

`famp sessions` showed an active Claude identity:

```text
{"name":"tg-claude","pid":94775,"joined":[]}
```

`famp inspect broker` showed the broker as healthy:

```text
state: HEALTHY pid=3388 socket=/Users/benlamm/.famp/bus.sock started_at=2026-05-31T02:50:16Z build=0.1.0
```

`famp inspect identities` showed `tg-claude` registered and listening:

```text
NAME       LISTEN  CWD                                   REGISTERED            UNREAD  TOTAL  LAST_SENDER  LAST_RECEIVED
tg-claude  true    /Users/benlamm/Workspace/torah-graph  2026-05-31T02:50:17Z  0       0      (none)       -
```

`famp inbox list --as tg-claude` returned successfully:

```text
{"next_offset":0}
```

These results indicate that the broker exists, the socket exists, and at least some client commands can communicate with the broker.

## What Failed

The command intended to hold the Codex identity failed:

```text
famp register --no-reconnect tg-codex
broker unreachable
```

The long-running registration attempt also failed repeatedly:

```text
broker connect failed (broker unreachable) — reconnecting in 30s
broker connect failed (broker unreachable) — reconnecting in 30s
broker connect failed (broker unreachable) — reconnecting in 30s
```

Because `tg-codex` never registered, commands that depend on identity binding also failed:

```text
tg-codex is not registered — start `famp register tg-codex` in another terminal first
```

I stopped the long-running retrying process with Ctrl-C.

## Version And Process Checks

I checked the installed FAMP binary and the local checkout builds:

```text
/Users/benlamm/.cargo/bin/famp --version
famp 0.1.0

/Users/benlamm/Workspace/FAMP/target/release/famp --version
famp 0.1.0

/Users/benlamm/Workspace/FAMP/target/debug/famp --version
famp 0.1.0
```

The broker also reported `build=0.1.0`, so the simple explanation of a visible CLI/broker version mismatch did not hold up.

Relevant FAMP processes seen during diagnosis:

```text
3388  famp broker
94775 /Users/benlamm/.cargo/bin/famp mcp
45982 /Users/benlamm/.cargo/bin/famp mcp
9078  /Users/benlamm/.cargo/bin/famp mcp
4670  famp register tg-codex
```

PID `4670` was the retrying `tg-codex` registration process and was stopped.

## Broker Log Evidence

The broker log showed repeated broker starts and idle shutdowns, plus a protocol decode error for an `inspect` frame:

```text
client ClientId(5) frame read error: Decode(InvalidJson(Error("unknown variant `inspect`, expected one of `hello`, `register`, `send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`", line: 1, column: 38)))
```

This is notable because `famp inspect broker` and `famp inspect identities` did produce useful output from the CLI side, but the broker log suggests the broker path handling at least one `inspect` request did not recognize that request variant.

## Current Hypothesis

The problem appears narrower than “FAMP is fully down.” The broker reports healthy and can service some commands, and `tg-claude` is registered. The failure is specifically around registering a new long-lived identity from this Codex session.

Most likely causes:

1. The `register` command is taking a different broker connection path than `inspect`, `sessions`, or `inbox`, and that path is failing before or during the initial hello/register handshake.
2. There may be stale broker/socket/process state: the socket exists and broker inspection succeeds, but the registration command treats the broker as unreachable.
3. There may be a protocol compatibility issue despite matching `0.1.0` version strings. The broker log's `unknown variant inspect` line suggests not all CLI requests line up with what the broker expects.
4. The active Claude MCP registrations may be keeping the broker in a state that works for existing MCP clients but rejects or fails new long-lived register clients.

I did not kill the broker or any `tg-claude`/Claude MCP process because that could disrupt the existing Claude session the user wanted me to contact.

## Intended Message That Was Not Sent

This is the message I intended to send to `tg-claude` after registering:

```text
Hello from tg-codex. I ran the refactoring-review read-only on torah-graph. The user asked us to compare notes and prepare a unified document. My top findings: materialized graph contracts should move out of scripts into shared package contracts; concept catalog/picker policy is split across seed lists, graph state, generated slug files, and sync scripts; ingest/cli.py is a god module with top-level anthropic import and embedded orchestration; Cypher/query semantics are duplicated between web and MCP; redirected pages still carry duplicate full implementations. Please send your findings/deltas and preferred unified priorities.
```

No message was sent because the `tg-codex` identity could not be registered.

## Refactoring Review Findings Available For Reconciliation

These are the findings from my completed read-only review, ready to merge with Claude's notes once FAMP is working:

1. Promote materialized graph contracts out of `scripts/`.
   - `scripts/materialize_concept_appearances.py` produces `Concept.appearances_json`.
   - `web/pages/api/concepts/[slug]/appearances.ts` consumes that JSON shape independently.
   - The producer/consumer contract should move into package-owned schemas or fixtures.

2. Unify concept catalog and picker policy.
   - Concept sets are split between mystical and halachic seed lists, live graph categories, generated TypeScript slug files, and `scripts/sync_concept_slugs_to_ts.py`.
   - A package-level `ConceptCatalog` service with explicit profiles such as `all`, `mystical`, `halachic`, and picker-specific profiles would make policy testable.

3. Split the CLI god module.
   - `src/torah_graph/ingest/cli.py` is large and mixes Click command declarations, pipeline orchestration, provider setup, and error handling.
   - It imports `anthropic` at module import time, making the whole CLI depend on the extraction extra even for commands that do not need it.

4. Create owned Cypher/query contracts.
   - Query semantics are duplicated across `web/lib/queries.ts` and `src/torah_graph/mcp/queries.py`.
   - Debate/contested/appearance semantics can drift without golden tests or generated query fixtures.

5. Extract shared web journey/appearance logic and retire redirected page implementations where possible.
   - `/explore`, `/halacha`, and `/minhagim` permanently redirect to `/jewish-spring`, but full page implementations remain.
   - These pages duplicate props, fetch behavior, and map/appearance logic with `web/pages/jewish-spring.tsx`.

## Recommended FAMP Debugging Steps

I stopped active connection attempts as requested. Suggested next steps for whoever continues FAMP debugging:

1. Reproduce with the smallest command:

```text
famp register --no-reconnect tg-codex
```

2. Add temporary logging around the register path in:

```text
/Users/benlamm/Workspace/FAMP/crates/famp/src/cli/register.rs
/Users/benlamm/Workspace/FAMP/crates/famp/src/bus_client/mod.rs
```

3. Compare the connection and handshake path used by:

```text
famp sessions
famp inspect identities
famp inbox list --as tg-claude
famp register --no-reconnect tg-codex
```

4. Investigate why the broker log reports `unknown variant inspect` even though inspect commands appear to work from the CLI.

5. Avoid restarting the broker until coordinating with the active Claude session, because `tg-claude` is currently registered through a running `famp mcp` process.

## Final State

I stopped trying to connect to FAMP.

Known remaining FAMP-related processes after stopping my retry loop were broker/MCP processes associated with the existing environment, not a running `tg-codex` registration attempt:

```text
3388
9078
9194
45982
94775
```

No unified report was jointly prepared with `tg-claude` because live contact did not succeed.
